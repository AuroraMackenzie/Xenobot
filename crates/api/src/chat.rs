//! Chat API module for Xenobot HTTP API.
//!
//! Provides HTTP endpoints equivalent to Xenobot's `chatApi` IPC methods.

use axum::{
    extract::{Path, Query},
    response::sse::{Event, KeepAlive, Sse},
    routing::{delete, get, post},
    Json, Router,
};
use futures::{stream, TryStreamExt};
use serde::{Deserialize, Serialize};
use sqlx::{
    migrate::Migrator,
    sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions},
    Column, Row,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    convert::Infallible,
    fs,
    path::{Path as FsPath, PathBuf},
    time::{Duration, Instant},
};
use tracing::{instrument, warn};
use xenobot_analysis::parsers::{
    ChatType as AnalysisChatType, MessageType as AnalysisMessageType, ParserRegistry,
};

use crate::database::repository::{
    ChatMeta, ImportProgress, ImportSourceCheckpoint, MemberActivity, Message,
    MessageLengthDistributionResult, MessageTypeDistribution, TimeActivity,
    TimeFilter as RepoTimeFilter, TimeRange,
};
use crate::ApiError;
use xenobot_core::webhook::{
    append_dead_letter_entry, build_dead_letter_entry, merge_webhook_dispatch_stats,
    webhook_rule_matches_event, WebhookDispatchStats, WebhookMessageCreatedEvent, WebhookRule,
};

/// Chat API router.
pub fn router() -> Router {
    Router::new()
        // Migration
        .route("/check-migration", get(check_migration))
        .route("/run-migration", post(run_migration))
        // File operations
        .route("/select-file", get(select_file))
        .route("/import", post(import))
        .route("/import-batch", post(import_batch))
        .route("/detect-format", post(detect_format))
        .route("/import-with-options", post(import_with_options))
        .route("/scan-multi-chat-file", post(scan_multi_chat_file))
        // Session management
        .route("/sessions", get(get_sessions))
        .route("/sessions/:session_id", get(get_session))
        .route("/sessions/:session_id", delete(delete_session))
        .route("/sessions/:session_id/rename", post(rename_session))
        // Analysis endpoints
        .route(
            "/sessions/:session_id/available-years",
            get(get_available_years),
        )
        .route(
            "/sessions/:session_id/member-activity",
            get(get_member_activity),
        )
        .route(
            "/sessions/:session_id/member-name-history/:member_id",
            get(get_member_name_history),
        )
        .route(
            "/sessions/:session_id/hourly-activity",
            get(get_hourly_activity),
        )
        .route(
            "/sessions/:session_id/daily-activity",
            get(get_daily_activity),
        )
        .route(
            "/sessions/:session_id/weekday-activity",
            get(get_weekday_activity),
        )
        .route(
            "/sessions/:session_id/monthly-activity",
            get(get_monthly_activity),
        )
        .route(
            "/sessions/:session_id/yearly-activity",
            get(get_yearly_activity),
        )
        .route(
            "/sessions/:session_id/message-length-distribution",
            get(get_message_length_distribution),
        )
        .route(
            "/sessions/:session_id/message-type-distribution",
            get(get_message_type_distribution),
        )
        .route("/sessions/:session_id/time-range", get(get_time_range))
        // Utility
        .route("/db-directory", get(get_db_directory))
        .route("/supported-formats", get(get_supported_formats))
        // Advanced analysis
        .route(
            "/sessions/:session_id/catchphrase-analysis",
            get(get_catchphrase_analysis),
        )
        .route(
            "/sessions/:session_id/mention-analysis",
            get(get_mention_analysis),
        )
        .route(
            "/sessions/:session_id/mention-graph",
            get(get_mention_graph),
        )
        .route(
            "/sessions/:session_id/cluster-graph",
            get(get_cluster_graph),
        )
        .route(
            "/sessions/:session_id/laugh-analysis",
            get(get_laugh_analysis),
        )
        .route(
            "/sessions/:session_id/night-owl-analysis",
            get(get_night_owl_analysis),
        )
        .route(
            "/sessions/:session_id/dragon-king-analysis",
            get(get_dragon_king_analysis),
        )
        .route(
            "/sessions/:session_id/lurker-analysis",
            get(get_lurker_analysis),
        )
        .route(
            "/sessions/:session_id/checkin-analysis",
            get(get_checkin_analysis),
        )
        .route(
            "/sessions/:session_id/repeat-analysis",
            get(get_repeat_analysis),
        )
        // Member management
        .route("/sessions/:session_id/members", get(get_members))
        .route(
            "/sessions/:session_id/members-paginated",
            get(get_members_paginated),
        )
        .route(
            "/sessions/:session_id/members/:member_id/aliases",
            post(update_member_aliases),
        )
        .route(
            "/sessions/:session_id/members/:member_id",
            delete(delete_member),
        )
        .route("/sessions/:session_id/owner", post(update_session_owner_id))
        // Plugin and SQL
        .route("/sessions/:session_id/plugin-query", post(plugin_query))
        .route("/plugin-compute", post(plugin_compute))
        .route("/sessions/:session_id/execute-sql", post(execute_sql))
        .route(
            "/sessions/:session_id/generate-sql",
            post(generate_sql_assist),
        )
        .route("/sessions/:session_id/schema", get(get_schema))
        // Incremental import
        .route(
            "/sessions/:session_id/analyze-incremental-import",
            post(analyze_incremental_import),
        )
        .route(
            "/sessions/:session_id/incremental-import",
            post(incremental_import),
        )
        // Export and temp files
        .route(
            "/export-sessions-to-temp-files",
            post(export_sessions_to_temp_files),
        )
        .route(
            "/cleanup-temp-export-files",
            post(cleanup_temp_export_files),
        )
        // Event listeners (SSE)
        .route("/import-progress", get(import_progress_sse))
}

// ==================== Request/Response Types ====================
#[derive(Debug, Deserialize)]
pub struct TimeFilter {
    start_ts: Option<i64>,
    end_ts: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportProgressResponse {
    pub total: u64,
    pub processed: u64,
    pub current_file: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionResponse {
    pub id: i64,
    pub name: String,
    pub platform: String,
    pub chat_type: String,
    pub imported_at: i64,
    pub message_count: Option<i64>,
    pub member_count: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisSession {
    pub id: i64,
    pub name: String,
    pub platform: String,
    pub chat_type: String,
    pub imported_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeRangeResponse {
    pub earliest: Option<i64>,
    pub latest: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SupportedFormat {
    pub id: String,
    pub name: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemberResponse {
    pub id: i64,
    pub platform_id: String,
    pub account_name: Option<String>,
    pub group_nickname: Option<String>,
    pub aliases: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemberWithStats {
    pub id: i64,
    pub platform_id: String,
    pub account_name: Option<String>,
    pub group_nickname: Option<String>,
    pub aliases: Vec<String>,
    pub message_count: i64,
    pub avatar: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MembersPaginatedResult {
    pub members: Vec<MemberWithStats>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemberActivityResponse {
    pub members: Vec<MemberActivityItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemberActivityItem {
    pub member_id: i64,
    pub account_name: Option<String>,
    pub message_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeDistributionResponse {
    pub distribution: Vec<TimeDistributionItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeDistributionItem {
    pub period: i64,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberNameHistoryResponse {
    pub name_type: String,
    pub name: String,
    pub start_ts: i64,
    pub end_ts: Option<i64>,
}

#[derive(Debug, Default, Clone)]
struct SkipReasons {
    no_sender_id: usize,
    no_account_name: usize,
    invalid_timestamp: usize,
    no_type: usize,
}

#[derive(Debug, Default, Clone)]
struct ImportParseStats {
    messages_received: usize,
    messages_written: usize,
    messages_skipped: usize,
    skip_reasons: SkipReasons,
}

#[derive(Debug, Clone)]
struct ParsedMessage {
    sender_platform_id: String,
    sender_name: Option<String>,
    ts: i64,
    msg_type: i64,
    content: Option<String>,
    platform_message_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ParsedMemberProfile {
    platform_id: String,
    account_name: Option<String>,
    group_nickname: Option<String>,
}

#[derive(Debug, Clone)]
struct ParsedChatPayload {
    name: String,
    platform: String,
    chat_type: String,
    members: Vec<ParsedMemberProfile>,
    messages: Vec<ParsedMessage>,
}

#[derive(Debug, Clone)]
struct DetectedFormat {
    id: String,
    platform: String,
    multi_chat: bool,
    confidence: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MultiChatScanItem {
    index: usize,
    name: String,
    #[serde(rename = "type")]
    chat_type: String,
    id: i64,
    message_count: i64,
}

const WEBHOOK_BATCH_SIZE_DEFAULT: usize = 64;
const WEBHOOK_MAX_CONCURRENCY_DEFAULT: usize = 8;
const WEBHOOK_REQUEST_TIMEOUT_MS_DEFAULT: u64 = 8_000;
const WEBHOOK_FLUSH_INTERVAL_MS_DEFAULT: u64 = 1_200;
const WEBHOOK_RETRY_ATTEMPTS_DEFAULT: u32 = 3;
const WEBHOOK_RETRY_BASE_DELAY_MS_DEFAULT: u64 = 150;
const SYSTEM_MEMBER_LABEL: &str = "系统消息";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiWebhookItem {
    id: String,
    url: String,
    #[serde(default, alias = "eventType")]
    event_type: Option<String>,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default, alias = "chatName")]
    chat_name: Option<String>,
    #[serde(default, alias = "metaId")]
    meta_id: Option<i64>,
    #[serde(default)]
    sender: Option<String>,
    #[serde(default)]
    keyword: Option<String>,
    #[serde(default, alias = "createdAt")]
    created_at: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct ApiWebhookDispatchSettings {
    #[serde(default, alias = "batchSize")]
    batch_size: Option<usize>,
    #[serde(default, alias = "maxConcurrency")]
    max_concurrency: Option<usize>,
    #[serde(default, alias = "requestTimeoutMs")]
    request_timeout_ms: Option<u64>,
    #[serde(default, alias = "flushIntervalMs")]
    flush_interval_ms: Option<u64>,
    #[serde(default, alias = "retryAttempts")]
    retry_attempts: Option<u32>,
    #[serde(default, alias = "retryBaseDelayMs")]
    retry_base_delay_ms: Option<u64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct ApiWebhookStore {
    #[serde(default)]
    items: Vec<ApiWebhookItem>,
    #[serde(default)]
    dispatch: ApiWebhookDispatchSettings,
}

#[derive(Debug, Clone)]
struct WebhookDispatchConfig {
    batch_size: usize,
    max_concurrency: usize,
    request_timeout_ms: u64,
    flush_interval_ms: u64,
    retry_attempts: u32,
    retry_base_delay_ms: u64,
}

impl Default for WebhookDispatchConfig {
    fn default() -> Self {
        Self {
            batch_size: WEBHOOK_BATCH_SIZE_DEFAULT,
            max_concurrency: WEBHOOK_MAX_CONCURRENCY_DEFAULT,
            request_timeout_ms: WEBHOOK_REQUEST_TIMEOUT_MS_DEFAULT,
            flush_interval_ms: WEBHOOK_FLUSH_INTERVAL_MS_DEFAULT,
            retry_attempts: WEBHOOK_RETRY_ATTEMPTS_DEFAULT,
            retry_base_delay_ms: WEBHOOK_RETRY_BASE_DELAY_MS_DEFAULT,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ApiWebhookRuntimeConfig {
    rules: Vec<WebhookRule>,
    dispatch: WebhookDispatchConfig,
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn api_webhook_store_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot")
        .join("webhooks.json")
}

fn api_webhook_item_to_rule(item: &ApiWebhookItem) -> WebhookRule {
    WebhookRule {
        id: item.id.clone(),
        url: item.url.clone(),
        event_type: item.event_type.clone(),
        platform: item.platform.clone(),
        chat_name: item.chat_name.clone(),
        meta_id: item.meta_id,
        sender: item.sender.clone(),
        keyword: item.keyword.clone(),
        created_at: item.created_at.clone(),
    }
}

fn sanitize_webhook_dispatch_settings(value: &ApiWebhookDispatchSettings) -> WebhookDispatchConfig {
    fn clamp_usize(value: Option<usize>, default_value: usize, min: usize, max: usize) -> usize {
        value.unwrap_or(default_value).clamp(min, max)
    }

    fn clamp_u64(value: Option<u64>, default_value: u64, min: u64, max: u64) -> u64 {
        value.unwrap_or(default_value).clamp(min, max)
    }

    fn clamp_u32(value: Option<u32>, default_value: u32, min: u32, max: u32) -> u32 {
        value.unwrap_or(default_value).clamp(min, max)
    }

    WebhookDispatchConfig {
        batch_size: clamp_usize(value.batch_size, WEBHOOK_BATCH_SIZE_DEFAULT, 1, 1024),
        max_concurrency: clamp_usize(
            value.max_concurrency,
            WEBHOOK_MAX_CONCURRENCY_DEFAULT,
            1,
            64,
        ),
        request_timeout_ms: clamp_u64(
            value.request_timeout_ms,
            WEBHOOK_REQUEST_TIMEOUT_MS_DEFAULT,
            200,
            60_000,
        ),
        flush_interval_ms: clamp_u64(
            value.flush_interval_ms,
            WEBHOOK_FLUSH_INTERVAL_MS_DEFAULT,
            0,
            30_000,
        ),
        retry_attempts: clamp_u32(value.retry_attempts, WEBHOOK_RETRY_ATTEMPTS_DEFAULT, 1, 8),
        retry_base_delay_ms: clamp_u64(
            value.retry_base_delay_ms,
            WEBHOOK_RETRY_BASE_DELAY_MS_DEFAULT,
            0,
            5_000,
        ),
    }
}

fn should_flush_webhook_queue(
    queue_len: usize,
    queue_age: Option<Duration>,
    dispatch: &WebhookDispatchConfig,
) -> bool {
    if queue_len == 0 {
        return false;
    }
    if queue_len >= dispatch.batch_size {
        return true;
    }
    queue_age
        .map(|age| age >= Duration::from_millis(dispatch.flush_interval_ms))
        .unwrap_or(false)
}

fn read_api_webhook_config() -> ApiWebhookRuntimeConfig {
    let mut runtime = ApiWebhookRuntimeConfig::default();
    let path = api_webhook_store_path();
    if !path.exists() {
        return runtime;
    }

    let raw = match fs::read_to_string(&path) {
        Ok(v) => v,
        Err(e) => {
            warn!("failed to read webhook config '{}': {}", path.display(), e);
            return runtime;
        }
    };
    if raw.trim().is_empty() {
        return runtime;
    }

    match serde_json::from_str::<ApiWebhookStore>(&raw) {
        Ok(store) => {
            runtime.rules = store.items.iter().map(api_webhook_item_to_rule).collect();
            runtime.dispatch = sanitize_webhook_dispatch_settings(&store.dispatch);
            runtime
        }
        Err(e) => {
            warn!("failed to parse webhook config '{}': {}", path.display(), e);
            runtime
        }
    }
}

async fn dispatch_api_webhook_message_created(
    client: &reqwest::Client,
    items: &[WebhookRule],
    dispatch: &WebhookDispatchConfig,
    event: &WebhookMessageCreatedEvent,
) -> WebhookDispatchStats {
    let mut stats = WebhookDispatchStats::default();
    for item in items {
        if !webhook_rule_matches_event(item, event) {
            stats.filtered += 1;
            continue;
        }
        stats.attempted += 1;

        let mut delivered = false;
        let mut attempts_used = 0u32;
        let mut last_error = "unknown delivery failure".to_string();
        for attempt in 0..dispatch.retry_attempts {
            attempts_used = attempt.saturating_add(1);
            let send_result = client
                .post(&item.url)
                .header("X-Xenobot-Event", &event.event_type)
                .header("X-Xenobot-Webhook-Id", &item.id)
                .json(event)
                .send()
                .await;

            match send_result {
                Ok(resp) if resp.status().is_success() => {
                    stats.delivered += 1;
                    delivered = true;
                    break;
                }
                Ok(resp) => {
                    last_error = format!("http status {}", resp.status());
                    if attempt.saturating_add(1) < dispatch.retry_attempts {
                        let backoff_factor = 1_u64 << attempt.min(10);
                        let wait_ms = dispatch.retry_base_delay_ms.saturating_mul(backoff_factor);
                        tokio::time::sleep(Duration::from_millis(wait_ms)).await;
                    }
                }
                Err(err) => {
                    last_error = err.to_string();
                    if attempt.saturating_add(1) < dispatch.retry_attempts {
                        let backoff_factor = 1_u64 << attempt.min(10);
                        let wait_ms = dispatch.retry_base_delay_ms.saturating_mul(backoff_factor);
                        tokio::time::sleep(Duration::from_millis(wait_ms)).await;
                    }
                }
            }
        }
        if !delivered {
            stats.failed += 1;
            let entry = build_dead_letter_entry(item, event, attempts_used, last_error);
            if let Err(err) = append_dead_letter_entry(&entry) {
                warn!(
                    "failed to persist webhook dead-letter entry {}: {}",
                    entry.id, err
                );
            }
        }
    }
    stats
}

async fn dispatch_api_webhook_batch(
    client: &reqwest::Client,
    items: &[WebhookRule],
    queue: &mut Vec<WebhookMessageCreatedEvent>,
    dispatch: &WebhookDispatchConfig,
) -> WebhookDispatchStats {
    if queue.is_empty() {
        return WebhookDispatchStats::default();
    }

    let mut set = tokio::task::JoinSet::new();
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(dispatch.max_concurrency));
    let shared_items = std::sync::Arc::new(items.to_vec());
    let shared_dispatch = std::sync::Arc::new(dispatch.clone());

    for event in queue.drain(..) {
        let client_clone = client.clone();
        let items_clone = shared_items.clone();
        let semaphore_clone = semaphore.clone();
        let dispatch_clone = shared_dispatch.clone();
        set.spawn(async move {
            let _permit = semaphore_clone.acquire_owned().await.ok();
            dispatch_api_webhook_message_created(
                &client_clone,
                items_clone.as_slice(),
                dispatch_clone.as_ref(),
                &event,
            )
            .await
        });
    }

    let mut total = WebhookDispatchStats::default();
    while let Some(joined) = set.join_next().await {
        match joined {
            Ok(stats) => merge_webhook_dispatch_stats(&mut total, &stats),
            Err(_) => {
                total.failed = total.failed.saturating_add(1);
            }
        }
    }

    total
}

fn infer_platform_from_path(file_path: &str) -> String {
    fn contains_alias_token(path: &str, token: &str) -> bool {
        let normalized = path
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
            .collect::<String>();
        normalized.split_whitespace().any(|part| part == token)
    }

    let lower = file_path.to_ascii_lowercase();
    if lower.contains("telegram") || contains_alias_token(&lower, "tg") {
        "telegram".to_string()
    } else if lower.contains("wechat") || contains_alias_token(&lower, "wx") {
        "wechat".to_string()
    } else if lower.contains("imessage") || lower.contains("messages") {
        "imessage".to_string()
    } else if lower.contains("messenger") || lower.contains("facebook") {
        "messenger".to_string()
    } else if lower.contains("kakaotalk") || lower.contains("kakao") {
        "kakaotalk".to_string()
    } else if lower.contains("qq") {
        "qq".to_string()
    } else if lower.contains("line") {
        "line".to_string()
    } else if lower.contains("whatsapp") || contains_alias_token(&lower, "wa") {
        "whatsapp".to_string()
    } else if lower.contains("discord") {
        "discord".to_string()
    } else if lower.contains("instagram") || contains_alias_token(&lower, "ig") {
        "instagram".to_string()
    } else if lower.contains("slack") {
        "slack".to_string()
    } else if lower.contains("teams") || lower.contains("msteams") {
        "teams".to_string()
    } else if lower.contains("signal") {
        "signal".to_string()
    } else if lower.contains("skype") {
        "skype".to_string()
    } else if lower.contains("googlechat") || lower.contains("hangouts") || lower.contains("google")
    {
        "googlechat".to_string()
    } else if lower.contains("zoom") {
        "zoom".to_string()
    } else if lower.contains("viber") {
        "viber".to_string()
    } else {
        "generic".to_string()
    }
}

fn classify_chat_type(raw_type: &str) -> String {
    let t = raw_type.to_ascii_lowercase();
    if t.contains("private") || t.contains("personal") || t.contains("bot") || t.contains("saved") {
        "private".to_string()
    } else {
        "group".to_string()
    }
}

fn file_stem_name(file_path: &str) -> String {
    FsPath::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Imported Chat".to_string())
}

fn normalized_timestamp(raw: i64) -> Option<i64> {
    let mut ts = raw;
    if ts > 10_000_000_000 {
        ts /= 1000;
    }
    if ts <= 0 {
        None
    } else {
        Some(ts)
    }
}

fn as_i64(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn as_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn extract_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        serde_json::Value::Array(arr) => {
            let mut parts = Vec::new();
            for item in arr {
                if let Some(text) = extract_text(item) {
                    parts.push(text);
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join(""))
            }
        }
        serde_json::Value::Object(obj) => {
            for key in ["text", "content", "message", "msg", "title"] {
                if let Some(v) = obj.get(key) {
                    if let Some(text) = extract_text(v) {
                        return Some(text);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn detect_format_from_bytes(file_path: &str, bytes: &[u8]) -> DetectedFormat {
    let ext = FsPath::new(file_path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let text = String::from_utf8_lossy(bytes);
    let trimmed = text.trim_start();
    let inferred_platform = infer_platform_from_path(file_path);

    if ext == "jsonl" {
        return DetectedFormat {
            id: "jsonl".to_string(),
            platform: inferred_platform,
            multi_chat: false,
            confidence: 0.90,
        };
    }

    if trimmed.starts_with('{') || trimmed.starts_with('[') || ext == "json" {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if extract_multi_chat_items(&value).is_some() {
                return DetectedFormat {
                    id: "telegram-json".to_string(),
                    platform: "telegram".to_string(),
                    multi_chat: true,
                    confidence: 0.96,
                };
            }

            if value.get("messages").and_then(|v| v.as_array()).is_some() {
                return DetectedFormat {
                    id: "json-chat".to_string(),
                    platform: inferred_platform,
                    multi_chat: false,
                    confidence: 0.92,
                };
            }

            if value.is_array() {
                return DetectedFormat {
                    id: "json-array".to_string(),
                    platform: inferred_platform,
                    multi_chat: false,
                    confidence: 0.88,
                };
            }

            return DetectedFormat {
                id: "json".to_string(),
                platform: inferred_platform,
                multi_chat: false,
                confidence: 0.80,
            };
        }
    }

    if ext == "txt" || ext == "md" {
        return DetectedFormat {
            id: "text".to_string(),
            platform: inferred_platform,
            multi_chat: false,
            confidence: 0.78,
        };
    }

    DetectedFormat {
        id: "unknown".to_string(),
        platform: inferred_platform,
        multi_chat: false,
        confidence: 0.30,
    }
}

fn extract_multi_chat_array<'a>(value: &'a serde_json::Value) -> Option<&'a [serde_json::Value]> {
    if let Some(chats) = value
        .get("chats")
        .and_then(|c| c.get("list"))
        .and_then(|list| list.as_array())
    {
        return Some(chats.as_slice());
    }

    if let Some(chats) = value.get("chats").and_then(|c| c.as_array()) {
        return Some(chats.as_slice());
    }

    None
}

fn extract_multi_chat_items(value: &serde_json::Value) -> Option<Vec<MultiChatScanItem>> {
    let chats = extract_multi_chat_array(value)?;
    let mut out = Vec::with_capacity(chats.len());
    for (index, chat) in chats.iter().enumerate() {
        let obj = match chat.as_object() {
            Some(v) => v,
            None => continue,
        };
        let id = obj.get("id").and_then(as_i64).unwrap_or(index as i64);
        let name = obj
            .get("name")
            .or_else(|| obj.get("title"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Chat {}", id));
        let chat_type = obj
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let message_count = obj
            .get("messages")
            .and_then(|v| v.as_array())
            .map(|arr| arr.len() as i64)
            .unwrap_or(0);
        out.push(MultiChatScanItem {
            index,
            name,
            chat_type,
            id,
            message_count,
        });
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn extract_sender_from_object(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> (Option<String>, Option<String>) {
    let mut sender_id: Option<String> = None;
    let mut sender_name: Option<String> = None;

    for id_key in [
        "sender_id",
        "from_id",
        "user_id",
        "author_id",
        "uid",
        "platform_id",
    ] {
        if let Some(v) = obj.get(id_key).and_then(as_string) {
            sender_id = Some(v);
            break;
        }
    }

    for name_key in [
        "sender_name",
        "sender",
        "from_name",
        "from",
        "author",
        "name",
        "nickname",
        "account_name",
    ] {
        if let Some(v) = obj.get(name_key) {
            if v.is_object() {
                if let Some(inner) = v
                    .get("name")
                    .or_else(|| v.get("username"))
                    .or_else(|| v.get("display_name"))
                    .and_then(|x| x.as_str())
                {
                    sender_name = Some(inner.to_string());
                    if sender_id.is_none() {
                        sender_id = v
                            .get("id")
                            .or_else(|| v.get("uid"))
                            .or_else(|| v.get("user_id"))
                            .and_then(as_string);
                    }
                    break;
                }
            } else if let Some(s) = v.as_str() {
                sender_name = Some(s.to_string());
                if sender_id.is_none() {
                    sender_id = Some(s.to_string());
                }
                break;
            }
        }
    }

    (sender_id, sender_name)
}

fn extract_timestamp_from_object(obj: &serde_json::Map<String, serde_json::Value>) -> Option<i64> {
    for key in [
        "timestamp",
        "ts",
        "time",
        "date_unixtime",
        "date",
        "send_time",
        "create_time",
    ] {
        if let Some(raw) = obj.get(key).and_then(as_i64) {
            if let Some(ts) = normalized_timestamp(raw) {
                return Some(ts);
            }
        }
    }
    None
}

fn extract_message_type_from_object(
    obj: &serde_json::Map<String, serde_json::Value>,
    stats: &mut ImportParseStats,
) -> i64 {
    for key in ["msg_type", "message_type", "type"] {
        if let Some(v) = obj.get(key) {
            if let Some(n) = as_i64(v) {
                return n;
            }
            if let Some(s) = v.as_str() {
                let lower = s.to_ascii_lowercase();
                if lower.contains("text") || lower == "message" {
                    return 0;
                }
                if lower.contains("image") || lower.contains("photo") || lower.contains("sticker") {
                    return 1;
                }
                if lower.contains("voice") || lower.contains("audio") {
                    return 2;
                }
                if lower.contains("video") {
                    return 3;
                }
                if lower.contains("file") || lower.contains("document") {
                    return 4;
                }
                return 0;
            }
        }
    }
    stats.skip_reasons.no_type += 1;
    0
}

fn parse_message_from_object(
    obj: &serde_json::Map<String, serde_json::Value>,
    fallback_sender: &str,
    stats: &mut ImportParseStats,
) -> Option<ParsedMessage> {
    stats.messages_received += 1;

    let ts = match extract_timestamp_from_object(obj) {
        Some(v) => v,
        None => {
            stats.messages_skipped += 1;
            stats.skip_reasons.invalid_timestamp += 1;
            return None;
        }
    };

    let (mut sender_id, mut sender_name) = extract_sender_from_object(obj);
    if sender_id.as_deref().unwrap_or_default().trim().is_empty() {
        if let Some(name) = sender_name.clone() {
            sender_id = Some(format!("name:{}", name.trim()));
        } else if !fallback_sender.is_empty() {
            sender_id = Some(fallback_sender.to_string());
        } else {
            stats.messages_skipped += 1;
            stats.skip_reasons.no_sender_id += 1;
            return None;
        }
    }

    if sender_name.is_none() {
        stats.skip_reasons.no_account_name += 1;
        sender_name = sender_id.clone();
    }

    let msg_type = extract_message_type_from_object(obj, stats);
    let sender_platform_id = sender_id.unwrap_or_else(|| "unknown".to_string());
    let (_, normalized_sender_name) = canonicalize_member_names(
        &sender_platform_id,
        sender_name.clone(),
        sender_name.clone(),
        Some(msg_type),
    );
    let content = obj
        .get("content")
        .or_else(|| obj.get("text"))
        .or_else(|| obj.get("message"))
        .or_else(|| obj.get("msg"))
        .and_then(extract_text);

    let platform_message_id = obj
        .get("platform_message_id")
        .or_else(|| obj.get("message_id"))
        .or_else(|| obj.get("msg_id"))
        .or_else(|| obj.get("id"))
        .and_then(as_string);

    Some(ParsedMessage {
        sender_platform_id,
        sender_name: normalized_sender_name,
        ts,
        msg_type,
        content,
        platform_message_id,
    })
}

fn parse_chat_payload_from_json(
    file_path: &str,
    value: &serde_json::Value,
    detected: &DetectedFormat,
    chat_index: Option<usize>,
    stats: &mut ImportParseStats,
) -> Result<ParsedChatPayload, ApiError> {
    if let Some(chats) = extract_multi_chat_array(value) {
        let index = chat_index.unwrap_or(0);
        let chat = chats
            .get(index)
            .ok_or_else(|| ApiError::InvalidRequest("error.invalid_chat_index".to_string()))?;
        let chat_obj = chat
            .as_object()
            .ok_or_else(|| ApiError::InvalidRequest("error.invalid_chat_object".to_string()))?;
        let name = chat_obj
            .get("name")
            .or_else(|| chat_obj.get("title"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("Chat {}", index));
        let raw_type = chat_obj
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("group");
        let mut messages = Vec::new();
        if let Some(arr) = chat_obj.get("messages").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(obj) = item.as_object() {
                    if let Some(msg) = parse_message_from_object(obj, "system", stats) {
                        messages.push(msg);
                    }
                } else {
                    stats.messages_received += 1;
                    stats.messages_skipped += 1;
                }
            }
        }
        return Ok(ParsedChatPayload {
            name,
            platform: detected.platform.clone(),
            chat_type: classify_chat_type(raw_type),
            members: vec![],
            messages,
        });
    }

    if let Some(obj) = value.as_object() {
        if let Some(arr) = obj.get("messages").and_then(|v| v.as_array()) {
            let name = obj
                .get("name")
                .or_else(|| obj.get("title"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| file_stem_name(file_path));
            let raw_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("group");
            let mut messages = Vec::new();
            for item in arr {
                if let Some(item_obj) = item.as_object() {
                    if let Some(msg) = parse_message_from_object(item_obj, "system", stats) {
                        messages.push(msg);
                    }
                } else {
                    stats.messages_received += 1;
                    stats.messages_skipped += 1;
                }
            }
            return Ok(ParsedChatPayload {
                name,
                platform: detected.platform.clone(),
                chat_type: classify_chat_type(raw_type),
                members: vec![],
                messages,
            });
        }
    }

    if let Some(arr) = value.as_array() {
        let mut messages = Vec::new();
        for item in arr {
            if let Some(obj) = item.as_object() {
                if let Some(msg) = parse_message_from_object(obj, "unknown", stats) {
                    messages.push(msg);
                }
            } else {
                stats.messages_received += 1;
                stats.messages_skipped += 1;
            }
        }
        return Ok(ParsedChatPayload {
            name: file_stem_name(file_path),
            platform: detected.platform.clone(),
            chat_type: "group".to_string(),
            members: vec![],
            messages,
        });
    }

    Err(ApiError::InvalidRequest(
        "error.unrecognized_format".to_string(),
    ))
}

fn parse_chat_payload_from_text(
    file_path: &str,
    text: &str,
    detected: &DetectedFormat,
    stats: &mut ImportParseStats,
) -> ParsedChatPayload {
    let base_ts = now_ts();
    let mut messages = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let content = line.trim();
        if content.is_empty() {
            continue;
        }
        stats.messages_received += 1;
        messages.push(ParsedMessage {
            sender_platform_id: "text-importer".to_string(),
            sender_name: Some("文本导入".to_string()),
            ts: base_ts + idx as i64,
            msg_type: 0,
            content: Some(content.to_string()),
            platform_message_id: None,
        });
    }
    ParsedChatPayload {
        name: file_stem_name(file_path),
        platform: detected.platform.clone(),
        chat_type: "group".to_string(),
        members: vec![],
        messages,
    }
}

fn parse_chat_payload_from_jsonl(
    file_path: &str,
    text: &str,
    detected: &DetectedFormat,
    stats: &mut ImportParseStats,
) -> ParsedChatPayload {
    let mut messages = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(value) => {
                if let Some(obj) = value.as_object() {
                    if let Some(msg) = parse_message_from_object(obj, "unknown", stats) {
                        messages.push(msg);
                    }
                } else {
                    stats.messages_received += 1;
                    stats.messages_skipped += 1;
                }
            }
            Err(_) => {
                stats.messages_received += 1;
                stats.messages_skipped += 1;
            }
        }
    }
    ParsedChatPayload {
        name: file_stem_name(file_path),
        platform: detected.platform.clone(),
        chat_type: "group".to_string(),
        members: vec![],
        messages,
    }
}

fn analysis_message_type_to_code(msg_type: &AnalysisMessageType) -> i64 {
    match msg_type {
        AnalysisMessageType::Text => 0,
        AnalysisMessageType::Image => 1,
        AnalysisMessageType::Audio => 2,
        AnalysisMessageType::Video => 3,
        AnalysisMessageType::File => 4,
        AnalysisMessageType::Sticker => 5,
        AnalysisMessageType::Location => 6,
        AnalysisMessageType::System => 7,
        AnalysisMessageType::Link => 8,
    }
}

fn analysis_chat_type_to_text(chat_type: &AnalysisChatType) -> String {
    match chat_type {
        AnalysisChatType::Private => "private".to_string(),
        AnalysisChatType::Group => "group".to_string(),
    }
}

fn normalize_member_name(value: Option<String>) -> Option<String> {
    value.and_then(|inner| {
        let trimmed = inner.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn is_system_member_marker(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "system" | "system message" | "system_message" | "系统消息" | "系统"
    )
}

fn canonicalize_member_names(
    platform_id: &str,
    account_name: Option<String>,
    group_nickname: Option<String>,
    msg_type: Option<i64>,
) -> (Option<String>, Option<String>) {
    let normalized_account_name = normalize_member_name(account_name);
    let normalized_group_nickname = normalize_member_name(group_nickname);
    let force_system = msg_type == Some(7)
        || platform_id
            .rsplit(':')
            .next()
            .is_some_and(is_system_member_marker)
        || normalized_account_name
            .as_deref()
            .is_some_and(is_system_member_marker)
        || normalized_group_nickname
            .as_deref()
            .is_some_and(is_system_member_marker);

    if force_system {
        (
            Some(SYSTEM_MEMBER_LABEL.to_string()),
            Some(SYSTEM_MEMBER_LABEL.to_string()),
        )
    } else {
        (normalized_account_name, normalized_group_nickname)
    }
}

fn upsert_parsed_member_profile(
    members: &mut BTreeMap<String, ParsedMemberProfile>,
    platform_id: String,
    account_name: Option<String>,
    group_nickname: Option<String>,
    msg_type: Option<i64>,
) {
    let (normalized_account_name, normalized_group_nickname) =
        canonicalize_member_names(&platform_id, account_name, group_nickname, msg_type);

    members
        .entry(platform_id.clone())
        .and_modify(|existing| {
            if existing.account_name.is_none() {
                existing.account_name = normalized_account_name.clone();
            }
            if let Some(new_group_nickname) = normalized_group_nickname.clone() {
                let should_replace_group_nickname = match existing.group_nickname.as_deref() {
                    None => true,
                    Some(existing_group_nickname) => existing
                        .account_name
                        .as_deref()
                        .is_some_and(|account_name| existing_group_nickname == account_name),
                };
                if should_replace_group_nickname {
                    existing.group_nickname = Some(new_group_nickname);
                }
            }
        })
        .or_insert(ParsedMemberProfile {
            platform_id,
            account_name: normalized_account_name,
            group_nickname: normalized_group_nickname,
        });
}

async fn ensure_member_profile_and_history(
    repo: &crate::database::Repository,
    profile: &ParsedMemberProfile,
    start_ts: i64,
) -> Result<i64, ApiError> {
    let member_id = repo
        .get_or_create_member_profile(
            &profile.platform_id,
            profile.account_name.as_deref(),
            profile.group_nickname.as_deref(),
        )
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    if let Some(account_name) = profile.account_name.as_deref() {
        repo.ensure_member_name_history_entry(member_id, "account_name", account_name, start_ts)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
    }
    if let Some(group_nickname) = profile.group_nickname.as_deref() {
        repo.ensure_member_name_history_entry(
            member_id,
            "group_nickname",
            group_nickname,
            start_ts,
        )
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    }

    Ok(member_id)
}

async fn parse_with_analysis_registry(
    file_path: &str,
) -> Option<(DetectedFormat, ParsedChatPayload, ImportParseStats)> {
    let owned_path = file_path.to_string();
    let parsed_chat = tokio::task::spawn_blocking(move || {
        let registry = ParserRegistry::new();
        registry
            .detect_and_parse(FsPath::new(&owned_path))
            .ok()
            .filter(|chat| !chat.messages.is_empty())
    })
    .await
    .ok()
    .flatten()?;

    let platform = if parsed_chat.platform.trim().is_empty() {
        infer_platform_from_path(file_path)
    } else {
        parsed_chat.platform.to_ascii_lowercase()
    };
    let analysis_chat_name = parsed_chat.chat_name.clone();
    let analysis_chat_type = analysis_chat_type_to_text(&parsed_chat.chat_type);

    let mut stats = ImportParseStats::default();
    stats.messages_received = parsed_chat.messages.len();

    let mut members = BTreeMap::new();
    for member in &parsed_chat.members {
        let raw_member_id = member.id.trim();
        if raw_member_id.is_empty() {
            continue;
        }
        upsert_parsed_member_profile(
            &mut members,
            format!("{}:{}", platform, raw_member_id),
            member.name.clone(),
            member.display_name.clone().or(member.name.clone()),
            None,
        );
    }

    let mut messages = Vec::with_capacity(parsed_chat.messages.len());
    for msg in parsed_chat.messages {
        if msg.timestamp <= 0 {
            stats.messages_skipped = stats.messages_skipped.saturating_add(1);
            stats.skip_reasons.invalid_timestamp =
                stats.skip_reasons.invalid_timestamp.saturating_add(1);
            continue;
        }
        let sender = msg.sender.trim();
        if sender.is_empty() {
            stats.messages_skipped = stats.messages_skipped.saturating_add(1);
            stats.skip_reasons.no_sender_id = stats.skip_reasons.no_sender_id.saturating_add(1);
            continue;
        }
        if msg.sender_name.is_none() {
            stats.skip_reasons.no_account_name =
                stats.skip_reasons.no_account_name.saturating_add(1);
        }
        let content = msg.content.trim();
        if content.is_empty() {
            stats.messages_skipped = stats.messages_skipped.saturating_add(1);
            continue;
        }
        let sender_platform_id = format!("{}:{}", platform, sender);
        let msg_type_code = analysis_message_type_to_code(&msg.msg_type);
        let (_, normalized_sender_name) = canonicalize_member_names(
            &sender_platform_id,
            msg.sender_name.clone(),
            msg.sender_name.clone(),
            Some(msg_type_code),
        );
        upsert_parsed_member_profile(
            &mut members,
            sender_platform_id.clone(),
            normalized_sender_name.clone(),
            normalized_sender_name.clone(),
            Some(msg_type_code),
        );
        messages.push(ParsedMessage {
            sender_platform_id,
            sender_name: normalized_sender_name,
            ts: msg.timestamp,
            msg_type: msg_type_code,
            content: Some(content.to_string()),
            platform_message_id: None,
        });
    }

    if messages.is_empty() {
        return None;
    }

    Some((
        DetectedFormat {
            id: format!("analysis-{}", platform),
            platform: platform.clone(),
            multi_chat: false,
            confidence: 0.99,
        },
        ParsedChatPayload {
            name: analysis_chat_name,
            platform,
            chat_type: analysis_chat_type,
            members: members.into_values().collect(),
            messages,
        },
        stats,
    ))
}

async fn detect_format_with_analysis(file_path: &str) -> Option<DetectedFormat> {
    parse_with_analysis_registry(file_path)
        .await
        .map(|(detected, _, _)| detected)
}

async fn parse_import_file(
    file_path: &str,
    chat_index: Option<usize>,
) -> Result<(DetectedFormat, ParsedChatPayload, ImportParseStats), ApiError> {
    if chat_index.is_none() {
        if let Some(parsed) = parse_with_analysis_registry(file_path).await {
            return Ok(parsed);
        }
    }

    let bytes = tokio::fs::read(file_path)
        .await
        .map_err(|_| ApiError::InvalidRequest("error.file_not_found".to_string()))?;
    let detected = detect_format_from_bytes(file_path, &bytes);
    let text = String::from_utf8_lossy(&bytes);
    let mut stats = ImportParseStats::default();

    let payload = if detected.id == "text" {
        parse_chat_payload_from_text(file_path, &text, &detected, &mut stats)
    } else if detected.id == "jsonl" {
        parse_chat_payload_from_jsonl(file_path, &text, &detected, &mut stats)
    } else {
        let json_value = serde_json::from_slice::<serde_json::Value>(&bytes)
            .map_err(|_| ApiError::InvalidRequest("error.unrecognized_format".to_string()))?;
        parse_chat_payload_from_json(file_path, &json_value, &detected, chat_index, &mut stats)?
    };

    Ok((detected, payload, stats))
}

fn import_diagnostics_json(detected_format: &str, stats: &ImportParseStats) -> serde_json::Value {
    serde_json::json!({
        "logFile": null,
        "detectedFormat": detected_format,
        "messagesReceived": stats.messages_received,
        "messagesWritten": stats.messages_written,
        "messagesSkipped": stats.messages_skipped,
        "skipReasons": {
            "noSenderId": stats.skip_reasons.no_sender_id,
            "noAccountName": stats.skip_reasons.no_account_name,
            "invalidTimestamp": stats.skip_reasons.invalid_timestamp,
            "noType": stats.skip_reasons.no_type,
        }
    })
}

fn normalized_content_for_signature(content: Option<&str>) -> String {
    content.unwrap_or_default().trim().replace('\n', " ")
}

fn normalized_platform_message_id_for_signature(platform_message_id: Option<&str>) -> String {
    platform_message_id.unwrap_or_default().trim().to_string()
}

fn signature_by_platform_with_message_id(
    sender_platform_id: &str,
    ts: i64,
    msg_type: i64,
    content: Option<&str>,
    platform_message_id: Option<&str>,
) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        sender_platform_id,
        ts,
        msg_type,
        normalized_content_for_signature(content),
        normalized_platform_message_id_for_signature(platform_message_id)
    )
}

fn signature_by_sender_id_with_message_id(
    sender_id: i64,
    ts: i64,
    msg_type: i64,
    content: Option<&str>,
    platform_message_id: Option<&str>,
) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        sender_id,
        ts,
        msg_type,
        normalized_content_for_signature(content),
        normalized_platform_message_id_for_signature(platform_message_id)
    )
}

#[derive(Debug, Clone)]
struct SourceCheckpointFingerprint {
    file_size: i64,
    modified_at: i64,
    fingerprint: String,
}

fn build_source_checkpoint_fingerprint(
    file_path: &str,
) -> Result<SourceCheckpointFingerprint, ApiError> {
    use std::io::Read;
    use std::time::UNIX_EPOCH;

    let path = FsPath::new(file_path);
    let meta = fs::metadata(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ApiError::InvalidRequest("error.file_not_found".to_string())
        } else {
            ApiError::Io(e)
        }
    })?;
    let file_size = i64::try_from(meta.len()).unwrap_or(i64::MAX);
    let modified = meta
        .modified()
        .ok()
        .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok());
    let modified_at = modified.map(|v| v.as_secs() as i64).unwrap_or(0);
    let modified_nanos = modified.map(|v| v.subsec_nanos()).unwrap_or(0);

    let mut file = fs::File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ApiError::InvalidRequest("error.file_not_found".to_string())
        } else {
            ApiError::Io(e)
        }
    })?;
    let mut buffer = vec![0u8; 256 * 1024];
    let mut hash_state: u64 = 0xcbf29ce484222325;
    loop {
        let read = file.read(&mut buffer).map_err(ApiError::Io)?;
        if read == 0 {
            break;
        }
        for byte in &buffer[..read] {
            hash_state ^= u64::from(*byte);
            hash_state = hash_state.wrapping_mul(0x100000001b3);
        }
    }
    let content_hash = format!("{:016x}", hash_state);
    let fingerprint = format!(
        "v2:{}:{}:{}:{}",
        file_size, modified_at, modified_nanos, content_hash
    );

    Ok(SourceCheckpointFingerprint {
        file_size,
        modified_at,
        fingerprint,
    })
}

fn usize_to_i64_saturating(value: usize) -> i64 {
    value.min(i64::MAX as usize) as i64
}

fn checkpoint_meta_json(source_fingerprint: &SourceCheckpointFingerprint) -> serde_json::Value {
    serde_json::json!({
        "fingerprint": source_fingerprint.fingerprint,
        "fileSize": source_fingerprint.file_size,
        "modifiedAt": source_fingerprint.modified_at
    })
}

fn checkpoint_state_json(checkpoint: Option<ImportSourceCheckpoint>) -> serde_json::Value {
    match checkpoint {
        Some(cp) => serde_json::json!({
            "status": cp.status,
            "lastInsertedMessages": cp.last_inserted_messages,
            "lastDuplicateMessages": cp.last_duplicate_messages,
            "lastProcessedAt": cp.last_processed_at,
            "errorMessage": cp.error_message
        }),
        None => serde_json::Value::Null,
    }
}

async fn upsert_source_checkpoint(
    repo: &crate::database::Repository,
    source_kind: &str,
    source_path: &str,
    source_fingerprint: &SourceCheckpointFingerprint,
    platform: Option<&str>,
    chat_name: Option<&str>,
    meta_id: Option<i64>,
    inserted: i64,
    duplicates: i64,
    status: &str,
    error_message: Option<String>,
) -> Result<(), ApiError> {
    repo.upsert_import_source_checkpoint(&ImportSourceCheckpoint {
        id: 0,
        source_kind: source_kind.to_string(),
        source_path: source_path.to_string(),
        fingerprint: source_fingerprint.fingerprint.clone(),
        file_size: source_fingerprint.file_size,
        modified_at: source_fingerprint.modified_at,
        platform: platform.map(|v| v.to_string()),
        chat_name: chat_name.map(|v| v.to_string()),
        meta_id,
        last_processed_at: now_ts(),
        last_inserted_messages: inserted,
        last_duplicate_messages: duplicates,
        status: status.to_string(),
        error_message,
    })
    .await
    .map_err(|e| ApiError::Database(e.to_string()))
}

fn count_keyword_occurrences(content: &str, keyword: &str) -> i64 {
    if keyword.trim().is_empty() {
        return 0;
    }
    let content_lower = content.to_lowercase();
    let keyword_lower = keyword.to_lowercase();
    content_lower.match_indices(&keyword_lower).count() as i64
}

fn collect_numeric_values(input: &serde_json::Value) -> Vec<f64> {
    match input {
        serde_json::Value::Number(n) => n.as_f64().into_iter().collect(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .flat_map(collect_numeric_values)
            .collect::<Vec<_>>(),
        serde_json::Value::Object(obj) => {
            if let Some(items) = obj
                .get("items")
                .or_else(|| obj.get("data"))
                .or_else(|| obj.get("values"))
            {
                collect_numeric_values(items)
            } else {
                obj.values().flat_map(collect_numeric_values).collect()
            }
        }
        _ => Vec::new(),
    }
}

async fn run_import_with_chat_index(
    file_path: &str,
    chat_index: Option<usize>,
) -> Result<serde_json::Value, ApiError> {
    if file_path.trim().is_empty() {
        return Ok(serde_json::json!({
            "success": false,
            "error": "error.no_file_selected"
        }));
    }

    if tokio::fs::metadata(file_path).await.is_err() {
        return Ok(serde_json::json!({
            "success": false,
            "error": "error.file_not_found"
        }));
    }
    let source_fingerprint = build_source_checkpoint_fingerprint(file_path)?;
    let source_kind = "api-import";

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let repo = crate::database::Repository::new(pool.clone());

    let started_at = now_ts();
    let progress_id = repo
        .create_import_progress(&ImportProgress {
            id: 0,
            file_path: file_path.to_string(),
            total_messages: Some(0),
            processed_messages: Some(0),
            status: Some("detecting".to_string()),
            started_at: Some(started_at),
            completed_at: None,
            error_message: None,
        })
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let parse_result = parse_import_file(file_path, chat_index).await;
    let (detected, payload, mut stats) = match parse_result {
        Ok(v) => v,
        Err(e) => {
            let _ = repo.fail_import(progress_id, &e.to_string()).await;
            let _ = upsert_source_checkpoint(
                &repo,
                source_kind,
                file_path,
                &source_fingerprint,
                None,
                None,
                None,
                0,
                0,
                "failed",
                Some(e.to_string()),
            )
            .await;
            return Ok(serde_json::json!({
                "success": false,
                "error": "error.unrecognized_format"
            }));
        }
    };

    if payload.messages.is_empty() {
        let _ = repo.fail_import(progress_id, "no messages parsed").await;
        let _ = upsert_source_checkpoint(
            &repo,
            source_kind,
            file_path,
            &source_fingerprint,
            Some(payload.platform.as_str()),
            Some(payload.name.as_str()),
            None,
            0,
            0,
            "failed",
            Some("no messages parsed".to_string()),
        )
        .await;
        return Ok(serde_json::json!({
            "success": false,
            "error": "error.no_messages",
            "diagnosis": {
                "suggestion": "error.no_messages"
            },
            "diagnostics": import_diagnostics_json(&detected.id, &stats)
        }));
    }

    let _ =
        sqlx::query("UPDATE import_progress SET total_messages = ?2, status = ?3 WHERE id = ?1")
            .bind(progress_id)
            .bind(payload.messages.len() as i32)
            .bind("saving")
            .execute(&*pool)
            .await;
    let payload_platform = payload.platform.clone();
    let payload_name = payload.name.clone();
    let payload_members = payload.members;
    let payload_messages = payload.messages;

    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: payload_name.clone(),
            platform: payload_platform.clone(),
            chat_type: payload.chat_type.clone(),
            imported_at: started_at,
            group_id: None,
            group_avatar: None,
            owner_id: None,
            schema_version: 3,
            session_gap_threshold: 1800,
        })
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let webhook_runtime = read_api_webhook_config();
    let webhook_items = webhook_runtime.rules;
    let webhook_dispatch = webhook_runtime.dispatch;
    let webhook_client = if webhook_items.is_empty() {
        None
    } else {
        Some(
            reqwest::Client::builder()
                .timeout(Duration::from_millis(webhook_dispatch.request_timeout_ms))
                .build()
                .map_err(|e| ApiError::Http(e.to_string()))?,
        )
    };
    let mut webhook_stats = WebhookDispatchStats::default();
    let mut webhook_queue: Vec<WebhookMessageCreatedEvent> = Vec::new();
    let mut webhook_queue_first_enqueued_at: Option<Instant> = None;

    let mut processed: i32 = 0;
    let write_result = async {
        let mut member_cache: HashMap<String, i64> = HashMap::new();
        for member in &payload_members {
            let member_id = ensure_member_profile_and_history(&repo, member, started_at).await?;
            member_cache.insert(member.platform_id.clone(), member_id);
        }

        for msg in payload_messages {
            let sender_id = if let Some(existing) = member_cache.get(&msg.sender_platform_id) {
                *existing
            } else {
                let profile = ParsedMemberProfile {
                    platform_id: msg.sender_platform_id.clone(),
                    account_name: msg.sender_name.clone(),
                    group_nickname: msg.sender_name.clone(),
                };
                let created = ensure_member_profile_and_history(&repo, &profile, msg.ts).await?;
                member_cache.insert(profile.platform_id, created);
                created
            };

            let inserted_message_id = repo
                .create_message(&Message {
                    id: 0,
                    sender_id,
                    sender_account_name: msg.sender_name.clone(),
                    sender_group_nickname: msg.sender_name.clone(),
                    ts: msg.ts,
                    msg_type: msg.msg_type,
                    content: msg.content.clone(),
                    reply_to_message_id: None,
                    platform_message_id: msg.platform_message_id.clone(),
                    meta_id,
                })
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?;

            if let Some(client) = webhook_client.as_ref() {
                let event = WebhookMessageCreatedEvent {
                    event_type: "message.created".to_string(),
                    platform: payload_platform.clone(),
                    chat_name: payload_name.clone(),
                    meta_id,
                    message_id: inserted_message_id,
                    sender_id,
                    sender_name: msg.sender_name.clone(),
                    ts: msg.ts,
                    msg_type: msg.msg_type,
                    content: msg.content.clone(),
                };
                if webhook_queue.is_empty() {
                    webhook_queue_first_enqueued_at = Some(Instant::now());
                }
                webhook_queue.push(event);
                let queue_age = webhook_queue_first_enqueued_at.map(|t| t.elapsed());
                if should_flush_webhook_queue(webhook_queue.len(), queue_age, &webhook_dispatch) {
                    let stats = dispatch_api_webhook_batch(
                        client,
                        &webhook_items,
                        &mut webhook_queue,
                        &webhook_dispatch,
                    )
                    .await;
                    merge_webhook_dispatch_stats(&mut webhook_stats, &stats);
                    webhook_queue_first_enqueued_at = None;
                }
            }

            processed += 1;
            if processed % 200 == 0 {
                let _ = repo.update_progress(progress_id, processed, "saving").await;
            }
        }

        if let Some(client) = webhook_client.as_ref() {
            let stats = dispatch_api_webhook_batch(
                client,
                &webhook_items,
                &mut webhook_queue,
                &webhook_dispatch,
            )
            .await;
            merge_webhook_dispatch_stats(&mut webhook_stats, &stats);
            webhook_queue_first_enqueued_at = None;
        }

        Ok::<(), ApiError>(())
    }
    .await;

    if let Err(err) = write_result {
        let _ = repo.update_progress(progress_id, processed, "failed").await;
        let _ = repo.fail_import(progress_id, &err.to_string()).await;
        let _ = upsert_source_checkpoint(
            &repo,
            source_kind,
            file_path,
            &source_fingerprint,
            Some(payload_platform.as_str()),
            Some(payload_name.as_str()),
            Some(meta_id),
            i64::from(processed.max(0)),
            0,
            "failed",
            Some(err.to_string()),
        )
        .await;
        return Err(err);
    }

    stats.messages_written = processed as usize;
    if stats.messages_received >= stats.messages_written {
        stats.messages_skipped = stats.messages_received - stats.messages_written;
    }
    let _ = repo.complete_import(progress_id, now_ts()).await;
    let _ = upsert_source_checkpoint(
        &repo,
        source_kind,
        file_path,
        &source_fingerprint,
        Some(payload_platform.as_str()),
        Some(payload_name.as_str()),
        Some(meta_id),
        i64::from(processed.max(0)),
        0,
        "completed",
        None,
    )
    .await;

    Ok(serde_json::json!({
        "success": true,
        "sessionId": meta_id.to_string(),
        "detectedPlatform": detected.platform,
        "payloadPlatform": payload_platform,
        "sessionName": payload_name,
        "diagnostics": import_diagnostics_json(&detected.id, &stats),
        "webhookSummary": webhook_stats
    }))
}

// ==================== Handler Implementations ====================

fn migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

fn migration_versions_from_disk() -> Result<Vec<i64>, ApiError> {
    let dir = migrations_dir();
    let entries = fs::read_dir(&dir).map_err(|e| {
        ApiError::Internal(format!(
            "failed to read migrations directory '{}': {}",
            dir.display(),
            e
        ))
    })?;

    let mut versions: Vec<i64> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with(".sql"))
        .filter_map(|name| {
            let prefix = name.split('_').next().unwrap_or_default();
            prefix.parse::<i64>().ok()
        })
        .collect();

    versions.sort_unstable();
    versions.dedup();
    Ok(versions)
}

async fn open_migration_pool(
    db_path: &FsPath,
    create_if_missing: bool,
) -> Result<SqlitePool, ApiError> {
    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(create_if_missing)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))
}

async fn current_migration_version(pool: &SqlitePool) -> Result<i64, ApiError> {
    let sqlx_table_exists = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name = ?1 LIMIT 1",
    )
    .bind("_sqlx_migrations")
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?
    .is_some();

    if sqlx_table_exists {
        return sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success = 1",
        )
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()));
    }

    let legacy_table_exists = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name = ?1 LIMIT 1",
    )
    .bind("schema_migrations")
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?
    .is_some();

    if legacy_table_exists {
        return sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        )
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()));
    }

    Ok(0)
}

#[instrument]
async fn check_migration() -> Result<Json<serde_json::Value>, ApiError> {
    let available_versions = migration_versions_from_disk()?;
    let target_version = available_versions.last().copied().unwrap_or(0);
    let db_path = crate::database::get_db_path();
    let current_version = if db_path.exists() {
        let pool = open_migration_pool(&db_path, false).await?;
        current_migration_version(&pool).await?
    } else {
        0
    };
    let needs_migration = current_version < target_version;

    Ok(Json(serde_json::json!({
        "needsMigration": needs_migration,
        "currentVersion": current_version,
        "targetVersion": target_version,
        "dbPath": db_path.to_string_lossy().to_string(),
        "migrationCount": available_versions.len()
    })))
}

#[instrument]
async fn run_migration() -> Result<Json<serde_json::Value>, ApiError> {
    let available_versions = migration_versions_from_disk()?;
    let target_version = available_versions.last().copied().unwrap_or(0);
    let db_path = crate::database::get_db_path();

    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(ApiError::Io)?;
    }

    let pool = open_migration_pool(&db_path, true).await?;
    let migrations = Migrator::new(FsPath::new(&migrations_dir()))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    migrations
        .run(&pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let current_version = current_migration_version(&pool).await?;
    let needs_migration = current_version < target_version;

    Ok(Json(serde_json::json!({
        "success": !needs_migration,
        "needsMigration": needs_migration,
        "currentVersion": current_version,
        "targetVersion": target_version,
        "dbPath": db_path.to_string_lossy().to_string()
    })))
}

#[instrument]
async fn select_file() -> Result<Json<serde_json::Value>, ApiError> {
    // In a real implementation, this would open a file dialog
    // For HTTP API, the frontend sends the file path directly
    Ok(Json(serde_json::json!({
        "message": "Use /import endpoint with file path"
    })))
}

#[derive(Debug, Deserialize)]
struct ImportRequest {
    file_path: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportBatchRequest {
    file_paths: Vec<String>,
    merge: Option<bool>,
    merged_session_name: Option<String>,
    retry_failed: Option<bool>,
    max_retries: Option<u32>,
}

#[derive(Debug, Clone)]
struct ParsedBatchSource {
    source_path: String,
    source_fingerprint: SourceCheckpointFingerprint,
    platform: String,
    chat_name: String,
    messages: Vec<ParsedMessage>,
}

#[instrument]
async fn import(Json(req): Json<ImportRequest>) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        run_import_with_chat_index(&req.file_path, None).await?,
    ))
}

#[instrument]
async fn import_batch(
    Json(req): Json<ImportBatchRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.file_paths.is_empty() {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "error.no_files"
        })));
    }

    if req.merge.unwrap_or(false) {
        return Ok(Json(
            run_merged_import_batch(&req.file_paths, req.merged_session_name.as_deref()).await?,
        ));
    }

    Ok(Json(
        run_separate_import_batch(
            &req.file_paths,
            req.retry_failed.unwrap_or(true),
            req.max_retries.unwrap_or(1),
        )
        .await?,
    ))
}

async fn run_separate_import_batch(
    file_paths: &[String],
    retry_failed: bool,
    max_retries: u32,
) -> Result<serde_json::Value, ApiError> {
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let repo = crate::database::Repository::new(pool);
    let mut imported_files = 0usize;
    let mut failed_files = 0usize;
    let mut skipped_files = 0usize;
    let mut items = Vec::with_capacity(file_paths.len());
    let source_kind = "api-import-batch-separate";
    let mut seen_input_paths: HashSet<String> = HashSet::new();

    for file_path in file_paths {
        if !seen_input_paths.insert(file_path.clone()) {
            skipped_files = skipped_files.saturating_add(1);
            items.push(serde_json::json!({
                "filePath": file_path,
                "checkpointSkipped": false,
                "duplicateInputSkipped": true,
                "attemptsUsed": 0,
                "result": {
                    "success": true,
                    "duplicateInputSkipped": true
                }
            }));
            continue;
        }

        let source_fingerprint = match build_source_checkpoint_fingerprint(file_path) {
            Ok(v) => v,
            Err(err) => {
                failed_files = failed_files.saturating_add(1);
                items.push(serde_json::json!({
                    "filePath": file_path,
                    "checkpointSkipped": false,
                    "attemptsUsed": 0,
                    "result": {
                        "success": false,
                        "error": format!("{err}")
                    }
                }));
                continue;
            }
        };

        let unchanged = repo
            .source_checkpoint_is_unchanged(
                source_kind,
                file_path.as_str(),
                source_fingerprint.fingerprint.as_str(),
            )
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
        if unchanged {
            skipped_files = skipped_files.saturating_add(1);
            items.push(serde_json::json!({
                "filePath": file_path,
                "checkpointSkipped": true,
                "attemptsUsed": 0,
                "result": {
                    "success": true,
                    "checkpointSkipped": true
                }
            }));
            continue;
        }

        let max_attempts = if retry_failed {
            max_retries.saturating_add(1)
        } else {
            1
        }
        .max(1);
        let mut attempts_used: u32 = 0;
        let mut final_result = serde_json::json!({
            "success": false,
            "error": "error.import_failed"
        });
        let mut success = false;

        while attempts_used < max_attempts {
            attempts_used = attempts_used.saturating_add(1);
            match run_import_with_chat_index(file_path, None).await {
                Ok(result) => {
                    let ok = result
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    final_result = result;
                    if ok {
                        success = true;
                        break;
                    }
                }
                Err(err) => {
                    final_result = serde_json::json!({
                        "success": false,
                        "error": err.to_string()
                    });
                }
            }
            if attempts_used < max_attempts {
                let backoff_ms = 150_u64.saturating_mul(u64::from(attempts_used));
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
            }
        }

        if success {
            imported_files = imported_files.saturating_add(1);
            let session_id = final_result
                .get("sessionId")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i64>().ok());
            let (checkpoint_platform, checkpoint_chat_name) = if let Some(meta_id) = session_id {
                match repo.get_chat(meta_id).await {
                    Ok(Some(chat_meta)) => (
                        Some(chat_meta.platform.to_string()),
                        Some(chat_meta.name.to_string()),
                    ),
                    _ => (None, None),
                }
            } else {
                (None, None)
            };
            let inserted = final_result
                .get("diagnostics")
                .and_then(|v| v.get("messagesWritten"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let duplicates = final_result
                .get("diagnostics")
                .and_then(|v| v.get("messagesSkipped"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let _ = upsert_source_checkpoint(
                &repo,
                source_kind,
                file_path.as_str(),
                &source_fingerprint,
                checkpoint_platform.as_deref(),
                checkpoint_chat_name.as_deref(),
                session_id,
                inserted,
                duplicates,
                "completed",
                None,
            )
            .await;
        } else {
            failed_files = failed_files.saturating_add(1);
            let error_message = final_result
                .get("error")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "error.import_failed".to_string());
            let platform_hint = infer_platform_from_path(file_path);
            let _ = upsert_source_checkpoint(
                &repo,
                source_kind,
                file_path.as_str(),
                &source_fingerprint,
                Some(platform_hint.as_str()),
                None,
                None,
                0,
                0,
                "failed",
                Some(error_message),
            )
            .await;
        }

        items.push(serde_json::json!({
            "filePath": file_path,
            "checkpointSkipped": false,
            "attemptsUsed": attempts_used,
            "result": final_result
        }));
    }

    Ok(serde_json::json!({
        "success": imported_files > 0 || skipped_files > 0,
        "mode": "separate",
        "totalFiles": file_paths.len(),
        "importedFiles": imported_files,
        "failedFiles": failed_files,
        "skippedFiles": skipped_files,
        "items": items
    }))
}

async fn run_merged_import_batch(
    file_paths: &[String],
    merged_session_name: Option<&str>,
) -> Result<serde_json::Value, ApiError> {
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let repo = crate::database::Repository::new(pool.clone());

    let mut parsed_sources: Vec<ParsedBatchSource> = Vec::new();
    let mut platform_votes: HashMap<String, usize> = HashMap::new();
    let mut group_chat_count = 0usize;
    let mut private_chat_count = 0usize;
    let mut source_results: Vec<serde_json::Value> = Vec::with_capacity(file_paths.len());
    let mut failed_files = 0usize;
    let mut skipped_files = 0usize;
    let mut seen_input_paths: HashSet<String> = HashSet::new();

    for file_path in file_paths {
        if !seen_input_paths.insert(file_path.clone()) {
            skipped_files = skipped_files.saturating_add(1);
            source_results.push(serde_json::json!({
                "filePath": file_path,
                "success": true,
                "duplicateInputSkipped": true
            }));
            continue;
        }

        let source_fingerprint = match build_source_checkpoint_fingerprint(file_path) {
            Ok(fp) => fp,
            Err(_) => {
                failed_files = failed_files.saturating_add(1);
                source_results.push(serde_json::json!({
                    "filePath": file_path,
                    "success": false,
                    "error": "error.file_not_found"
                }));
                continue;
            }
        };

        let unchanged = repo
            .source_checkpoint_is_unchanged(
                "api-import-batch-merged",
                file_path.as_str(),
                source_fingerprint.fingerprint.as_str(),
            )
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
        if unchanged {
            skipped_files = skipped_files.saturating_add(1);
            source_results.push(serde_json::json!({
                "filePath": file_path,
                "success": true,
                "checkpointSkipped": true
            }));
            continue;
        }

        match parse_import_file(file_path, None).await {
            Ok((_detected, payload, _stats)) => {
                if payload.messages.is_empty() {
                    failed_files = failed_files.saturating_add(1);
                    let platform_hint = infer_platform_from_path(file_path);
                    let _ = upsert_source_checkpoint(
                        &repo,
                        "api-import-batch-merged",
                        file_path.as_str(),
                        &source_fingerprint,
                        Some(platform_hint.as_str()),
                        None,
                        None,
                        0,
                        0,
                        "failed",
                        Some("error.no_messages".to_string()),
                    )
                    .await;
                    source_results.push(serde_json::json!({
                        "filePath": file_path,
                        "success": false,
                        "error": "error.no_messages"
                    }));
                    continue;
                }
                *platform_votes.entry(payload.platform.clone()).or_insert(0) += 1;
                if payload.chat_type.eq_ignore_ascii_case("private") {
                    private_chat_count = private_chat_count.saturating_add(1);
                } else {
                    group_chat_count = group_chat_count.saturating_add(1);
                }
                parsed_sources.push(ParsedBatchSource {
                    source_path: file_path.clone(),
                    source_fingerprint,
                    platform: payload.platform.clone(),
                    chat_name: payload.name.clone(),
                    messages: payload.messages,
                });
            }
            Err(_) => {
                failed_files = failed_files.saturating_add(1);
                let platform_hint = infer_platform_from_path(file_path);
                let _ = upsert_source_checkpoint(
                    &repo,
                    "api-import-batch-merged",
                    file_path.as_str(),
                    &source_fingerprint,
                    Some(platform_hint.as_str()),
                    None,
                    None,
                    0,
                    0,
                    "failed",
                    Some("error.unrecognized_format".to_string()),
                )
                .await;
                source_results.push(serde_json::json!({
                    "filePath": file_path,
                    "success": false,
                    "error": "error.unrecognized_format"
                }));
            }
        }
    }

    for source in &mut parsed_sources {
        source.messages.sort_by(|a, b| {
            a.ts.cmp(&b.ts)
                .then_with(|| a.sender_platform_id.cmp(&b.sender_platform_id))
                .then_with(|| a.msg_type.cmp(&b.msg_type))
                .then_with(|| {
                    a.content
                        .as_deref()
                        .unwrap_or_default()
                        .cmp(b.content.as_deref().unwrap_or_default())
                })
                .then_with(|| {
                    a.platform_message_id
                        .as_deref()
                        .unwrap_or_default()
                        .cmp(b.platform_message_id.as_deref().unwrap_or_default())
                })
        });
    }
    parsed_sources.sort_by(|a, b| {
        let a_min_ts = a.messages.first().map(|m| m.ts).unwrap_or(i64::MAX);
        let b_min_ts = b.messages.first().map(|m| m.ts).unwrap_or(i64::MAX);
        a_min_ts
            .cmp(&b_min_ts)
            .then_with(|| a.source_path.cmp(&b.source_path))
    });

    if parsed_sources.is_empty() {
        return Ok(serde_json::json!({
            "success": skipped_files > 0,
            "mode": "merged",
            "checkpointOnly": skipped_files > 0,
            "totalFiles": file_paths.len(),
            "mergedSessionId": serde_json::Value::Null,
            "importedFiles": 0,
            "failedFiles": failed_files,
            "skippedFiles": skipped_files,
            "items": source_results
        }));
    }

    let merged_platform = platform_votes
        .into_iter()
        .max_by_key(|(_, cnt)| *cnt)
        .map(|(platform, _)| platform)
        .unwrap_or_else(|| "generic".to_string());
    let merged_chat_type = if private_chat_count > group_chat_count {
        "private".to_string()
    } else {
        "group".to_string()
    };
    let merged_name = merged_session_name
        .map(|v| v.to_string())
        .unwrap_or_else(|| format!("Merged Import ({})", merged_platform));

    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: merged_name.clone(),
            platform: merged_platform.clone(),
            chat_type: merged_chat_type,
            imported_at: now_ts(),
            group_id: None,
            group_avatar: None,
            owner_id: None,
            schema_version: 3,
            session_gap_threshold: 1800,
        })
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let mut merged_seen: HashSet<String> = HashSet::new();
    let mut imported_files = 0usize;
    let mut total_inserted = 0usize;
    let mut total_duplicates = 0usize;

    for source in parsed_sources {
        let mut source_inserted = 0usize;
        let mut source_duplicates = 0usize;
        let mut source_failed = false;
        let mut source_error: Option<String> = None;

        for msg in source.messages {
            let sender_id = match repo
                .get_or_create_member(&msg.sender_platform_id, msg.sender_name.as_deref())
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    source_failed = true;
                    source_error = Some(e.to_string());
                    break;
                }
            };

            let signature = signature_by_sender_id_with_message_id(
                sender_id,
                msg.ts,
                msg.msg_type,
                msg.content.as_deref(),
                msg.platform_message_id.as_deref(),
            );
            if merged_seen.contains(&signature) {
                source_duplicates = source_duplicates.saturating_add(1);
                total_duplicates = total_duplicates.saturating_add(1);
                continue;
            }

            let row = Message {
                id: 0,
                sender_id,
                sender_account_name: msg.sender_name.clone(),
                sender_group_nickname: msg.sender_name.clone(),
                ts: msg.ts,
                msg_type: msg.msg_type,
                content: msg.content.clone(),
                reply_to_message_id: None,
                platform_message_id: msg.platform_message_id.clone(),
                meta_id,
            };
            if let Err(e) = repo.create_message(&row).await {
                source_failed = true;
                source_error = Some(e.to_string());
                break;
            }
            merged_seen.insert(signature);
            source_inserted = source_inserted.saturating_add(1);
            total_inserted = total_inserted.saturating_add(1);
        }

        let checkpoint_status = if source_failed { "failed" } else { "completed" };
        let _ = upsert_source_checkpoint(
            &repo,
            "api-import-batch-merged",
            source.source_path.as_str(),
            &source.source_fingerprint,
            Some(source.platform.as_str()),
            Some(source.chat_name.as_str()),
            Some(meta_id),
            usize_to_i64_saturating(source_inserted),
            usize_to_i64_saturating(source_duplicates),
            checkpoint_status,
            source_error.clone(),
        )
        .await;

        if source_failed {
            failed_files = failed_files.saturating_add(1);
            source_results.push(serde_json::json!({
                "filePath": source.source_path,
                "success": false,
                "error": source_error.unwrap_or_else(|| "error.import_failed".to_string())
            }));
        } else {
            imported_files = imported_files.saturating_add(1);
            source_results.push(serde_json::json!({
                "filePath": source.source_path,
                "success": true,
                "insertedMessages": source_inserted,
                "duplicateMessages": source_duplicates
            }));
        }
    }

    Ok(serde_json::json!({
        "success": imported_files > 0 || skipped_files > 0,
        "mode": "merged",
        "mergedSessionId": meta_id.to_string(),
        "mergedSessionName": merged_name,
        "totalFiles": file_paths.len(),
        "importedFiles": imported_files,
        "failedFiles": failed_files,
        "skippedFiles": skipped_files,
        "totalInsertedMessages": total_inserted,
        "totalDuplicateMessages": total_duplicates,
        "items": source_results
    }))
}

#[instrument]
async fn detect_format(
    Json(req): Json<ImportRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if let Some(detected) = detect_format_with_analysis(&req.file_path).await {
        return Ok(Json(serde_json::json!({
            "format": detected.id,
            "platform": detected.platform,
            "multiChat": detected.multi_chat,
            "confidence": detected.confidence,
            "parserSource": "analysis"
        })));
    }

    let bytes = tokio::fs::read(&req.file_path)
        .await
        .map_err(|_| ApiError::InvalidRequest("error.file_not_found".to_string()))?;
    let detected = detect_format_from_bytes(&req.file_path, &bytes);
    Ok(Json(serde_json::json!({
        "format": detected.id,
        "platform": detected.platform,
        "multiChat": detected.multi_chat,
        "confidence": detected.confidence,
        "parserSource": "builtin"
    })))
}

#[derive(Debug, Deserialize)]
struct ImportWithOptionsRequest {
    file_path: String,
    format_options: HashMap<String, serde_json::Value>,
}

#[instrument]
async fn import_with_options(
    Json(req): Json<ImportWithOptionsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let chat_index = req
        .format_options
        .get("chatIndex")
        .or_else(|| req.format_options.get("chat_index"))
        .and_then(|v| {
            v.as_u64()
                .map(|n| n as usize)
                .or_else(|| v.as_i64().map(|n| n as usize))
        });
    Ok(Json(
        run_import_with_chat_index(&req.file_path, chat_index).await?,
    ))
}

#[instrument]
async fn scan_multi_chat_file(
    Json(req): Json<ImportRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let bytes = match tokio::fs::read(&req.file_path).await {
        Ok(v) => v,
        Err(_) => {
            return Ok(Json(serde_json::json!({
                "success": false,
                "error": "error.file_not_found"
            })));
        }
    };
    let value = match serde_json::from_slice::<serde_json::Value>(&bytes) {
        Ok(v) => v,
        Err(_) => {
            return Ok(Json(serde_json::json!({
                "success": false,
                "error": "error.unrecognized_format"
            })));
        }
    };

    if let Some(chats) = extract_multi_chat_items(&value) {
        return Ok(Json(serde_json::json!({
            "success": true,
            "chats": chats
        })));
    }

    Ok(Json(serde_json::json!({
        "success": false,
        "error": "error.no_multi_chat"
    })))
}

#[instrument]
pub async fn get_available_years(
    Path(session_id): Path<String>,
    Query(_filter): Query<TimeFilter>, // English engineering note.
) -> Result<Json<Vec<i64>>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);

    let years = repo
        .get_available_years(meta_id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(years))
}

#[instrument]
async fn get_sessions() -> Result<Json<Vec<AnalysisSession>>, ApiError> {
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);
    let chats = repo
        .list_chats(None, 100, 0)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let sessions: Vec<AnalysisSession> = chats
        .into_iter()
        .map(|c| AnalysisSession {
            id: c.id,
            name: c.name,
            platform: c.platform,
            chat_type: c.chat_type,
            imported_at: c.imported_at,
        })
        .collect();

    Ok(Json(sessions))
}

#[instrument]
async fn get_session(Path(session_id): Path<String>) -> Result<Json<AnalysisSession>, ApiError> {
    let id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);
    let chat = repo
        .get_chat(id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;

    Ok(Json(AnalysisSession {
        id: chat.id,
        name: chat.name,
        platform: chat.platform,
        chat_type: chat.chat_type,
        imported_at: chat.imported_at,
    }))
}

#[instrument]
async fn delete_session(Path(session_id): Path<String>) -> Result<Json<bool>, ApiError> {
    let id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);

    repo.delete_chat(id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(true))
}

#[derive(Debug, Deserialize)]
struct RenameSessionRequest {
    new_name: String,
}

#[instrument]
async fn rename_session(
    Path(session_id): Path<String>,
    Json(req): Json<RenameSessionRequest>,
) -> Result<Json<bool>, ApiError> {
    let id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);

    // Get the chat and update its name
    let mut chat = repo
        .get_chat(id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Session not found".to_string()))?;

    chat.name = req.new_name;

    // Update in database
    repo.update_chat(&chat)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(true))
}

#[instrument]
async fn get_member_activity(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<Vec<MemberActivity>>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);

    // Convert handler TimeFilter to repository TimeFilter (member_id = None)
    let repo_filter = if filter.start_ts.is_none() && filter.end_ts.is_none() {
        None
    } else {
        Some(RepoTimeFilter {
            start_ts: filter.start_ts,
            end_ts: filter.end_ts,
            member_id: None,
        })
    };

    let activities = repo
        .get_member_activity_with_filter(meta_id, repo_filter)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(activities))
}

#[instrument]
async fn get_member_name_history(
    Path((session_id, member_id)): Path<(String, i64)>,
) -> Result<Json<Vec<MemberNameHistoryResponse>>, ApiError> {
    let _meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);

    let histories = repo
        .get_member_name_history_by_member_id(member_id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    // Map to response DTO, ignoring id and member_id fields
    let response = histories
        .into_iter()
        .map(|h| MemberNameHistoryResponse {
            name_type: h.name_type,
            name: h.name,
            start_ts: h.start_ts,
            end_ts: h.end_ts,
        })
        .collect();

    Ok(Json(response))
}

#[instrument]
async fn get_hourly_activity(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<Vec<TimeActivity>>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);

    // Convert handler TimeFilter to repository TimeFilter (member_id = None)
    let repo_filter = if filter.start_ts.is_none() && filter.end_ts.is_none() {
        None
    } else {
        Some(RepoTimeFilter {
            start_ts: filter.start_ts,
            end_ts: filter.end_ts,
            member_id: None,
        })
    };

    let activities = repo
        .get_hourly_activity_with_filter(meta_id, repo_filter)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(activities))
}

#[instrument]
async fn get_daily_activity(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<Vec<TimeActivity>>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);

    // Convert handler TimeFilter to repository TimeFilter (member_id = None)
    let repo_filter = if filter.start_ts.is_none() && filter.end_ts.is_none() {
        None
    } else {
        Some(RepoTimeFilter {
            start_ts: filter.start_ts,
            end_ts: filter.end_ts,
            member_id: None,
        })
    };

    let activities = repo
        .get_daily_activity_with_filter(meta_id, repo_filter)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(activities))
}

#[instrument]
async fn get_weekday_activity(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<Vec<TimeActivity>>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);

    // Convert handler TimeFilter to repository TimeFilter (member_id = None)
    let repo_filter = if filter.start_ts.is_none() && filter.end_ts.is_none() {
        None
    } else {
        Some(RepoTimeFilter {
            start_ts: filter.start_ts,
            end_ts: filter.end_ts,
            member_id: None,
        })
    };

    let activities = repo
        .get_weekday_activity_with_filter(meta_id, repo_filter)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(activities))
}

#[instrument]
async fn get_monthly_activity(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<Vec<TimeActivity>>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);

    // Convert handler TimeFilter to repository TimeFilter (member_id = None)
    let repo_filter = if filter.start_ts.is_none() && filter.end_ts.is_none() {
        None
    } else {
        Some(RepoTimeFilter {
            start_ts: filter.start_ts,
            end_ts: filter.end_ts,
            member_id: None,
        })
    };

    let activities = repo
        .get_monthly_activity_with_filter(meta_id, repo_filter)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(activities))
}

#[instrument]
async fn get_yearly_activity(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<Vec<TimeActivity>>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);

    // Convert handler TimeFilter to repository TimeFilter (member_id = None)
    let repo_filter = if filter.start_ts.is_none() && filter.end_ts.is_none() {
        None
    } else {
        Some(RepoTimeFilter {
            start_ts: filter.start_ts,
            end_ts: filter.end_ts,
            member_id: None,
        })
    };

    let activities = repo
        .get_yearly_activity_with_filter(meta_id, repo_filter)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(activities))
}

#[instrument]
async fn get_message_length_distribution(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<MessageLengthDistributionResult>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);

    // Convert handler TimeFilter to repository TimeFilter (member_id = None)
    let repo_filter = if filter.start_ts.is_none() && filter.end_ts.is_none() {
        None
    } else {
        Some(RepoTimeFilter {
            start_ts: filter.start_ts,
            end_ts: filter.end_ts,
            member_id: None,
        })
    };

    let distribution = repo
        .get_message_length_distribution_with_filter(meta_id, repo_filter)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(distribution))
}

#[instrument]
async fn get_message_type_distribution(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<Vec<MessageTypeDistribution>>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);

    // Convert handler TimeFilter to repository TimeFilter (member_id = None)
    let repo_filter = if filter.start_ts.is_none() && filter.end_ts.is_none() {
        None
    } else {
        Some(RepoTimeFilter {
            start_ts: filter.start_ts,
            end_ts: filter.end_ts,
            member_id: None,
        })
    };

    let distribution = repo
        .get_message_type_distribution_with_filter(meta_id, repo_filter)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(distribution))
}

#[instrument]
async fn get_time_range(
    Path(session_id): Path<String>,
) -> Result<Json<Option<TimeRange>>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let repo = crate::database::Repository::new(pool);

    let time_range = repo
        .get_time_range(meta_id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(time_range))
}

#[instrument]
async fn get_db_directory() -> Result<Json<Option<String>>, ApiError> {
    let db_path = crate::database::get_db_path();
    let db_dir = db_path.parent().map(|p| p.to_string_lossy().into_owned());
    Ok(Json(db_dir))
}

#[instrument]
async fn get_supported_formats() -> Result<Json<Vec<SupportedFormat>>, ApiError> {
    let formats = vec![
        SupportedFormat {
            id: "wechat-json".to_string(),
            name: "WeChat Export".to_string(),
            extensions: vec![
                ".json".to_string(),
                ".jsonl".to_string(),
                ".txt".to_string(),
            ],
        },
        SupportedFormat {
            id: "whatsapp-native-txt".to_string(),
            name: "WhatsApp Native TXT".to_string(),
            extensions: vec![".txt".to_string()],
        },
        SupportedFormat {
            id: "line-native-txt".to_string(),
            name: "LINE Native TXT".to_string(),
            extensions: vec![".txt".to_string()],
        },
        SupportedFormat {
            id: "qq-native-txt".to_string(),
            name: "QQ Native TXT".to_string(),
            extensions: vec![".txt".to_string()],
        },
        SupportedFormat {
            id: "telegram-native-json".to_string(),
            name: "Telegram Native JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "discord-export-json".to_string(),
            name: "Discord Export JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "instagram-export-json".to_string(),
            name: "Instagram Export JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "imessage-export-json".to_string(),
            name: "iMessage Export JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "messenger-export-json".to_string(),
            name: "Messenger Export JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "kakaotalk-export-json".to_string(),
            name: "KakaoTalk Export JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "slack-export-json".to_string(),
            name: "Slack Export JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "teams-export-json".to_string(),
            name: "Microsoft Teams Export JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "signal-export-json".to_string(),
            name: "Signal Export JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "skype-export-json".to_string(),
            name: "Skype Export JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "googlechat-export-json".to_string(),
            name: "Google Chat Export JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "zoom-export-json".to_string(),
            name: "Zoom Chat Export JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "viber-export-json".to_string(),
            name: "Viber Export JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "markdown".to_string(),
            name: "Markdown".to_string(),
            extensions: vec![".md".to_string()],
        },
        SupportedFormat {
            id: "json".to_string(),
            name: "JSON".to_string(),
            extensions: vec![".json".to_string()],
        },
        SupportedFormat {
            id: "text".to_string(),
            name: "Plain Text".to_string(),
            extensions: vec![".txt".to_string()],
        },
        SupportedFormat {
            id: "pdf".to_string(),
            name: "PDF".to_string(),
            extensions: vec![".pdf".to_string()],
        },
        SupportedFormat {
            id: "image".to_string(),
            name: "Image".to_string(),
            extensions: vec![
                ".jpg".to_string(),
                ".jpeg".to_string(),
                ".png".to_string(),
                ".gif".to_string(),
                ".webp".to_string(),
            ],
        },
    ];

    Ok(Json(formats))
}

async fn ensure_chat_session_exists(
    pool: &std::sync::Arc<sqlx::SqlitePool>,
    meta_id: i64,
) -> Result<(), ApiError> {
    let repo = crate::database::Repository::new(pool.clone());
    let exists = repo
        .get_chat(meta_id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(ApiError::NotFound(format!("session {} not found", meta_id)))
    }
}

#[instrument]
async fn get_catchphrase_analysis(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    ensure_chat_session_exists(&pool, meta_id).await?;

    let repo = crate::database::Repository::new(pool);

    // Convert handler TimeFilter to repository TimeFilter (member_id = None)
    let repo_filter = if filter.start_ts.is_none() && filter.end_ts.is_none() {
        None
    } else {
        Some(RepoTimeFilter {
            start_ts: filter.start_ts,
            end_ts: filter.end_ts,
            member_id: None,
        })
    };

    let analysis = repo
        .get_catchphrase_analysis(meta_id, repo_filter)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(serde_json::to_value(analysis)?))
}

#[instrument]
async fn get_mention_analysis(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    ensure_chat_session_exists(&pool, meta_id).await?;

    let repo = crate::database::Repository::new(pool);

    // Convert handler TimeFilter to repository TimeFilter (member_id = None)
    let repo_filter = if filter.start_ts.is_none() && filter.end_ts.is_none() {
        None
    } else {
        Some(RepoTimeFilter {
            start_ts: filter.start_ts,
            end_ts: filter.end_ts,
            member_id: None,
        })
    };

    let analysis = repo
        .get_mention_analysis(meta_id, repo_filter)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(serde_json::to_value(analysis)?))
}

#[instrument]
async fn get_mention_graph(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    ensure_chat_session_exists(&pool, meta_id).await?;

    let repo = crate::database::Repository::new(pool);

    // Convert handler TimeFilter to repository TimeFilter (member_id = None)
    let repo_filter = if filter.start_ts.is_none() && filter.end_ts.is_none() {
        None
    } else {
        Some(RepoTimeFilter {
            start_ts: filter.start_ts,
            end_ts: filter.end_ts,
            member_id: None,
        })
    };

    let graph = repo
        .get_mention_graph(meta_id, repo_filter)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(serde_json::to_value(graph)?))
}

#[instrument]
async fn get_cluster_graph(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    ensure_chat_session_exists(&pool, meta_id).await?;
    let repo = crate::database::Repository::new(pool.clone());

    let repo_filter = if filter.start_ts.is_none() && filter.end_ts.is_none() {
        None
    } else {
        Some(RepoTimeFilter {
            start_ts: filter.start_ts,
            end_ts: filter.end_ts,
            member_id: None,
        })
    };

    let mention_graph = repo
        .get_mention_graph(meta_id, repo_filter)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let total_members: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT m.id)
        FROM member m
        INNER JOIN message msg ON m.id = msg.sender_id
        WHERE msg.meta_id = ?1
          AND msg.msg_type != 7
          AND COALESCE(m.account_name, '') != '系统消息'
        "#,
    )
    .bind(meta_id)
    .fetch_one(&*pool)
    .await
    .unwrap_or(0);

    let total_messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
        .bind(meta_id)
        .fetch_one(&*pool)
        .await
        .unwrap_or(0);

    let mut degree_by_name: HashMap<String, i64> = HashMap::new();
    for link in &mention_graph.links {
        *degree_by_name.entry(link.source.clone()).or_insert(0) += link.value;
        *degree_by_name.entry(link.target.clone()).or_insert(0) += link.value;
    }
    let max_degree = degree_by_name.values().copied().max().unwrap_or(0);

    // Weighted label propagation with deterministic tie-breaking.
    let mut sorted_names: Vec<String> = mention_graph
        .nodes
        .iter()
        .map(|node| node.name.clone())
        .collect();
    sorted_names.sort();
    let mut name_to_idx: HashMap<String, usize> = HashMap::with_capacity(sorted_names.len());
    for (idx, name) in sorted_names.iter().enumerate() {
        name_to_idx.insert(name.clone(), idx);
    }

    let mut weighted_adjacency: Vec<HashMap<usize, i64>> = vec![HashMap::new(); sorted_names.len()];
    for link in &mention_graph.links {
        let Some(&source_idx) = name_to_idx.get(&link.source) else {
            continue;
        };
        let Some(&target_idx) = name_to_idx.get(&link.target) else {
            continue;
        };
        if source_idx == target_idx {
            continue;
        }
        let weight = link.value.max(1);
        *weighted_adjacency[source_idx]
            .entry(target_idx)
            .or_insert(0) += weight;
        *weighted_adjacency[target_idx]
            .entry(source_idx)
            .or_insert(0) += weight;
    }

    let mut labels: Vec<usize> = (0..sorted_names.len()).collect();
    let mut node_order: Vec<usize> = (0..sorted_names.len()).collect();
    node_order.sort_by(|a, b| {
        let name_a = &sorted_names[*a];
        let name_b = &sorted_names[*b];
        let degree_a = degree_by_name.get(name_a).copied().unwrap_or(0);
        let degree_b = degree_by_name.get(name_b).copied().unwrap_or(0);
        degree_b.cmp(&degree_a).then_with(|| name_a.cmp(name_b))
    });

    let max_iterations = 24;
    let mut iterations = 0usize;
    for iter in 0..max_iterations {
        let mut changed = false;
        for &node_idx in &node_order {
            let neighbors = &weighted_adjacency[node_idx];
            if neighbors.is_empty() {
                continue;
            }

            let mut label_score: HashMap<usize, i64> = HashMap::new();
            for (neighbor_idx, weight) in neighbors {
                let neighbor_label = labels[*neighbor_idx];
                *label_score.entry(neighbor_label).or_insert(0) += *weight;
            }

            let mut best_label = labels[node_idx];
            let mut best_score = *label_score.get(&best_label).unwrap_or(&0);
            for (label, score) in label_score {
                if score > best_score || (score == best_score && label < best_label) {
                    best_score = score;
                    best_label = label;
                }
            }

            if best_label != labels[node_idx] {
                labels[node_idx] = best_label;
                changed = true;
            }
        }
        iterations = iter + 1;
        if !changed {
            break;
        }
    }

    let mut members_by_label: HashMap<usize, Vec<usize>> = HashMap::new();
    for (idx, label) in labels.iter().copied().enumerate() {
        members_by_label.entry(label).or_default().push(idx);
    }

    let mut community_groups: Vec<(usize, Vec<usize>)> = members_by_label.into_iter().collect();
    community_groups.sort_by(|a, b| {
        b.1.len().cmp(&a.1.len()).then_with(|| {
            let a_first =
                a.1.iter()
                    .map(|idx| sorted_names[*idx].as_str())
                    .min()
                    .unwrap_or("");
            let b_first =
                b.1.iter()
                    .map(|idx| sorted_names[*idx].as_str())
                    .min()
                    .unwrap_or("");
            a_first.cmp(b_first)
        })
    });

    let mut label_to_community_id: HashMap<usize, i64> = HashMap::new();
    for (offset, (label, _)) in community_groups.iter().enumerate() {
        label_to_community_id.insert(*label, offset as i64 + 1);
    }

    let mut member_to_community_id: HashMap<String, i64> = HashMap::new();
    for (name, idx) in &name_to_idx {
        let label = labels[*idx];
        let community_id = *label_to_community_id.get(&label).unwrap_or(&0);
        member_to_community_id.insert(name.clone(), community_id);
    }

    let nodes: Vec<serde_json::Value> = mention_graph
        .nodes
        .iter()
        .map(|node| {
            let degree = degree_by_name.get(&node.name).copied().unwrap_or(0);
            let normalized_degree = if max_degree > 0 {
                degree as f64 / max_degree as f64
            } else {
                0.0
            };
            let community_id = member_to_community_id.get(&node.name).copied().unwrap_or(0);
            serde_json::json!({
                "id": node.id,
                "name": node.name,
                "messageCount": node.value,
                "symbolSize": node.symbol_size,
                "degree": degree,
                "normalizedDegree": normalized_degree,
                "communityId": community_id
            })
        })
        .collect();

    let links: Vec<serde_json::Value> = mention_graph
        .links
        .iter()
        .map(|link| {
            serde_json::json!({
                "source": link.source,
                "target": link.target,
                "value": link.value,
                "rawScore": link.raw_score.unwrap_or(link.value as f64),
                "expectedScore": link.expected_score.unwrap_or(0.0),
                "coOccurrenceCount": link.co_occurrence_count.unwrap_or(link.value)
            })
        })
        .collect();

    let mut communities: Vec<serde_json::Value> = Vec::new();
    for (label, member_indices) in &community_groups {
        let community_id = label_to_community_id.get(label).copied().unwrap_or(0);
        let mut member_names: Vec<String> = member_indices
            .iter()
            .map(|idx| sorted_names[*idx].clone())
            .collect();
        member_names.sort();

        let member_set: HashSet<&str> = member_names.iter().map(String::as_str).collect();
        let mut internal_edge_weight = 0i64;
        let mut internal_edge_count = 0i64;
        let mut external_edge_weight = 0i64;
        for link in &mention_graph.links {
            let source_in = member_set.contains(link.source.as_str());
            let target_in = member_set.contains(link.target.as_str());
            if source_in && target_in {
                internal_edge_count += 1;
                internal_edge_weight += link.value;
            } else if source_in || target_in {
                external_edge_weight += link.value;
            }
        }

        let possible_edges = (member_indices.len() * member_indices.len().saturating_sub(1)) / 2;
        let density = if possible_edges > 0 {
            internal_edge_count as f64 / possible_edges as f64
        } else {
            0.0
        };

        communities.push(serde_json::json!({
            "id": community_id,
            "name": format!("Community {}", community_id),
            "size": member_indices.len() as i64,
            "members": member_names,
            "internalEdgeWeight": internal_edge_weight,
            "internalEdgeCount": internal_edge_count,
            "externalEdgeWeight": external_edge_weight,
            "density": density
        }));
    }

    Ok(Json(serde_json::json!({
        "nodes": nodes,
        "links": links,
        "maxLinkValue": mention_graph.max_link_value,
        "communities": communities,
        "stats": {
            "totalMembers": total_members,
            "totalMessages": total_messages,
            "involvedMembers": mention_graph.nodes.len() as i64,
            "edgeCount": mention_graph.links.len() as i64,
            "communityCount": communities.len() as i64,
            "algorithm": "weighted_label_propagation",
            "iterations": iterations as i64
        }
    })))
}

#[derive(Debug, Deserialize)]
struct LaughAnalysisRequest {
    keywords: Option<Vec<String>>,
}

#[instrument]
async fn get_laugh_analysis(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
    Query(keywords_req): Query<LaughAnalysisRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let mut keywords = keywords_req
        .keywords
        .unwrap_or_else(|| {
            vec![
                "哈哈".to_string(),
                "哈哈哈".to_string(),
                "233".to_string(),
                "hhh".to_string(),
                "lol".to_string(),
                "笑死".to_string(),
                "笑哭".to_string(),
                "😂".to_string(),
                "🤣".to_string(),
            ]
        })
        .into_iter()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect::<Vec<_>>();
    keywords.sort();
    keywords.dedup();

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    ensure_chat_session_exists(&pool, meta_id).await?;

    #[derive(Debug, sqlx::FromRow)]
    struct LaughRow {
        member_id: i64,
        platform_id: String,
        name: String,
        content: Option<String>,
    }

    let mut query_sql = String::from(
        r#"
        SELECT
            m.id as member_id,
            m.platform_id as platform_id,
            COALESCE(m.group_nickname, m.account_name, m.platform_id) as name,
            msg.content as content
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id = ?1
          AND msg.msg_type = 0
          AND COALESCE(m.account_name, '') != '系统消息'
        "#,
    );

    if filter.start_ts.is_some() {
        query_sql.push_str(" AND msg.ts >= ?");
    }
    if filter.end_ts.is_some() {
        query_sql.push_str(" AND msg.ts <= ?");
    }

    let mut query = sqlx::query_as::<_, LaughRow>(&query_sql).bind(meta_id);
    if let Some(start_ts) = filter.start_ts {
        query = query.bind(start_ts);
    }
    if let Some(end_ts) = filter.end_ts {
        query = query.bind(end_ts);
    }

    let rows = query
        .fetch_all(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    #[derive(Debug)]
    struct MemberLaughAgg {
        member_id: i64,
        platform_id: String,
        name: String,
        message_count: i64,
        laugh_count: i64,
        keyword_counts: HashMap<String, i64>,
    }

    let mut member_agg: HashMap<i64, MemberLaughAgg> = HashMap::new();
    let mut global_keyword_counts: HashMap<String, i64> = HashMap::new();
    let mut total_laughs: i64 = 0;
    let total_messages = rows.len() as i64;

    for row in rows {
        let entry = member_agg
            .entry(row.member_id)
            .or_insert_with(|| MemberLaughAgg {
                member_id: row.member_id,
                platform_id: row.platform_id.clone(),
                name: row.name.clone(),
                message_count: 0,
                laugh_count: 0,
                keyword_counts: HashMap::new(),
            });

        entry.message_count += 1;
        let content = row.content.unwrap_or_default();
        if content.is_empty() {
            continue;
        }

        for keyword in &keywords {
            let count = count_keyword_occurrences(&content, keyword);
            if count > 0 {
                entry.laugh_count += count;
                *entry.keyword_counts.entry(keyword.clone()).or_insert(0) += count;
                *global_keyword_counts.entry(keyword.clone()).or_insert(0) += count;
                total_laughs += count;
            }
        }
    }

    let mut rank_items: Vec<serde_json::Value> = member_agg
        .values()
        .map(|m| {
            let laugh_rate = if m.message_count > 0 {
                m.laugh_count as f64 / m.message_count as f64 * 100.0
            } else {
                0.0
            };
            let percentage = if total_laughs > 0 {
                m.laugh_count as f64 / total_laughs as f64 * 100.0
            } else {
                0.0
            };

            let mut keyword_distribution: Vec<serde_json::Value> = m
                .keyword_counts
                .iter()
                .map(|(keyword, count)| {
                    let keyword_percentage = if m.laugh_count > 0 {
                        *count as f64 / m.laugh_count as f64 * 100.0
                    } else {
                        0.0
                    };
                    serde_json::json!({
                        "keyword": keyword,
                        "count": count,
                        "percentage": keyword_percentage
                    })
                })
                .collect();
            keyword_distribution.sort_by(|a, b| {
                let ac = a.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                let bc = b.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
                bc.cmp(&ac)
            });

            serde_json::json!({
                "memberId": m.member_id,
                "platformId": m.platform_id,
                "name": m.name,
                "laughCount": m.laugh_count,
                "messageCount": m.message_count,
                "laughRate": laugh_rate,
                "percentage": percentage,
                "keywordDistribution": keyword_distribution
            })
        })
        .collect();

    let mut rank_by_count = rank_items.clone();
    rank_by_count.sort_by(|a, b| {
        let ac = a.get("laughCount").and_then(|v| v.as_i64()).unwrap_or(0);
        let bc = b.get("laughCount").and_then(|v| v.as_i64()).unwrap_or(0);
        bc.cmp(&ac)
    });

    rank_items.sort_by(|a, b| {
        let ar = a.get("laughRate").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let br = b.get("laughRate").and_then(|v| v.as_f64()).unwrap_or(0.0);
        br.total_cmp(&ar)
    });
    let rank_by_rate = rank_items;

    let mut type_distribution: Vec<serde_json::Value> = global_keyword_counts
        .iter()
        .map(|(keyword, count)| {
            let percentage = if total_laughs > 0 {
                *count as f64 / total_laughs as f64 * 100.0
            } else {
                0.0
            };
            serde_json::json!({
                "type": keyword,
                "count": count,
                "percentage": percentage
            })
        })
        .collect();
    type_distribution.sort_by(|a, b| {
        let ac = a.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
        let bc = b.get("count").and_then(|v| v.as_i64()).unwrap_or(0);
        bc.cmp(&ac)
    });

    let group_laugh_rate = if total_messages > 0 {
        total_laughs as f64 / total_messages as f64 * 100.0
    } else {
        0.0
    };

    Ok(Json(serde_json::json!({
        "rankByRate": rank_by_rate,
        "rankByCount": rank_by_count,
        "typeDistribution": type_distribution,
        "totalLaughs": total_laughs,
        "totalMessages": total_messages,
        "groupLaughRate": group_laugh_rate
    })))
}

#[instrument]
async fn get_night_owl_analysis(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    ensure_chat_session_exists(&pool, meta_id).await?;

    #[derive(Debug, sqlx::FromRow)]
    struct NightRow {
        member_id: i64,
        platform_id: String,
        name: String,
        message_count: i64,
        night_count: i64,
    }

    let mut query_sql = String::from(
        r#"
        SELECT
            m.id as member_id,
            m.platform_id as platform_id,
            COALESCE(m.group_nickname, m.account_name, m.platform_id) as name,
            COUNT(*) as message_count,
            SUM(
                CASE
                    WHEN CAST(strftime('%H', datetime(msg.ts, 'unixepoch', 'localtime')) AS INTEGER) BETWEEN 0 AND 5
                    THEN 1 ELSE 0
                END
            ) as night_count
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id = ?1
          AND msg.msg_type != 7
          AND COALESCE(m.account_name, '') != '系统消息'
        "#,
    );
    if filter.start_ts.is_some() {
        query_sql.push_str(" AND msg.ts >= ?");
    }
    if filter.end_ts.is_some() {
        query_sql.push_str(" AND msg.ts <= ?");
    }
    query_sql.push_str(
        " GROUP BY m.id, m.platform_id, COALESCE(m.group_nickname, m.account_name, m.platform_id)",
    );

    let mut query = sqlx::query_as::<_, NightRow>(&query_sql).bind(meta_id);
    if let Some(start_ts) = filter.start_ts {
        query = query.bind(start_ts);
    }
    if let Some(end_ts) = filter.end_ts {
        query = query.bind(end_ts);
    }
    let mut rows = query
        .fetch_all(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    rows.sort_by(|a, b| {
        b.night_count
            .cmp(&a.night_count)
            .then_with(|| b.message_count.cmp(&a.message_count))
            .then_with(|| a.member_id.cmp(&b.member_id))
    });

    let total_messages = rows.iter().map(|r| r.message_count).sum::<i64>();
    let total_night_messages = rows.iter().map(|r| r.night_count).sum::<i64>();
    let members = rows
        .iter()
        .enumerate()
        .map(|(idx, row)| {
            let night_ratio = if row.message_count > 0 {
                row.night_count as f64 / row.message_count as f64 * 100.0
            } else {
                0.0
            };
            let contribution = if total_night_messages > 0 {
                row.night_count as f64 / total_night_messages as f64 * 100.0
            } else {
                0.0
            };
            serde_json::json!({
                "rank": idx as i64 + 1,
                "memberId": row.member_id,
                "platformId": row.platform_id,
                "name": row.name,
                "messageCount": row.message_count,
                "nightMessageCount": row.night_count,
                "nightRatio": night_ratio,
                "nightContribution": contribution
            })
        })
        .collect::<Vec<_>>();

    let group_night_ratio = if total_messages > 0 {
        total_night_messages as f64 / total_messages as f64 * 100.0
    } else {
        0.0
    };

    Ok(Json(serde_json::json!({
        "members": members,
        "stats": {
            "totalMessages": total_messages,
            "totalNightMessages": total_night_messages,
            "groupNightRatio": group_night_ratio,
            "nightWindowHours": [0, 1, 2, 3, 4, 5]
        }
    })))
}

#[instrument]
async fn get_dragon_king_analysis(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    ensure_chat_session_exists(&pool, meta_id).await?;

    #[derive(Debug, sqlx::FromRow)]
    struct DragonRow {
        member_id: i64,
        platform_id: String,
        name: String,
        message_count: i64,
        active_days: i64,
    }

    let mut query_sql = String::from(
        r#"
        SELECT
            m.id as member_id,
            m.platform_id as platform_id,
            COALESCE(m.group_nickname, m.account_name, m.platform_id) as name,
            COUNT(*) as message_count,
            COUNT(DISTINCT date(datetime(msg.ts, 'unixepoch', 'localtime'))) as active_days
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id = ?1
          AND msg.msg_type != 7
          AND COALESCE(m.account_name, '') != '系统消息'
        "#,
    );
    if filter.start_ts.is_some() {
        query_sql.push_str(" AND msg.ts >= ?");
    }
    if filter.end_ts.is_some() {
        query_sql.push_str(" AND msg.ts <= ?");
    }
    query_sql.push_str(
        " GROUP BY m.id, m.platform_id, COALESCE(m.group_nickname, m.account_name, m.platform_id)",
    );

    let mut query = sqlx::query_as::<_, DragonRow>(&query_sql).bind(meta_id);
    if let Some(start_ts) = filter.start_ts {
        query = query.bind(start_ts);
    }
    if let Some(end_ts) = filter.end_ts {
        query = query.bind(end_ts);
    }

    let mut rows = query
        .fetch_all(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    rows.sort_by(|a, b| {
        b.message_count
            .cmp(&a.message_count)
            .then_with(|| b.active_days.cmp(&a.active_days))
            .then_with(|| a.member_id.cmp(&b.member_id))
    });

    let total_messages = rows.iter().map(|r| r.message_count).sum::<i64>();
    let leaderboard = rows
        .iter()
        .enumerate()
        .map(|(idx, row)| {
            let contribution = if total_messages > 0 {
                row.message_count as f64 / total_messages as f64 * 100.0
            } else {
                0.0
            };
            serde_json::json!({
                "rank": idx as i64 + 1,
                "memberId": row.member_id,
                "platformId": row.platform_id,
                "name": row.name,
                "messageCount": row.message_count,
                "activeDays": row.active_days,
                "contribution": contribution
            })
        })
        .collect::<Vec<_>>();

    let dragon_king = leaderboard
        .first()
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    Ok(Json(serde_json::json!({
        "dragonKing": dragon_king,
        "leaderboard": leaderboard,
        "stats": {
            "totalMessages": total_messages,
            "memberCount": rows.len() as i64
        }
    })))
}

#[instrument]
async fn get_lurker_analysis(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    ensure_chat_session_exists(&pool, meta_id).await?;

    #[derive(Debug, sqlx::FromRow)]
    struct LurkerRow {
        member_id: i64,
        platform_id: String,
        name: String,
        message_count: i64,
        last_ts: i64,
    }

    let mut query_sql = String::from(
        r#"
        SELECT
            m.id as member_id,
            m.platform_id as platform_id,
            COALESCE(m.group_nickname, m.account_name, m.platform_id) as name,
            COUNT(*) as message_count,
            MAX(msg.ts) as last_ts
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id = ?1
          AND msg.msg_type != 7
          AND COALESCE(m.account_name, '') != '系统消息'
        "#,
    );
    if filter.start_ts.is_some() {
        query_sql.push_str(" AND msg.ts >= ?");
    }
    if filter.end_ts.is_some() {
        query_sql.push_str(" AND msg.ts <= ?");
    }
    query_sql.push_str(
        " GROUP BY m.id, m.platform_id, COALESCE(m.group_nickname, m.account_name, m.platform_id)",
    );

    let mut query = sqlx::query_as::<_, LurkerRow>(&query_sql).bind(meta_id);
    if let Some(start_ts) = filter.start_ts {
        query = query.bind(start_ts);
    }
    if let Some(end_ts) = filter.end_ts {
        query = query.bind(end_ts);
    }
    let rows = query
        .fetch_all(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    if rows.is_empty() {
        return Ok(Json(serde_json::json!({
            "lurkers": [],
            "stats": {
                "memberCount": 0,
                "averageMessageCount": 0.0,
                "lurkerThreshold": 0
            }
        })));
    }

    let total_messages = rows.iter().map(|r| r.message_count).sum::<i64>();
    let average_message_count = total_messages as f64 / rows.len() as f64;
    let lurker_threshold = (average_message_count * 0.25).ceil() as i64;
    let lurker_threshold = lurker_threshold.max(1);
    let latest_ts = rows.iter().map(|r| r.last_ts).max().unwrap_or(0);

    let mut lurkers = rows
        .iter()
        .filter_map(|row| {
            let idle_days = if latest_ts > row.last_ts {
                (latest_ts - row.last_ts) / 86_400
            } else {
                0
            };
            let is_lurker = row.message_count <= lurker_threshold || idle_days >= 14;
            if !is_lurker {
                return None;
            }
            Some(serde_json::json!({
                "memberId": row.member_id,
                "platformId": row.platform_id,
                "name": row.name,
                "messageCount": row.message_count,
                "idleDays": idle_days,
                "lastActiveTs": row.last_ts,
                "relativeActivity": if average_message_count > 0.0 {
                    row.message_count as f64 / average_message_count
                } else {
                    0.0
                }
            }))
        })
        .collect::<Vec<_>>();

    lurkers.sort_by(|a, b| {
        let ac = a.get("messageCount").and_then(|v| v.as_i64()).unwrap_or(0);
        let bc = b.get("messageCount").and_then(|v| v.as_i64()).unwrap_or(0);
        let ai = a.get("idleDays").and_then(|v| v.as_i64()).unwrap_or(0);
        let bi = b.get("idleDays").and_then(|v| v.as_i64()).unwrap_or(0);
        ac.cmp(&bc).then_with(|| bi.cmp(&ai)).then_with(|| {
            let an = a.get("memberId").and_then(|v| v.as_i64()).unwrap_or(0);
            let bn = b.get("memberId").and_then(|v| v.as_i64()).unwrap_or(0);
            an.cmp(&bn)
        })
    });

    Ok(Json(serde_json::json!({
        "lurkers": lurkers,
        "stats": {
            "memberCount": rows.len() as i64,
            "totalMessages": total_messages,
            "averageMessageCount": average_message_count,
            "lurkerThreshold": lurker_threshold
        }
    })))
}

#[instrument]
async fn get_checkin_analysis(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    ensure_chat_session_exists(&pool, meta_id).await?;

    #[derive(Debug, sqlx::FromRow)]
    struct CheckinRow {
        member_id: i64,
        platform_id: String,
        name: String,
        message_count: i64,
        active_days: i64,
        first_ts: i64,
        last_ts: i64,
    }

    let mut query_sql = String::from(
        r#"
        SELECT
            m.id as member_id,
            m.platform_id as platform_id,
            COALESCE(m.group_nickname, m.account_name, m.platform_id) as name,
            COUNT(*) as message_count,
            COUNT(DISTINCT date(datetime(msg.ts, 'unixepoch', 'localtime'))) as active_days,
            MIN(msg.ts) as first_ts,
            MAX(msg.ts) as last_ts
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id = ?1
          AND msg.msg_type != 7
          AND COALESCE(m.account_name, '') != '系统消息'
        "#,
    );
    if filter.start_ts.is_some() {
        query_sql.push_str(" AND msg.ts >= ?");
    }
    if filter.end_ts.is_some() {
        query_sql.push_str(" AND msg.ts <= ?");
    }
    query_sql.push_str(
        " GROUP BY m.id, m.platform_id, COALESCE(m.group_nickname, m.account_name, m.platform_id)",
    );

    let mut query = sqlx::query_as::<_, CheckinRow>(&query_sql).bind(meta_id);
    if let Some(start_ts) = filter.start_ts {
        query = query.bind(start_ts);
    }
    if let Some(end_ts) = filter.end_ts {
        query = query.bind(end_ts);
    }
    let rows = query
        .fetch_all(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let mut leaderboard = rows
        .iter()
        .filter(|row| row.message_count >= 3)
        .map(|row| {
            let span_days = ((row.last_ts - row.first_ts) / 86_400).max(0) + 1;
            let checkin_rate = if span_days > 0 {
                row.active_days as f64 / span_days as f64 * 100.0
            } else {
                0.0
            };
            serde_json::json!({
                "memberId": row.member_id,
                "platformId": row.platform_id,
                "name": row.name,
                "messageCount": row.message_count,
                "activeDays": row.active_days,
                "spanDays": span_days,
                "checkinRate": checkin_rate,
                "firstTs": row.first_ts,
                "lastTs": row.last_ts
            })
        })
        .collect::<Vec<_>>();
    leaderboard.sort_by(|a, b| {
        let ar = a.get("checkinRate").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let br = b.get("checkinRate").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let ad = a.get("activeDays").and_then(|v| v.as_i64()).unwrap_or(0);
        let bd = b.get("activeDays").and_then(|v| v.as_i64()).unwrap_or(0);
        br.total_cmp(&ar).then_with(|| bd.cmp(&ad))
    });

    #[derive(Debug, sqlx::FromRow)]
    struct WeekdayRow {
        weekday: String,
        message_count: i64,
    }
    let mut weekday_sql = String::from(
        r#"
        SELECT
            strftime('%w', datetime(msg.ts, 'unixepoch', 'localtime')) as weekday,
            COUNT(*) as message_count
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id = ?1
          AND msg.msg_type != 7
          AND COALESCE(m.account_name, '') != '系统消息'
        "#,
    );
    if filter.start_ts.is_some() {
        weekday_sql.push_str(" AND msg.ts >= ?");
    }
    if filter.end_ts.is_some() {
        weekday_sql.push_str(" AND msg.ts <= ?");
    }
    weekday_sql.push_str(" GROUP BY strftime('%w', datetime(msg.ts, 'unixepoch', 'localtime'))");
    let mut weekday_query = sqlx::query_as::<_, WeekdayRow>(&weekday_sql).bind(meta_id);
    if let Some(start_ts) = filter.start_ts {
        weekday_query = weekday_query.bind(start_ts);
    }
    if let Some(end_ts) = filter.end_ts {
        weekday_query = weekday_query.bind(end_ts);
    }
    let weekday_rows = weekday_query
        .fetch_all(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let weekday_distribution = weekday_rows
        .into_iter()
        .map(|row| {
            let weekday = row.weekday.parse::<i64>().unwrap_or(0);
            serde_json::json!({
                "weekday": weekday,
                "messageCount": row.message_count
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(serde_json::json!({
        "leaderboard": leaderboard,
        "weekdayDistribution": weekday_distribution,
        "stats": {
            "memberCount": rows.len() as i64,
            "qualifiedMemberCount": leaderboard.len() as i64
        }
    })))
}

fn normalize_repeat_content(content: &str) -> String {
    let mut normalized = String::with_capacity(content.len());
    let mut last_space = false;
    for ch in content.trim().chars() {
        let c = if ch.is_whitespace() { ' ' } else { ch };
        if c == ' ' {
            if !last_space {
                normalized.push(c);
                last_space = true;
            }
        } else {
            normalized.extend(c.to_lowercase());
            last_space = false;
        }
    }
    normalized.trim().to_string()
}

#[instrument]
async fn get_repeat_analysis(
    Path(session_id): Path<String>,
    Query(filter): Query<TimeFilter>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    ensure_chat_session_exists(&pool, meta_id).await?;

    #[derive(Debug, sqlx::FromRow)]
    struct RepeatRow {
        id: i64,
        ts: i64,
        sender_platform_id: String,
        sender_name: String,
        content: Option<String>,
    }

    let mut query_sql = String::from(
        r#"
        SELECT
            msg.id as id,
            msg.ts as ts,
            m.platform_id as sender_platform_id,
            COALESCE(m.group_nickname, m.account_name, m.platform_id) as sender_name,
            msg.content as content
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id = ?1
          AND msg.msg_type = 0
          AND COALESCE(m.account_name, '') != '系统消息'
        "#,
    );
    if filter.start_ts.is_some() {
        query_sql.push_str(" AND msg.ts >= ?");
    }
    if filter.end_ts.is_some() {
        query_sql.push_str(" AND msg.ts <= ?");
    }
    query_sql.push_str(" ORDER BY msg.ts ASC, msg.id ASC");

    let mut query = sqlx::query_as::<_, RepeatRow>(&query_sql).bind(meta_id);
    if let Some(start_ts) = filter.start_ts {
        query = query.bind(start_ts);
    }
    if let Some(end_ts) = filter.end_ts {
        query = query.bind(end_ts);
    }
    let rows = query
        .fetch_all(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    #[derive(Debug, Default)]
    struct PhraseAgg {
        occurrences: i64,
        total_messages_in_runs: i64,
        max_chain_len: i64,
        participants: HashSet<String>,
    }

    let mut phrase_agg: HashMap<String, PhraseAgg> = HashMap::new();
    let mut runs: Vec<serde_json::Value> = Vec::new();

    let mut current_phrase = String::new();
    let mut current_start_ts = 0i64;
    let mut current_end_ts = 0i64;
    let mut current_len = 0i64;
    let mut current_last_ts = 0i64;
    let mut current_participants: HashSet<String> = HashSet::new();
    let mut current_names: HashSet<String> = HashSet::new();

    let mut finalize_current =
        |phrase: &str,
         start_ts: i64,
         end_ts: i64,
         chain_len: i64,
         participants: &HashSet<String>,
         participant_names: &HashSet<String>| {
            if phrase.is_empty() || chain_len < 2 || participants.len() < 2 {
                return;
            }
            let entry = phrase_agg.entry(phrase.to_string()).or_default();
            entry.occurrences += 1;
            entry.total_messages_in_runs += chain_len;
            entry.max_chain_len = entry.max_chain_len.max(chain_len);
            for participant in participants {
                entry.participants.insert(participant.clone());
            }
            runs.push(serde_json::json!({
                "phrase": phrase,
                "startTs": start_ts,
                "endTs": end_ts,
                "chainLength": chain_len,
                "participantCount": participants.len() as i64,
                "participants": participant_names.iter().cloned().collect::<Vec<_>>()
            }));
        };

    for row in rows.iter() {
        let raw_content = row.content.as_deref().unwrap_or_default();
        let normalized = normalize_repeat_content(raw_content);
        if normalized.len() < 2 || normalized.len() > 120 {
            finalize_current(
                &current_phrase,
                current_start_ts,
                current_end_ts,
                current_len,
                &current_participants,
                &current_names,
            );
            current_phrase.clear();
            current_participants.clear();
            current_names.clear();
            current_len = 0;
            continue;
        }

        let joins_current = !current_phrase.is_empty()
            && normalized == current_phrase
            && (row.ts - current_last_ts).abs() <= 300;
        if joins_current {
            current_len += 1;
            current_end_ts = row.ts;
            current_last_ts = row.ts;
            current_participants.insert(row.sender_platform_id.clone());
            current_names.insert(row.sender_name.clone());
            continue;
        }

        finalize_current(
            &current_phrase,
            current_start_ts,
            current_end_ts,
            current_len,
            &current_participants,
            &current_names,
        );
        current_phrase = normalized;
        current_start_ts = row.ts;
        current_end_ts = row.ts;
        current_last_ts = row.ts;
        current_len = 1;
        current_participants.clear();
        current_names.clear();
        current_participants.insert(row.sender_platform_id.clone());
        current_names.insert(row.sender_name.clone());
    }
    finalize_current(
        &current_phrase,
        current_start_ts,
        current_end_ts,
        current_len,
        &current_participants,
        &current_names,
    );

    let mut phrases = phrase_agg
        .into_iter()
        .map(|(phrase, agg)| {
            serde_json::json!({
                "phrase": phrase,
                "occurrences": agg.occurrences,
                "totalMessagesInRuns": agg.total_messages_in_runs,
                "maxChainLength": agg.max_chain_len,
                "participantCount": agg.participants.len() as i64
            })
        })
        .collect::<Vec<_>>();
    phrases.sort_by(|a, b| {
        let at = a
            .get("totalMessagesInRuns")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let bt = b
            .get("totalMessagesInRuns")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let ao = a.get("occurrences").and_then(|v| v.as_i64()).unwrap_or(0);
        let bo = b.get("occurrences").and_then(|v| v.as_i64()).unwrap_or(0);
        bt.cmp(&at).then_with(|| bo.cmp(&ao))
    });

    runs.sort_by(|a, b| {
        let al = a.get("chainLength").and_then(|v| v.as_i64()).unwrap_or(0);
        let bl = b.get("chainLength").and_then(|v| v.as_i64()).unwrap_or(0);
        bl.cmp(&al)
    });

    Ok(Json(serde_json::json!({
        "phrases": phrases,
        "runs": runs,
        "stats": {
            "scannedMessages": rows.len() as i64,
            "repeatingPhraseCount": phrases.len() as i64,
            "repeatingRunCount": runs.len() as i64
        }
    })))
}

#[instrument]
async fn get_members(
    Path(session_id): Path<String>,
) -> Result<Json<Vec<MemberResponse>>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    // English engineering note.
    let query = r#"
        SELECT DISTINCT
            m.id,
            m.platform_id,
            m.account_name,
            m.group_nickname,
            m.aliases,
            m.avatar,
            m.roles
        FROM member m
        INNER JOIN message msg ON m.id = msg.sender_id
        WHERE msg.meta_id = ?1
          AND msg.msg_type != 7
          AND COALESCE(m.account_name, '') != '系统消息'
        ORDER BY m.id
    "#;

    #[derive(Debug, sqlx::FromRow)]
    struct MemberRow {
        id: i64,
        platform_id: String,
        account_name: Option<String>,
        group_nickname: Option<String>,
        aliases: Option<String>,
        avatar: Option<String>,
        roles: Option<String>,
    }

    let rows: Vec<MemberRow> = sqlx::query_as(query)
        .bind(meta_id)
        .fetch_all(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    // English engineering note.
    let members: Vec<MemberResponse> = rows
        .into_iter()
        .map(|row| {
            let aliases = match row.aliases {
                Some(json_str) => serde_json::from_str(&json_str).unwrap_or_default(),
                None => Vec::new(),
            };

            MemberResponse {
                id: row.id,
                platform_id: row.platform_id,
                account_name: row.account_name,
                group_nickname: row.group_nickname,
                aliases,
            }
        })
        .collect();

    Ok(Json(members))
}

#[derive(Debug, Deserialize)]
struct PaginationParams {
    page: Option<u32>,
    page_size: Option<u32>,
    search: Option<String>,
    sort_order: Option<String>,
}

#[instrument]
async fn get_members_paginated(
    Path(session_id): Path<String>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<MembersPaginatedResult>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).max(1).min(100);
    let search = params.search.as_deref().unwrap_or("");
    let sort_order = params.sort_order.as_deref().unwrap_or("desc");
    let offset = (page - 1) * page_size;

    let search_condition = if search.is_empty() {
        String::new()
    } else {
        format!(
            " AND (m.group_nickname LIKE '%{}%' COLLATE NOCASE OR m.account_name LIKE '%{}%' COLLATE NOCASE OR m.platform_id LIKE '%{}%' COLLATE NOCASE OR m.aliases LIKE '%{}%' COLLATE NOCASE)",
            search, search, search, search
        )
    };

    let order_direction = if sort_order == "asc" { "ASC" } else { "DESC" };

    let count_sql = format!(
        "SELECT COUNT(*) as total FROM (
            SELECT m.id FROM member m
            INNER JOIN message msg ON m.id = msg.sender_id AND msg.meta_id = ?1
            WHERE COALESCE(m.group_nickname, m.account_name, m.platform_id) != '系统消息'
              AND msg.msg_type != 7
            {}
            GROUP BY m.id
        )",
        search_condition
    );

    let total: i64 = sqlx::query_scalar(&count_sql)
        .bind(meta_id)
        .fetch_one(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let total_pages = (total as f64 / page_size as f64).ceil() as i64;

    let data_sql = format!(
        "SELECT m.id, m.platform_id, m.account_name, m.group_nickname,
            m.aliases, m.avatar, COUNT(msg.id) as message_count
        FROM member m
        INNER JOIN message msg ON m.id = msg.sender_id AND msg.meta_id = ?1
        WHERE COALESCE(m.group_nickname, m.account_name, m.platform_id) != '系统消息'
          AND msg.msg_type != 7
        {}
        GROUP BY m.id
        ORDER BY message_count {} 
        LIMIT ?2 OFFSET ?3",
        search_condition, order_direction
    );

    #[derive(Debug, sqlx::FromRow)]
    struct MemberRow {
        id: i64,
        platform_id: String,
        account_name: Option<String>,
        group_nickname: Option<String>,
        aliases: Option<String>,
        avatar: Option<String>,
        message_count: i64,
    }

    let rows: Vec<MemberRow> = sqlx::query_as(&data_sql)
        .bind(meta_id)
        .bind(page_size as i64)
        .bind(offset as i64)
        .fetch_all(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let members: Vec<MemberWithStats> = rows
        .into_iter()
        .map(|row| {
            let aliases = match row.aliases {
                Some(json_str) => serde_json::from_str(&json_str).unwrap_or_default(),
                None => Vec::new(),
            };

            MemberWithStats {
                id: row.id,
                platform_id: row.platform_id,
                account_name: row.account_name,
                group_nickname: row.group_nickname,
                aliases,
                message_count: row.message_count,
                avatar: row.avatar,
            }
        })
        .collect();

    let result = MembersPaginatedResult {
        members,
        total,
        page: page as i64,
        page_size: page_size as i64,
        total_pages,
    };

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
struct UpdateMemberAliasesRequest {
    aliases: Vec<String>,
}

#[instrument(name = "update_member_aliases", fields(session_id = %session_id, member_id = %member_id))]
async fn update_member_aliases(
    Path((session_id, member_id)): Path<(String, i32)>,
    Json(req): Json<UpdateMemberAliasesRequest>,
) -> Result<Json<bool>, ApiError> {
    // Parse meta_id from session_id
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session_id".to_string()))?;
    // Serialize aliases to JSON
    let aliases_json = serde_json::to_string(&req.aliases)
        .map_err(|e| ApiError::InvalidRequest(format!("Invalid aliases: {}", e)))?;
    // Get DB pool
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    // Execute update with session verification (member belongs to session)
    let query = "UPDATE member SET aliases = ? WHERE id = ? AND EXISTS (SELECT 1 FROM message WHERE sender_id = ? AND meta_id = ?)";
    let result = sqlx::query(query)
        .bind(&aliases_json)
        .bind(member_id as i64)
        .bind(member_id as i64)
        .bind(meta_id)
        .execute(pool.as_ref())
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let updated = result.rows_affected() > 0;
    Ok(Json(updated))
}

#[instrument(name = "delete_member", fields(session_id = %session_id, member_id = %member_id))]
async fn delete_member(
    Path((session_id, member_id)): Path<(String, i32)>,
) -> Result<Json<bool>, ApiError> {
    // Parse meta_id from session_id
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session_id".to_string()))?;

    // Get DB pool
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let res_messages = sqlx::query(r#"DELETE FROM message WHERE sender_id = ?1 AND meta_id = ?2"#)
        .bind(member_id as i64)
        .bind(meta_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let mut total_affected: i64 = res_messages.rows_affected() as i64;

    let remaining: i64 = sqlx::query_scalar(
        r#" 
        SELECT COUNT(*) FROM message WHERE sender_id = ?1
        "#,
    )
    .bind(member_id as i64)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    if remaining == 0 {
        let res_hist = sqlx::query(r#"DELETE FROM member_name_history WHERE member_id = ?1"#)
            .bind(member_id as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
        total_affected += res_hist.rows_affected() as i64;

        let res_member = sqlx::query(r#"DELETE FROM member WHERE id = ?1"#)
            .bind(member_id as i64)
            .execute(&mut *tx)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
        total_affected += res_member.rows_affected() as i64;
    }

    tx.commit()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(total_affected > 0))
}

#[derive(Debug, Deserialize)]
struct UpdateSessionOwnerRequest {
    owner_id: Option<String>,
}

#[instrument(level = "info", skip(req))]
async fn update_session_owner_id(
    Path(session_id): Path<String>,
    Json(req): Json<UpdateSessionOwnerRequest>,
) -> Result<Json<bool>, ApiError> {
    // Obtain DB pool
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    // Parse meta_id from session_id
    let meta_id: i64 = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session_id".to_string()))?;
    // Execute update: set owner_id (nullable) for the given meta id
    let rows_affected = sqlx::query("UPDATE meta SET owner_id = ? WHERE id = ?")
        .bind(req.owner_id.as_deref())
        .bind(meta_id)
        .execute(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?
        .rows_affected();
    Ok(Json(rows_affected > 0))
}

#[derive(Debug, Deserialize)]
struct PluginQueryRequest {
    sql: String,
    params: Vec<serde_json::Value>,
}

#[instrument]
async fn plugin_query(
    Path(session_id): Path<String>,
    Json(req): Json<PluginQueryRequest>,
) -> Result<Json<Vec<serde_json::Value>>, ApiError> {
    // Parse meta_id from session_id
    let _meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    // Security: only allow SELECT statements
    let sql_text = req.sql.trim();
    let upper = sql_text.to_uppercase();
    if !upper.starts_with("SELECT") {
        return Err(ApiError::InvalidRequest(
            "Only SELECT statements are allowed".to_string(),
        ));
    }

    // Get DB pool
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    // Build and bind parameters
    let mut query = sqlx::query(sql_text);
    for p in req.params.iter() {
        match p {
            serde_json::Value::Null => {
                query = query.bind::<Option<String>>(None);
            }
            serde_json::Value::Bool(b) => {
                query = query.bind(*b);
            }
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    query = query.bind(i);
                } else if let Some(f) = n.as_f64() {
                    query = query.bind(f);
                } else {
                    query = query.bind(n.to_string());
                }
            }
            serde_json::Value::String(s) => {
                query = query.bind(s.clone());
            }
            serde_json::Value::Array(a) => {
                if let Ok(s) = serde_json::to_string(a) {
                    query = query.bind(s);
                } else {
                    query = query.bind("".to_string());
                }
            }
            serde_json::Value::Object(o) => {
                if let Ok(s) = serde_json::to_string(o) {
                    query = query.bind(s);
                } else {
                    query = query.bind("".to_string());
                }
            }
        }
    }

    // Execute query
    let rows = query
        .fetch_all(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    // Map each row to JSON object with column-name -> value mapping
    let mut results: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut obj = serde_json::Map::new();
        for (idx, col) in row.columns().iter().enumerate() {
            let name = col.name().to_string();
            let value = if let Ok(v) = row.try_get::<i64, _>(idx) {
                serde_json::Value::Number(serde_json::Number::from(v))
            } else if let Ok(v) = row.try_get::<f64, _>(idx) {
                if let Some(num) = serde_json::Number::from_f64(v) {
                    serde_json::Value::Number(num)
                } else {
                    serde_json::Value::Null
                }
            } else if let Ok(v) = row.try_get::<bool, _>(idx) {
                serde_json::Value::Bool(v)
            } else if let Ok(v) = row.try_get::<String, _>(idx) {
                serde_json::Value::String(v)
            } else if let Ok(v) = row.try_get::<Vec<u8>, _>(idx) {
                if v.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::Value::String("<binary>".to_string())
                }
            } else {
                serde_json::Value::Null
            };
            obj.insert(name, value);
        }
        results.push(serde_json::Value::Object(obj));
    }

    Ok(Json(results))
}

#[derive(Debug, Deserialize)]
struct PluginComputeRequest {
    fn_string: String,
    input: serde_json::Value,
}

#[instrument]
async fn plugin_compute(
    Json(req): Json<PluginComputeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let fn_name = req.fn_string.trim().to_ascii_lowercase();
    let result = match fn_name.as_str() {
        "identity" | "pass_through" | "return_input" => req.input,
        "count" => match &req.input {
            serde_json::Value::Array(arr) => serde_json::json!(arr.len()),
            serde_json::Value::Object(obj) => serde_json::json!(obj.len()),
            serde_json::Value::Null => serde_json::json!(0),
            _ => serde_json::json!(1),
        },
        "sum" => {
            let values = collect_numeric_values(&req.input);
            serde_json::json!(values.iter().sum::<f64>())
        }
        "avg" | "mean" => {
            let values = collect_numeric_values(&req.input);
            if values.is_empty() {
                serde_json::json!(0.0)
            } else {
                serde_json::json!(values.iter().sum::<f64>() / values.len() as f64)
            }
        }
        "min" => {
            let values = collect_numeric_values(&req.input);
            if values.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!(values.iter().fold(f64::INFINITY, |acc, v| acc.min(*v)))
            }
        }
        "max" => {
            let values = collect_numeric_values(&req.input);
            if values.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!(values.iter().fold(f64::NEG_INFINITY, |acc, v| acc.max(*v)))
            }
        }
        "unique_count" => {
            if let Some(arr) = req.input.as_array() {
                let mut uniq: HashSet<String> = HashSet::new();
                for item in arr {
                    let key = serde_json::to_string(item).unwrap_or_default();
                    uniq.insert(key);
                }
                serde_json::json!(uniq.len())
            } else {
                serde_json::json!(0)
            }
        }
        _ => {
            return Err(ApiError::InvalidRequest(format!(
                "Unsupported plugin function: {}",
                req.fn_string
            )))
        }
    };

    Ok(Json(serde_json::json!({
        "success": true,
        "result": result
    })))
}

#[derive(Debug, Deserialize)]
struct ExecuteSqlRequest {
    sql: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerateSqlRequest {
    prompt: String,
    max_rows: Option<usize>,
}

fn strip_sql_literals_and_comments_for_validation(sql: &str) -> String {
    #[derive(Copy, Clone, Eq, PartialEq)]
    enum State {
        Normal,
        SingleQuote,
        DoubleQuote,
        BacktickQuote,
        LineComment,
        BlockComment,
    }

    let chars: Vec<char> = sql.chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut i = 0usize;
    let mut state = State::Normal;
    while i < chars.len() {
        let ch = chars[i];
        let next = chars.get(i + 1).copied();
        match state {
            State::Normal => {
                if ch == '\'' {
                    state = State::SingleQuote;
                    i += 1;
                    continue;
                }
                if ch == '"' {
                    state = State::DoubleQuote;
                    i += 1;
                    continue;
                }
                if ch == '`' {
                    state = State::BacktickQuote;
                    i += 1;
                    continue;
                }
                if ch == '-' && next == Some('-') {
                    state = State::LineComment;
                    i += 2;
                    continue;
                }
                if ch == '/' && next == Some('*') {
                    state = State::BlockComment;
                    i += 2;
                    continue;
                }
                out.push(ch);
                i += 1;
            }
            State::SingleQuote => {
                if ch == '\'' {
                    if next == Some('\'') {
                        i += 2;
                        continue;
                    }
                    state = State::Normal;
                }
                i += 1;
            }
            State::DoubleQuote => {
                if ch == '"' {
                    if next == Some('"') {
                        i += 2;
                        continue;
                    }
                    state = State::Normal;
                }
                i += 1;
            }
            State::BacktickQuote => {
                if ch == '`' {
                    state = State::Normal;
                }
                i += 1;
            }
            State::LineComment => {
                if ch == '\n' {
                    out.push(' ');
                    state = State::Normal;
                }
                i += 1;
            }
            State::BlockComment => {
                if ch == '*' && next == Some('/') {
                    out.push(' ');
                    state = State::Normal;
                    i += 2;
                } else {
                    i += 1;
                }
            }
        }
    }
    out
}

fn validate_read_only_sql(sql: &str) -> Result<(), ApiError> {
    let trimmed = sql.trim();
    if trimmed.is_empty() {
        return Err(ApiError::InvalidRequest(
            "SQL query cannot be empty".to_string(),
        ));
    }

    let no_trailing_semicolon = trimmed.trim_end_matches(';').trim_end();
    if no_trailing_semicolon.contains(';') {
        return Err(ApiError::InvalidRequest(
            "Multiple SQL statements are not allowed".to_string(),
        ));
    }

    let cleaned = strip_sql_literals_and_comments_for_validation(no_trailing_semicolon);
    let tokens = cleaned
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_uppercase())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return Err(ApiError::InvalidRequest(
            "Only SELECT statements are allowed".to_string(),
        ));
    }

    let first = tokens[0].as_str();
    if first != "SELECT" && first != "WITH" {
        return Err(ApiError::InvalidRequest(
            "Only SELECT statements are allowed".to_string(),
        ));
    }
    if first == "WITH" && !tokens.iter().any(|t| t == "SELECT") {
        return Err(ApiError::InvalidRequest(
            "CTE query must contain SELECT".to_string(),
        ));
    }

    const FORBIDDEN: &[&str] = &[
        "INSERT",
        "UPDATE",
        "DELETE",
        "DROP",
        "ALTER",
        "CREATE",
        "REPLACE",
        "ATTACH",
        "DETACH",
        "VACUUM",
        "PRAGMA",
        "BEGIN",
        "COMMIT",
        "ROLLBACK",
        "SAVEPOINT",
        "RELEASE",
        "TRUNCATE",
    ];
    if tokens
        .iter()
        .any(|token| FORBIDDEN.contains(&token.as_str()))
    {
        return Err(ApiError::InvalidRequest(
            "Only read-only SELECT queries are allowed".to_string(),
        ));
    }
    Ok(())
}

fn sql_generation_limit(max_rows: Option<usize>) -> usize {
    max_rows.unwrap_or(100).clamp(1, 500)
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn extract_first_quoted_segment(text: &str) -> Option<String> {
    let quote_pairs = [
        ('"', '"'),
        ('\'', '\''),
        ('`', '`'),
        ('“', '”'),
        ('‘', '’'),
        ('「', '」'),
        ('『', '』'),
    ];

    for (open, close) in quote_pairs {
        let mut in_segment = false;
        let mut buf = String::new();
        for ch in text.chars() {
            if !in_segment && ch == open {
                in_segment = true;
                buf.clear();
                continue;
            }
            if in_segment && ch == close {
                let value = buf.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
                in_segment = false;
                continue;
            }
            if in_segment {
                buf.push(ch);
            }
        }
    }
    None
}

fn escape_like_pattern(raw: &str) -> String {
    raw.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
        .replace('\'', "''")
}

fn build_sql_generation_message_filter(meta_id: i64, keyword: Option<&str>) -> String {
    let mut where_clause = format!("WHERE msg.meta_id = {}", meta_id);
    if let Some(value) = keyword.map(str::trim).filter(|v| !v.is_empty()) {
        let escaped = escape_like_pattern(value);
        where_clause.push_str(&format!(
            " AND msg.content LIKE '%{escaped}%' ESCAPE '\\\\'"
        ));
    }
    where_clause
}

fn append_sql_condition(where_clause: &str, condition: &str) -> String {
    if where_clause.trim().is_empty() {
        format!("WHERE {}", condition)
    } else {
        format!("{} AND {}", where_clause, condition)
    }
}

fn generate_sql_from_prompt(
    prompt: &str,
    meta_id: i64,
    max_rows: usize,
    has_message_table: bool,
    has_member_table: bool,
) -> (String, String, Vec<String>) {
    if !has_message_table {
        let sql = format!(
            "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name LIMIT {}",
            max_rows
        );
        let explanation = "The database does not contain a `message` table yet, so this query lists available tables first.".to_string();
        return (
            sql,
            explanation,
            vec!["message table is missing; generated a schema discovery query".to_string()],
        );
    }

    let normalized = prompt.to_lowercase();
    let keyword = extract_first_quoted_segment(prompt);
    let where_clause = build_sql_generation_message_filter(meta_id, keyword.as_deref());
    let mut warnings = Vec::new();
    if keyword.is_none()
        && contains_any(
            &normalized,
            &["keyword", "关键词", "包含", "contains", "search", "搜索"],
        )
    {
        warnings.push(
            "No quoted keyword detected; generated query without explicit keyword filter"
                .to_string(),
        );
    }

    let is_count_intent = contains_any(
        &normalized,
        &[
            "count", "统计", "数量", "多少", "排行", "top", "active", "活跃",
        ],
    );
    let is_message_type_intent = contains_any(
        &normalized,
        &[
            "message type",
            "msg type",
            "type distribution",
            "消息类型",
            "类型分布",
            "文字",
            "图片",
            "视频",
            "语音",
            "文件",
        ],
    );
    let is_longest_message_intent = contains_any(
        &normalized,
        &[
            "longest",
            "long message",
            "message length",
            "最长",
            "长度",
            "长文本",
        ],
    );
    let is_mention_intent = normalized.contains('@')
        || contains_any(
            &normalized,
            &["mention", "mentions", "@提及", "提及", "被提到"],
        );
    let is_hourly_intent = contains_any(
        &normalized,
        &[
            "hour",
            "hourly",
            "按小时",
            "小时",
            "时段",
            "time distribution",
            "时间分布",
        ],
    );
    let is_weekday_intent = contains_any(
        &normalized,
        &[
            "weekday",
            "week day",
            "按星期",
            "星期",
            "周几",
            "周内",
            "day of week",
        ],
    );
    let is_daily_intent = contains_any(
        &normalized,
        &["daily", "day", "按天", "每日", "每天", "日期"],
    );
    let is_monthly_intent = contains_any(
        &normalized,
        &[
            "monthly",
            "month",
            "按月",
            "每月",
            "月份",
            "month distribution",
        ],
    );
    let is_yearly_intent = contains_any(
        &normalized,
        &[
            "yearly",
            "year",
            "按年",
            "每年",
            "年份",
            "year distribution",
        ],
    );
    let is_recent_intent = contains_any(
        &normalized,
        &["recent", "latest", "last", "最近", "最新", "刚刚", "near"],
    );

    if is_hourly_intent {
        let sql = format!(
            "SELECT strftime('%H', datetime(msg.ts, 'unixepoch', 'localtime')) AS hour_bucket, COUNT(*) AS message_count \
             FROM message msg {} GROUP BY hour_bucket ORDER BY hour_bucket LIMIT {}",
            where_clause, max_rows
        );
        let explanation =
            "Groups messages by local hour and returns message counts per hour bucket.".to_string();
        return (sql, explanation, warnings);
    }

    if is_weekday_intent {
        let sql = format!(
            "SELECT strftime('%w', datetime(msg.ts, 'unixepoch', 'localtime')) AS weekday_bucket, COUNT(*) AS message_count \
             FROM message msg {} GROUP BY weekday_bucket ORDER BY weekday_bucket LIMIT {}",
            where_clause, max_rows
        );
        let explanation =
            "Groups messages by local weekday (0-6) and returns message counts per bucket."
                .to_string();
        return (sql, explanation, warnings);
    }

    if is_daily_intent {
        let sql = format!(
            "SELECT date(datetime(msg.ts, 'unixepoch', 'localtime')) AS day_bucket, COUNT(*) AS message_count \
             FROM message msg {} GROUP BY day_bucket ORDER BY day_bucket DESC LIMIT {}",
            where_clause, max_rows
        );
        let explanation =
            "Groups messages by local day and returns message counts per day bucket.".to_string();
        return (sql, explanation, warnings);
    }

    if is_monthly_intent {
        let sql = format!(
            "SELECT strftime('%Y-%m', datetime(msg.ts, 'unixepoch', 'localtime')) AS month_bucket, COUNT(*) AS message_count \
             FROM message msg {} GROUP BY month_bucket ORDER BY month_bucket DESC LIMIT {}",
            where_clause, max_rows
        );
        let explanation =
            "Groups messages by local month and returns message counts per month bucket."
                .to_string();
        return (sql, explanation, warnings);
    }

    if is_yearly_intent {
        let sql = format!(
            "SELECT strftime('%Y', datetime(msg.ts, 'unixepoch', 'localtime')) AS year_bucket, COUNT(*) AS message_count \
             FROM message msg {} GROUP BY year_bucket ORDER BY year_bucket DESC LIMIT {}",
            where_clause, max_rows
        );
        let explanation =
            "Groups messages by local year and returns message counts per year bucket.".to_string();
        return (sql, explanation, warnings);
    }

    if is_message_type_intent {
        let sql = format!(
            "SELECT msg.msg_type, COUNT(*) AS message_count \
             FROM message msg {} GROUP BY msg.msg_type ORDER BY message_count DESC, msg.msg_type LIMIT {}",
            where_clause, max_rows
        );
        let explanation =
            "Returns message counts grouped by msg_type, ordered by frequency.".to_string();
        return (sql, explanation, warnings);
    }

    if is_longest_message_intent {
        let where_with_content = append_sql_condition(
            &where_clause,
            "msg.content IS NOT NULL AND length(trim(msg.content)) > 0",
        );
        if has_member_table {
            let sql = format!(
                "SELECT msg.id, datetime(msg.ts, 'unixepoch', 'localtime') AS local_time, \
                 COALESCE(m.group_nickname, m.account_name, m.platform_id, printf('member_%d', msg.sender_id)) AS sender_name, \
                 msg.content, length(msg.content) AS content_length \
                 FROM message msg \
                 LEFT JOIN member m ON m.id = msg.sender_id \
                 {} \
                 ORDER BY content_length DESC, msg.ts DESC \
                 LIMIT {}",
                where_with_content, max_rows
            );
            let explanation =
                "Returns messages with the longest content length and sender display name."
                    .to_string();
            return (sql, explanation, warnings);
        }

        warnings.push(
            "member table is missing; sender display name is unavailable and sender_id is returned"
                .to_string(),
        );
        let sql = format!(
            "SELECT msg.id, datetime(msg.ts, 'unixepoch', 'localtime') AS local_time, msg.sender_id, \
             msg.content, length(msg.content) AS content_length \
             FROM message msg {} ORDER BY content_length DESC, msg.ts DESC LIMIT {}",
            where_with_content, max_rows
        );
        let explanation =
            "Returns messages with the longest content length and sender_id.".to_string();
        return (sql, explanation, warnings);
    }

    if is_mention_intent {
        let mention_where = append_sql_condition(&where_clause, "msg.content LIKE '%@%'");
        if has_member_table {
            let sql = format!(
                "SELECT msg.id, datetime(msg.ts, 'unixepoch', 'localtime') AS local_time, \
                 COALESCE(m.group_nickname, m.account_name, m.platform_id, printf('member_%d', msg.sender_id)) AS sender_name, \
                 msg.content \
                 FROM message msg \
                 LEFT JOIN member m ON m.id = msg.sender_id \
                 {} \
                 ORDER BY msg.ts DESC, msg.id DESC \
                 LIMIT {}",
                mention_where, max_rows
            );
            let explanation =
                "Returns recent messages containing @ mentions with sender display name."
                    .to_string();
            return (sql, explanation, warnings);
        }

        warnings.push(
            "member table is missing; sender display name is unavailable and sender_id is returned"
                .to_string(),
        );
        let sql = format!(
            "SELECT msg.id, datetime(msg.ts, 'unixepoch', 'localtime') AS local_time, msg.sender_id, msg.content \
             FROM message msg {} ORDER BY msg.ts DESC, msg.id DESC LIMIT {}",
            mention_where, max_rows
        );
        let explanation =
            "Returns recent messages containing @ mentions with sender_id.".to_string();
        return (sql, explanation, warnings);
    }

    if is_count_intent {
        if has_member_table {
            let sql = format!(
                "SELECT COALESCE(m.group_nickname, m.account_name, m.platform_id, printf('member_%d', msg.sender_id)) AS sender_name, \
                 COUNT(*) AS message_count \
                 FROM message msg \
                 LEFT JOIN member m ON m.id = msg.sender_id \
                 {} \
                 GROUP BY msg.sender_id, sender_name \
                 ORDER BY message_count DESC \
                 LIMIT {}",
                where_clause, max_rows
            );
            let explanation =
                "Returns sender-level message counts ranked from most active to least active."
                    .to_string();
            return (sql, explanation, warnings);
        }

        warnings.push(
            "member table is missing; sender names are unavailable and sender_id is used instead"
                .to_string(),
        );
        let sql = format!(
            "SELECT msg.sender_id, COUNT(*) AS message_count \
             FROM message msg {} GROUP BY msg.sender_id ORDER BY message_count DESC LIMIT {}",
            where_clause, max_rows
        );
        let explanation =
            "Returns sender-level message counts using sender_id because member metadata is unavailable."
                .to_string();
        return (sql, explanation, warnings);
    }

    if is_recent_intent || keyword.is_some() {
        if has_member_table {
            let sql = format!(
                "SELECT msg.id, datetime(msg.ts, 'unixepoch', 'localtime') AS local_time, \
                 COALESCE(m.group_nickname, m.account_name, m.platform_id, printf('member_%d', msg.sender_id)) AS sender_name, \
                 msg.content \
                 FROM message msg \
                 LEFT JOIN member m ON m.id = msg.sender_id \
                 {} \
                 ORDER BY msg.ts DESC, msg.id DESC \
                 LIMIT {}",
                where_clause, max_rows
            );
            let explanation =
                "Returns recent messages (optionally keyword-filtered) with local time and sender display name."
                    .to_string();
            return (sql, explanation, warnings);
        }

        warnings.push(
            "member table is missing; sender display name is unavailable and sender_id is returned"
                .to_string(),
        );
        let sql = format!(
            "SELECT msg.id, datetime(msg.ts, 'unixepoch', 'localtime') AS local_time, msg.sender_id, msg.content \
             FROM message msg {} ORDER BY msg.ts DESC, msg.id DESC LIMIT {}",
            where_clause, max_rows
        );
        let explanation =
            "Returns recent messages (optionally keyword-filtered) with sender_id.".to_string();
        return (sql, explanation, warnings);
    }

    let sql = format!(
        "SELECT msg.id, datetime(msg.ts, 'unixepoch', 'localtime') AS local_time, msg.sender_id, msg.content \
         FROM message msg {} ORDER BY msg.ts DESC, msg.id DESC LIMIT {}",
        where_clause, max_rows
    );
    let explanation = "Returns recent message rows as the default fallback query.".to_string();
    (
        sql,
        explanation,
        vec!["No specific intent detected; generated default recent-message query".to_string()],
    )
}

fn sql_lab_timeout_ms() -> u64 {
    const DEFAULT_TIMEOUT_MS: u64 = 5_000;
    const MIN_TIMEOUT_MS: u64 = 50;
    const MAX_TIMEOUT_MS: u64 = 120_000;

    match std::env::var("XENOBOT_SQL_TIMEOUT_MS") {
        Ok(raw) => raw
            .trim()
            .parse::<u64>()
            .ok()
            .map(|v| v.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS))
            .unwrap_or(DEFAULT_TIMEOUT_MS),
        Err(_) => DEFAULT_TIMEOUT_MS,
    }
}

#[instrument]
async fn generate_sql_assist(
    Path(session_id): Path<String>,
    Json(req): Json<GenerateSqlRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let prompt = req.prompt.trim();
    if prompt.is_empty() {
        return Err(ApiError::InvalidRequest(
            "Prompt cannot be empty".to_string(),
        ));
    }

    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session_id".to_string()))?;
    let limit = sql_generation_limit(req.max_rows);

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    ensure_chat_session_exists(&pool, meta_id).await?;

    let table_names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;
    let table_set: HashSet<String> = table_names.into_iter().collect();
    let has_message_table = table_set.contains("message");
    let has_member_table = table_set.contains("member");

    let (sql, explanation, warnings) =
        generate_sql_from_prompt(prompt, meta_id, limit, has_message_table, has_member_table);
    validate_read_only_sql(&sql)?;

    Ok(Json(serde_json::json!({
        "success": true,
        "sql": sql,
        "explanation": explanation,
        "strategy": "rule_based_safe_sql",
        "limit": limit,
        "warnings": warnings
    })))
}

#[instrument]
async fn execute_sql(
    Path(session_id): Path<String>,
    Json(req): Json<ExecuteSqlRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    const MAX_SQL_RESULT_ROWS: usize = 5000;

    // 1) Parse and validate input SQL
    let sql = req.sql.trim().to_string();
    validate_read_only_sql(&sql)?;

    // 2) Parse meta_id from session_id
    let meta_id: i64 = match session_id.parse::<i64>() {
        Ok(v) => v,
        Err(_) => return Err(ApiError::InvalidRequest("Invalid session_id".to_string())),
    };

    // 3) Acquire DB pool
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    ensure_chat_session_exists(&pool, meta_id).await?;

    // 4) Execute and time the query
    let timeout_ms = sql_lab_timeout_ms();
    let start = std::time::Instant::now();
    let (rows, limited) =
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), async {
            let mut rows = Vec::new();
            let mut limited = false;
            let mut stream = sqlx::query(&sql).fetch(&*pool);
            while let Some(row) = stream
                .try_next()
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?
            {
                if rows.len() >= MAX_SQL_RESULT_ROWS {
                    limited = true;
                    break;
                }
                rows.push(row);
            }
            Ok::<_, ApiError>((rows, limited))
        })
        .await
        .map_err(|_| ApiError::Timeout(format!("SQL query exceeded {} ms", timeout_ms)))??;
    let duration = start.elapsed().as_millis();

    // 5) Build columns from first row (if any)
    let mut columns: Vec<String> = Vec::new();
    if let Some(first) = rows.get(0) {
        for c in first.columns() {
            columns.push(c.name().to_string());
        }
    }

    // 6) Build rows as JSON values
    let mut json_rows: Vec<Vec<serde_json::Value>> = Vec::new();
    for row in rows {
        let mut row_vals: Vec<serde_json::Value> = Vec::with_capacity(columns.len());
        for idx in 0..columns.len() {
            let value: serde_json::Value = if let Ok(v) = row.try_get::<String, _>(idx) {
                serde_json::Value::String(v)
            } else if let Ok(v) = row.try_get::<i64, _>(idx) {
                serde_json::Value::Number(serde_json::Number::from(v))
            } else if let Ok(v) = row.try_get::<f64, _>(idx) {
                serde_json::Value::Number(
                    serde_json::Number::from_f64(v).unwrap_or(serde_json::Number::from(0)),
                )
            } else if let Ok(v) = row.try_get::<bool, _>(idx) {
                serde_json::Value::Bool(v)
            } else if let Ok(v) = row.try_get::<Vec<u8>, _>(idx) {
                if v.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::Value::String("<binary>".to_string())
                }
            } else {
                serde_json::Value::Null
            };
            row_vals.push(value);
        }
        json_rows.push(row_vals);
    }

    let result = serde_json::json!({
        "columns": columns,
        "rows": json_rows,
        "rowCount": json_rows.len(),
        "duration": duration,
        "limited": limited,
        "timedOut": false
    });

    Ok(Json(result))
}

#[instrument]
async fn get_schema(
    Path(session_id): Path<String>,
    Query(query): Query<SchemaQueryParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session_id".to_string()))?;
    // Get database connection pool
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    ensure_chat_session_exists(&pool, meta_id).await?;

    // Query all user tables (excluding sqlite internal tables)
    #[derive(Debug, sqlx::FromRow)]
    struct TableRow {
        name: String,
    }

    let tables: Vec<TableRow> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    )
    .fetch_all(&*pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    let mut schema = Vec::new();
    let mut total_columns: usize = 0;
    let mut total_indexes: usize = 0;
    let mut total_foreign_keys: usize = 0;

    for table in tables {
        // Get column information for this table using PRAGMA table_info
        // SQLite PRAGMA table_info does not support bound parameters reliably
        // across all drivers, so we use an escaped string literal from a DB-
        // discovered table name (not user input) to keep this path safe.
        let escaped_table_name = escape_sqlite_literal(&table.name);
        let pragma_sql = format!("PRAGMA table_info('{escaped_table_name}')");
        let columns = sqlx::query(&pragma_sql)
            .fetch_all(&*pool)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        let column_info: Vec<serde_json::Value> = columns
            .iter()
            .map(|row| {
                let cid = row.try_get::<i64, _>("cid").unwrap_or_default();
                let name = row.try_get::<String, _>("name").unwrap_or_default();
                let col_type = row.try_get::<String, _>("type").unwrap_or_default();
                let notnull = row.try_get::<i64, _>("notnull").unwrap_or_default();
                let dflt_value = row
                    .try_get::<Option<String>, _>("dflt_value")
                    .ok()
                    .flatten();
                let pk = row.try_get::<i64, _>("pk").unwrap_or_default();
                serde_json::json!({
                    "cid": cid,
                    "name": name,
                    "type": col_type,
                    "notnull": notnull == 1,
                    "dflt_value": dflt_value,
                    "pk": pk == 1,
                })
            })
            .collect();

        total_columns += column_info.len();

        if !query.detailed {
            schema.push(serde_json::json!({
                "name": table.name,
                "columns": column_info,
            }));
            continue;
        }

        // SQLite PRAGMA index_list/index_info also require dynamic statement assembly.
        let index_list_sql = format!("PRAGMA index_list('{escaped_table_name}')");
        let index_list_rows = sqlx::query(&index_list_sql)
            .fetch_all(&*pool)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
        let mut index_values = Vec::new();

        for index_row in index_list_rows {
            let index_name = index_row.try_get::<String, _>("name").unwrap_or_default();
            if index_name.is_empty() {
                continue;
            }

            let escaped_index_name = escape_sqlite_literal(&index_name);
            let index_info_sql = format!("PRAGMA index_info('{escaped_index_name}')");
            let index_columns_rows = sqlx::query(&index_info_sql)
                .fetch_all(&*pool)
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?;
            let index_columns: Vec<String> = index_columns_rows
                .iter()
                .filter_map(|index_col| index_col.try_get::<String, _>("name").ok())
                .collect();

            index_values.push(serde_json::json!({
                "name": index_name,
                "unique": index_row.try_get::<i64, _>("unique").unwrap_or_default() == 1,
                "origin": index_row.try_get::<String, _>("origin").unwrap_or_else(|_| "c".to_string()),
                "partial": index_row.try_get::<i64, _>("partial").unwrap_or_default() == 1,
                "columns": index_columns,
            }));
        }
        total_indexes += index_values.len();

        let fk_list_sql = format!("PRAGMA foreign_key_list('{escaped_table_name}')");
        let fk_rows = sqlx::query(&fk_list_sql)
            .fetch_all(&*pool)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
        let foreign_keys: Vec<serde_json::Value> = fk_rows
            .iter()
            .map(|fk| {
                serde_json::json!({
                    "id": fk.try_get::<i64, _>("id").unwrap_or_default(),
                    "seq": fk.try_get::<i64, _>("seq").unwrap_or_default(),
                    "table": fk.try_get::<String, _>("table").unwrap_or_default(),
                    "from": fk.try_get::<String, _>("from").unwrap_or_default(),
                    "to": fk.try_get::<String, _>("to").unwrap_or_default(),
                    "onUpdate": fk.try_get::<String, _>("on_update").unwrap_or_default(),
                    "onDelete": fk.try_get::<String, _>("on_delete").unwrap_or_default(),
                    "match": fk.try_get::<String, _>("match").unwrap_or_default(),
                })
            })
            .collect();
        total_foreign_keys += foreign_keys.len();

        let row_count = if query.include_row_count {
            let row_count_sql = format!(
                "SELECT COUNT(*) FROM {}",
                escape_sqlite_identifier(&table.name)
            );
            Some(
                sqlx::query_scalar::<_, i64>(&row_count_sql)
                    .fetch_one(&*pool)
                    .await
                    .map_err(|e| ApiError::Database(e.to_string()))?,
            )
        } else {
            None
        };

        schema.push(serde_json::json!({
            "name": table.name,
            "columns": column_info,
            "indexes": index_values,
            "foreignKeys": foreign_keys,
            "rowCount": row_count,
        }));
    }

    if query.detailed {
        return Ok(Json(serde_json::json!({
            "tables": schema,
            "summary": {
                "tableCount": schema.len(),
                "columnCount": total_columns,
                "indexCount": total_indexes,
                "foreignKeyCount": total_foreign_keys
            },
            "includesRowCount": query.include_row_count
        })));
    }

    Ok(Json(serde_json::json!(schema)))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SchemaQueryParams {
    #[serde(default)]
    detailed: bool,
    #[serde(default, alias = "include_row_count")]
    include_row_count: bool,
}

fn escape_sqlite_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn escape_sqlite_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnalyzeIncrementalImportRequest {
    #[serde(alias = "file_path")]
    file_path: String,
    #[serde(default, alias = "expected_fingerprint")]
    expected_fingerprint: Option<String>,
}

#[instrument]
async fn analyze_incremental_import(
    Path(session_id): Path<String>,
    Json(req): Json<AnalyzeIncrementalImportRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let repo = crate::database::Repository::new(pool.clone());
    let session_exists = repo
        .get_chat(meta_id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?
        .is_some();
    if !session_exists {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "error.session_not_found",
            "newMessageCount": 0,
            "duplicateCount": 0,
            "totalInFile": 0,
        })));
    }

    let source_fingerprint = build_source_checkpoint_fingerprint(&req.file_path)?;
    let source_kind = format!("api-incremental-{}", meta_id);
    let unchanged = repo
        .source_checkpoint_is_unchanged(
            source_kind.as_str(),
            req.file_path.as_str(),
            source_fingerprint.fingerprint.as_str(),
        )
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    if unchanged {
        let checkpoint = repo
            .get_import_source_checkpoint(source_kind.as_str(), req.file_path.as_str())
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
        return Ok(Json(serde_json::json!({
            "success": true,
            "sessionId": meta_id.to_string(),
            "newMessageCount": 0,
            "duplicateCount": 0,
            "totalInFile": 0,
            "checkpointSkipped": true,
            "sourceFingerprint": source_fingerprint.fingerprint,
            "checkpointMeta": checkpoint_meta_json(&source_fingerprint),
            "lastCheckpoint": checkpoint_state_json(checkpoint),
        })));
    }

    let (_, payload, stats) = match parse_import_file(&req.file_path, None).await {
        Ok(v) => v,
        Err(_) => {
            let _ = upsert_source_checkpoint(
                &repo,
                source_kind.as_str(),
                req.file_path.as_str(),
                &source_fingerprint,
                None,
                None,
                Some(meta_id),
                0,
                0,
                "failed",
                Some("analyze parse failed".to_string()),
            )
            .await;
            return Ok(Json(serde_json::json!({
                "success": false,
                "sessionId": meta_id.to_string(),
                "error": "error.unrecognized_format",
                "newMessageCount": 0,
                "duplicateCount": 0,
                "totalInFile": 0,
                "sourceFingerprint": source_fingerprint.fingerprint,
                "checkpointMeta": checkpoint_meta_json(&source_fingerprint),
            })));
        }
    };

    #[derive(Debug, sqlx::FromRow)]
    struct ExistingRow {
        sender_platform_id: String,
        ts: i64,
        msg_type: i64,
        content: Option<String>,
        platform_message_id: Option<String>,
    }

    let existing_rows: Vec<ExistingRow> = sqlx::query_as(
        r#"
        SELECT
            m.platform_id as sender_platform_id,
            msg.ts as ts,
            msg.msg_type as msg_type,
            msg.content as content,
            msg.platform_message_id as platform_message_id
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id = ?1
        "#,
    )
    .bind(meta_id)
    .fetch_all(&*pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    let mut existing_signatures: HashSet<String> = HashSet::with_capacity(existing_rows.len());
    for row in existing_rows {
        existing_signatures.insert(signature_by_platform_with_message_id(
            &row.sender_platform_id,
            row.ts,
            row.msg_type,
            row.content.as_deref(),
            row.platform_message_id.as_deref(),
        ));
    }

    let mut duplicate_count = 0usize;
    let mut new_count = 0usize;
    for msg in payload.messages {
        let sig = signature_by_platform_with_message_id(
            &msg.sender_platform_id,
            msg.ts,
            msg.msg_type,
            msg.content.as_deref(),
            msg.platform_message_id.as_deref(),
        );
        if existing_signatures.contains(&sig) {
            duplicate_count += 1;
        } else {
            existing_signatures.insert(sig);
            new_count += 1;
        }
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "sessionId": meta_id.to_string(),
        "newMessageCount": new_count,
        "duplicateCount": duplicate_count,
        "totalInFile": stats.messages_received,
        "sourceFingerprint": source_fingerprint.fingerprint,
        "checkpointMeta": checkpoint_meta_json(&source_fingerprint),
    })))
}

#[instrument]
async fn incremental_import(
    Path(session_id): Path<String>,
    Json(req): Json<AnalyzeIncrementalImportRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let repo = crate::database::Repository::new(pool.clone());

    let session_exists = repo
        .get_chat(meta_id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?
        .is_some();
    if !session_exists {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "error.session_not_found"
        })));
    }
    let source_kind = format!("api-incremental-{}", meta_id);
    let source_fingerprint = build_source_checkpoint_fingerprint(&req.file_path)?;
    if let Some(expected_fingerprint) = req.expected_fingerprint.as_ref() {
        if expected_fingerprint != &source_fingerprint.fingerprint {
            return Ok(Json(serde_json::json!({
                "success": false,
                "sessionId": meta_id.to_string(),
                "error": "error.source_changed_since_analyze",
                "expectedFingerprint": expected_fingerprint,
                "sourceFingerprint": source_fingerprint.fingerprint,
                "checkpointMeta": checkpoint_meta_json(&source_fingerprint),
            })));
        }
    }
    let checkpoint_unchanged = repo
        .source_checkpoint_is_unchanged(
            source_kind.as_str(),
            req.file_path.as_str(),
            source_fingerprint.fingerprint.as_str(),
        )
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    if checkpoint_unchanged {
        let checkpoint = repo
            .get_import_source_checkpoint(source_kind.as_str(), req.file_path.as_str())
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
        return Ok(Json(serde_json::json!({
            "success": true,
            "sessionId": meta_id.to_string(),
            "newMessageCount": 0,
            "duplicateCount": 0,
            "totalInFile": 0,
            "checkpointSkipped": true,
            "sourceFingerprint": source_fingerprint.fingerprint,
            "checkpointMeta": checkpoint_meta_json(&source_fingerprint),
            "lastCheckpoint": checkpoint_state_json(checkpoint),
        })));
    }

    let started_at = now_ts();
    let progress_id = repo
        .create_import_progress(&ImportProgress {
            id: 0,
            file_path: req.file_path.clone(),
            total_messages: Some(0),
            processed_messages: Some(0),
            status: Some("detecting".to_string()),
            started_at: Some(started_at),
            completed_at: None,
            error_message: None,
        })
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let parse_result = parse_import_file(&req.file_path, None).await;
    let (_, payload, stats) = match parse_result {
        Ok(v) => v,
        Err(_) => {
            let _ = repo.fail_import(progress_id, "unrecognized format").await;
            let _ = upsert_source_checkpoint(
                &repo,
                source_kind.as_str(),
                req.file_path.as_str(),
                &source_fingerprint,
                None,
                None,
                Some(meta_id),
                0,
                0,
                "failed",
                Some("unrecognized format".to_string()),
            )
            .await;
            return Ok(Json(serde_json::json!({
                "success": false,
                "sessionId": meta_id.to_string(),
                "error": "error.unrecognized_format",
                "sourceFingerprint": source_fingerprint.fingerprint,
                "checkpointMeta": checkpoint_meta_json(&source_fingerprint),
            })));
        }
    };

    if payload.messages.is_empty() {
        let _ = repo.fail_import(progress_id, "no new messages").await;
        let _ = upsert_source_checkpoint(
            &repo,
            source_kind.as_str(),
            req.file_path.as_str(),
            &source_fingerprint,
            Some(payload.platform.as_str()),
            Some(payload.name.as_str()),
            Some(meta_id),
            0,
            0,
            "failed",
            Some("no new messages".to_string()),
        )
        .await;
        return Ok(Json(serde_json::json!({
            "success": false,
            "sessionId": meta_id.to_string(),
            "error": "error.no_messages",
            "sourceFingerprint": source_fingerprint.fingerprint,
            "checkpointMeta": checkpoint_meta_json(&source_fingerprint),
        })));
    }

    let _ =
        sqlx::query("UPDATE import_progress SET total_messages = ?2, status = ?3 WHERE id = ?1")
            .bind(progress_id)
            .bind(payload.messages.len() as i32)
            .bind("saving")
            .execute(&*pool)
            .await;
    let payload_platform = payload.platform.clone();
    let payload_name = payload.name.clone();
    let payload_members = payload.members;
    let payload_messages = payload.messages;

    #[derive(Debug, sqlx::FromRow)]
    struct ExistingRow {
        sender_id: i64,
        ts: i64,
        msg_type: i64,
        content: Option<String>,
        platform_message_id: Option<String>,
    }
    let existing_rows: Vec<ExistingRow> = sqlx::query_as(
        r#"
        SELECT sender_id, ts, msg_type, content, platform_message_id
        FROM message
        WHERE meta_id = ?1
        "#,
    )
    .bind(meta_id)
    .fetch_all(&*pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    let mut existing_signatures: HashSet<String> = HashSet::with_capacity(existing_rows.len());
    for row in existing_rows {
        existing_signatures.insert(signature_by_sender_id_with_message_id(
            row.sender_id,
            row.ts,
            row.msg_type,
            row.content.as_deref(),
            row.platform_message_id.as_deref(),
        ));
    }

    let webhook_runtime = read_api_webhook_config();
    let webhook_items = webhook_runtime.rules;
    let webhook_dispatch = webhook_runtime.dispatch;
    let webhook_client = if webhook_items.is_empty() {
        None
    } else {
        Some(
            reqwest::Client::builder()
                .timeout(Duration::from_millis(webhook_dispatch.request_timeout_ms))
                .build()
                .map_err(|e| ApiError::Http(e.to_string()))?,
        )
    };
    let mut webhook_stats = WebhookDispatchStats::default();
    let mut webhook_queue: Vec<WebhookMessageCreatedEvent> = Vec::new();
    let mut webhook_queue_first_enqueued_at: Option<Instant> = None;

    let mut processed = 0i32;
    let mut duplicate_count = 0usize;
    let mut new_count = 0usize;
    let write_result = async {
        let mut member_cache: HashMap<String, i64> = HashMap::new();
        for member in &payload_members {
            let member_id = ensure_member_profile_and_history(&repo, member, started_at).await?;
            member_cache.insert(member.platform_id.clone(), member_id);
        }

        for msg in payload_messages {
            let sender_id = if let Some(existing) = member_cache.get(&msg.sender_platform_id) {
                *existing
            } else {
                let profile = ParsedMemberProfile {
                    platform_id: msg.sender_platform_id.clone(),
                    account_name: msg.sender_name.clone(),
                    group_nickname: msg.sender_name.clone(),
                };
                let created = ensure_member_profile_and_history(&repo, &profile, msg.ts).await?;
                member_cache.insert(profile.platform_id, created);
                created
            };

            let signature = signature_by_sender_id_with_message_id(
                sender_id,
                msg.ts,
                msg.msg_type,
                msg.content.as_deref(),
                msg.platform_message_id.as_deref(),
            );
            if existing_signatures.contains(&signature) {
                duplicate_count += 1;
                processed += 1;
                continue;
            }

            let inserted_message_id = repo
                .create_message(&Message {
                    id: 0,
                    sender_id,
                    sender_account_name: msg.sender_name.clone(),
                    sender_group_nickname: msg.sender_name.clone(),
                    ts: msg.ts,
                    msg_type: msg.msg_type,
                    content: msg.content.clone(),
                    reply_to_message_id: None,
                    platform_message_id: msg.platform_message_id.clone(),
                    meta_id,
                })
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?;
            existing_signatures.insert(signature);

            if let Some(client) = webhook_client.as_ref() {
                let event = WebhookMessageCreatedEvent {
                    event_type: "message.created".to_string(),
                    platform: payload_platform.clone(),
                    chat_name: payload_name.clone(),
                    meta_id,
                    message_id: inserted_message_id,
                    sender_id,
                    sender_name: msg.sender_name.clone(),
                    ts: msg.ts,
                    msg_type: msg.msg_type,
                    content: msg.content.clone(),
                };
                if webhook_queue.is_empty() {
                    webhook_queue_first_enqueued_at = Some(Instant::now());
                }
                webhook_queue.push(event);
                let queue_age = webhook_queue_first_enqueued_at.map(|t| t.elapsed());
                if should_flush_webhook_queue(webhook_queue.len(), queue_age, &webhook_dispatch) {
                    let stats = dispatch_api_webhook_batch(
                        client,
                        &webhook_items,
                        &mut webhook_queue,
                        &webhook_dispatch,
                    )
                    .await;
                    merge_webhook_dispatch_stats(&mut webhook_stats, &stats);
                    webhook_queue_first_enqueued_at = None;
                }
            }

            new_count += 1;
            processed += 1;
            if processed % 200 == 0 {
                let _ = repo.update_progress(progress_id, processed, "saving").await;
            }
        }

        if let Some(client) = webhook_client.as_ref() {
            let stats = dispatch_api_webhook_batch(
                client,
                &webhook_items,
                &mut webhook_queue,
                &webhook_dispatch,
            )
            .await;
            merge_webhook_dispatch_stats(&mut webhook_stats, &stats);
            webhook_queue_first_enqueued_at = None;
        }

        Ok::<(), ApiError>(())
    }
    .await;

    if let Err(err) = write_result {
        let _ = repo.update_progress(progress_id, processed, "failed").await;
        let _ = repo.fail_import(progress_id, &err.to_string()).await;
        let _ = upsert_source_checkpoint(
            &repo,
            source_kind.as_str(),
            req.file_path.as_str(),
            &source_fingerprint,
            Some(payload_platform.as_str()),
            Some(payload_name.as_str()),
            Some(meta_id),
            usize_to_i64_saturating(new_count),
            usize_to_i64_saturating(duplicate_count),
            "failed",
            Some(err.to_string()),
        )
        .await;
        return Err(err);
    }

    let _ = repo.complete_import(progress_id, now_ts()).await;
    let _ = upsert_source_checkpoint(
        &repo,
        source_kind.as_str(),
        req.file_path.as_str(),
        &source_fingerprint,
        Some(payload_platform.as_str()),
        Some(payload_name.as_str()),
        Some(meta_id),
        usize_to_i64_saturating(new_count),
        usize_to_i64_saturating(duplicate_count),
        "completed",
        None,
    )
    .await;

    Ok(Json(serde_json::json!({
        "success": true,
        "sessionId": meta_id.to_string(),
        "newMessageCount": new_count,
        "duplicateCount": duplicate_count,
        "totalInFile": stats.messages_received,
        "sourceFingerprint": source_fingerprint.fingerprint,
        "checkpointMeta": checkpoint_meta_json(&source_fingerprint),
        "webhookSummary": webhook_stats
    })))
}

#[derive(Debug, Deserialize)]
struct ExportSessionsRequest {
    session_ids: Vec<String>,
}

#[instrument]
async fn export_sessions_to_temp_files(
    Json(req): Json<ExportSessionsRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.session_ids.is_empty() {
        return Ok(Json(serde_json::json!({
            "success": true,
            "tempFiles": []
        })));
    }

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let export_dir = std::env::temp_dir().join("xenobot-export");
    tokio::fs::create_dir_all(&export_dir)
        .await
        .map_err(|e| ApiError::Io(e))?;

    #[derive(Debug, sqlx::FromRow)]
    struct MetaRow {
        id: i64,
        name: String,
        platform: String,
        chat_type: String,
        imported_at: i64,
    }

    #[derive(Debug, sqlx::FromRow)]
    struct MessageRow {
        id: i64,
        ts: i64,
        msg_type: i64,
        content: Option<String>,
        platform_message_id: Option<String>,
        sender_platform_id: String,
        sender_name: String,
    }

    let mut temp_files: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for (idx, session_id) in req.session_ids.iter().enumerate() {
        let meta_id = match session_id.parse::<i64>() {
            Ok(v) => v,
            Err(_) => {
                errors.push(format!("Invalid session id: {}", session_id));
                continue;
            }
        };

        let meta = match sqlx::query_as::<_, MetaRow>(
            r#"
            SELECT id, name, platform, chat_type, imported_at
            FROM meta
            WHERE id = ?1
            "#,
        )
        .bind(meta_id)
        .fetch_optional(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?
        {
            Some(v) => v,
            None => {
                errors.push(format!("Session not found: {}", session_id));
                continue;
            }
        };

        let messages: Vec<MessageRow> = sqlx::query_as(
            r#"
            SELECT
                msg.id as id,
                msg.ts as ts,
                msg.msg_type as msg_type,
                msg.content as content,
                msg.platform_message_id as platform_message_id,
                m.platform_id as sender_platform_id,
                COALESCE(
                    msg.sender_group_nickname,
                    msg.sender_account_name,
                    m.group_nickname,
                    m.account_name,
                    m.platform_id
                ) as sender_name
            FROM message msg
            JOIN member m ON msg.sender_id = m.id
            WHERE msg.meta_id = ?1
            ORDER BY msg.ts, msg.id
            "#,
        )
        .bind(meta_id)
        .fetch_all(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

        let payload = serde_json::json!({
            "meta": {
                "id": meta.id,
                "name": meta.name,
                "platform": meta.platform,
                "chatType": meta.chat_type,
                "importedAt": meta.imported_at
            },
            "messages": messages.into_iter().map(|m| {
                serde_json::json!({
                    "id": m.id,
                    "timestamp": m.ts,
                    "type": m.msg_type,
                    "content": m.content,
                    "platformMessageId": m.platform_message_id,
                    "senderId": m.sender_platform_id,
                    "senderName": m.sender_name
                })
            }).collect::<Vec<_>>()
        });

        let file_name = format!(
            "xenobot-export-session-{}-{}-{}.json",
            meta_id,
            now_ts(),
            idx
        );
        let output_path: PathBuf = export_dir.join(file_name);
        let bytes = serde_json::to_vec_pretty(&payload)?;
        if let Err(e) = tokio::fs::write(&output_path, bytes).await {
            errors.push(format!("{}: {}", session_id, e));
            continue;
        }
        temp_files.push(output_path.to_string_lossy().to_string());
    }

    if !errors.is_empty() {
        for file in &temp_files {
            let _ = tokio::fs::remove_file(file).await;
        }
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": errors.join("; ")
        })));
    }

    Ok(Json(serde_json::json!({
        "success": true,
        "tempFiles": temp_files
    })))
}

#[derive(Debug, Deserialize)]
struct CleanupTempFilesRequest {
    file_paths: Vec<String>,
}

#[instrument]
async fn cleanup_temp_export_files(
    Json(req): Json<CleanupTempFilesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.file_paths.is_empty() {
        return Ok(Json(serde_json::json!({
            "success": true,
            "removed": 0
        })));
    }

    let export_dir = std::env::temp_dir().join("xenobot-export");
    let mut removed = 0usize;
    let mut failed: Vec<String> = Vec::new();

    for file_path in req.file_paths {
        let path = PathBuf::from(&file_path);
        if !path.starts_with(&export_dir) {
            failed.push(format!("Not in export directory: {}", file_path));
            continue;
        }
        match tokio::fs::remove_file(&path).await {
            Ok(_) => removed += 1,
            Err(e) => failed.push(format!("{}: {}", file_path, e)),
        }
    }

    Ok(Json(serde_json::json!({
        "success": failed.is_empty(),
        "removed": removed,
        "failed": failed
    })))
}

// Server-Sent Events for import progress
#[instrument]
async fn import_progress_sse() -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    #[derive(Debug, sqlx::FromRow)]
    struct ProgressRow {
        file_path: String,
        total_messages: Option<i32>,
        processed_messages: Option<i32>,
        status: Option<String>,
        error_message: Option<String>,
    }

    let pool_result = crate::database::get_pool().await.map_err(|e| e.to_string());
    let snapshot = match pool_result {
        Ok(pool) => {
            let row = sqlx::query_as::<_, ProgressRow>(
                r#"
                SELECT file_path, total_messages, processed_messages, status, error_message
                FROM import_progress
                ORDER BY id DESC
                LIMIT 1
                "#,
            )
            .fetch_optional(&*pool)
            .await
            .ok()
            .flatten();

            match row {
                Some(r) => ImportProgressResponse {
                    total: r.total_messages.unwrap_or(0).max(0) as u64,
                    processed: r.processed_messages.unwrap_or(0).max(0) as u64,
                    current_file: Some(r.file_path),
                    status: r.status.unwrap_or_else(|| "pending".to_string()),
                    error: r.error_message,
                },
                None => ImportProgressResponse {
                    total: 0,
                    processed: 0,
                    current_file: None,
                    status: "idle".to_string(),
                    error: None,
                },
            }
        }
        Err(e) => ImportProgressResponse {
            total: 0,
            processed: 0,
            current_file: None,
            status: "error".to_string(),
            error: Some(e),
        },
    };

    let data = serde_json::to_string(&snapshot).unwrap_or_else(|_| {
        "{\"total\":0,\"processed\":0,\"status\":\"error\",\"error\":\"serialize\"}".to_string()
    });
    let stream = stream::once(async move {
        Ok::<Event, Infallible>(Event::default().event("import-progress").data(data))
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        sanitize_webhook_dispatch_settings, should_flush_webhook_queue, validate_read_only_sql,
        ApiWebhookDispatchSettings, ApiWebhookItem, ApiWebhookStore, WEBHOOK_BATCH_SIZE_DEFAULT,
        WEBHOOK_FLUSH_INTERVAL_MS_DEFAULT, WEBHOOK_MAX_CONCURRENCY_DEFAULT,
        WEBHOOK_REQUEST_TIMEOUT_MS_DEFAULT, WEBHOOK_RETRY_ATTEMPTS_DEFAULT,
        WEBHOOK_RETRY_BASE_DELAY_MS_DEFAULT,
    };
    use std::time::Duration;

    #[test]
    fn validate_read_only_sql_accepts_select_and_with_queries() {
        assert!(validate_read_only_sql("SELECT id, content FROM message LIMIT 10").is_ok());
        assert!(validate_read_only_sql(
            "WITH recent AS (SELECT id FROM message ORDER BY id DESC LIMIT 5) SELECT * FROM recent"
        )
        .is_ok());
    }

    #[test]
    fn validate_read_only_sql_rejects_mutation_and_multiple_statements() {
        assert!(validate_read_only_sql("DELETE FROM message").is_err());
        assert!(validate_read_only_sql("SELECT 1; DROP TABLE message").is_err());
        assert!(validate_read_only_sql("SELECT 1; SELECT 2").is_err());
    }

    #[test]
    fn validate_read_only_sql_ignores_keywords_inside_literals_and_comments() {
        assert!(validate_read_only_sql("SELECT 'drop table users' AS txt").is_ok());
        assert!(
            validate_read_only_sql("SELECT 1 -- DELETE FROM message\nFROM (SELECT 1 AS v) t")
                .is_ok()
        );
    }

    #[test]
    fn webhook_dispatch_settings_default_to_stable_values() {
        let cfg = sanitize_webhook_dispatch_settings(&ApiWebhookDispatchSettings::default());
        assert_eq!(cfg.batch_size, WEBHOOK_BATCH_SIZE_DEFAULT);
        assert_eq!(cfg.max_concurrency, WEBHOOK_MAX_CONCURRENCY_DEFAULT);
        assert_eq!(cfg.request_timeout_ms, WEBHOOK_REQUEST_TIMEOUT_MS_DEFAULT);
        assert_eq!(cfg.flush_interval_ms, WEBHOOK_FLUSH_INTERVAL_MS_DEFAULT);
        assert_eq!(cfg.retry_attempts, WEBHOOK_RETRY_ATTEMPTS_DEFAULT);
        assert_eq!(cfg.retry_base_delay_ms, WEBHOOK_RETRY_BASE_DELAY_MS_DEFAULT);
    }

    #[test]
    fn webhook_dispatch_settings_are_clamped_to_safe_ranges() {
        let cfg = sanitize_webhook_dispatch_settings(&ApiWebhookDispatchSettings {
            batch_size: Some(0),
            max_concurrency: Some(10_000),
            request_timeout_ms: Some(1),
            flush_interval_ms: Some(50_000),
            retry_attempts: Some(100),
            retry_base_delay_ms: Some(10_000),
        });
        assert_eq!(cfg.batch_size, 1);
        assert_eq!(cfg.max_concurrency, 64);
        assert_eq!(cfg.request_timeout_ms, 200);
        assert_eq!(cfg.flush_interval_ms, 30_000);
        assert_eq!(cfg.retry_attempts, 8);
        assert_eq!(cfg.retry_base_delay_ms, 5_000);
    }

    #[test]
    fn webhook_store_supports_camel_case_fields_for_filters_and_dispatch() {
        let raw = r#"
        {
          "items": [
            {
              "id": "wh_1",
              "url": "https://example.com/hook",
              "eventType": "message.created",
              "platform": "wechat",
              "chatName": "core",
              "metaId": 9,
              "sender": "alice",
              "keyword": "urgent",
              "createdAt": "2026-03-05T00:00:00Z"
            }
          ],
          "dispatch": {
            "batchSize": 16,
            "maxConcurrency": 4,
            "requestTimeoutMs": 5000,
            "flushIntervalMs": 250,
            "retryAttempts": 2,
            "retryBaseDelayMs": 10
          }
        }
        "#;
        let parsed: ApiWebhookStore = serde_json::from_str(raw).expect("parse webhook store");
        assert_eq!(parsed.items.len(), 1);
        let item: &ApiWebhookItem = &parsed.items[0];
        assert_eq!(item.event_type.as_deref(), Some("message.created"));
        assert_eq!(item.platform.as_deref(), Some("wechat"));
        assert_eq!(item.chat_name.as_deref(), Some("core"));
        assert_eq!(item.meta_id, Some(9));
        assert_eq!(item.created_at.as_deref(), Some("2026-03-05T00:00:00Z"));
        let cfg = sanitize_webhook_dispatch_settings(&parsed.dispatch);
        assert_eq!(cfg.batch_size, 16);
        assert_eq!(cfg.max_concurrency, 4);
        assert_eq!(cfg.request_timeout_ms, 5_000);
        assert_eq!(cfg.flush_interval_ms, 250);
        assert_eq!(cfg.retry_attempts, 2);
        assert_eq!(cfg.retry_base_delay_ms, 10);
    }

    #[test]
    fn webhook_queue_flush_decision_prefers_size_then_age() {
        let cfg = sanitize_webhook_dispatch_settings(&ApiWebhookDispatchSettings {
            batch_size: Some(4),
            max_concurrency: None,
            request_timeout_ms: None,
            flush_interval_ms: Some(1_000),
            retry_attempts: None,
            retry_base_delay_ms: None,
        });
        assert!(!should_flush_webhook_queue(0, None, &cfg));
        assert!(!should_flush_webhook_queue(
            2,
            Some(Duration::from_millis(999)),
            &cfg
        ));
        assert!(should_flush_webhook_queue(4, None, &cfg));
        assert!(should_flush_webhook_queue(
            2,
            Some(Duration::from_millis(1_000)),
            &cfg
        ));
    }
}

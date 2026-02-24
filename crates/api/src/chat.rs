//! Chat API module for Xenobot HTTP API.
//!
//! Provides HTTP endpoints equivalent to Xenobot's `chatApi` IPC methods.

use axum::{
    extract::{Path, Query},
    response::sse::{Event, KeepAlive, Sse},
    routing::{delete, get, post},
    Json, Router,
};
use futures::stream;
use serde::{Deserialize, Serialize};
use sqlx::{Column, Row};
use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    fs,
    path::{Path as FsPath, PathBuf},
    time::Duration,
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
struct ParsedChatPayload {
    name: String,
    platform: String,
    chat_type: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiWebhookItem {
    id: String,
    url: String,
    event_type: Option<String>,
    sender: Option<String>,
    keyword: Option<String>,
    created_at: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct ApiWebhookStore {
    items: Vec<ApiWebhookItem>,
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
        sender: item.sender.clone(),
        keyword: item.keyword.clone(),
        created_at: item.created_at.clone(),
    }
}

fn read_api_webhook_items() -> Vec<WebhookRule> {
    let path = api_webhook_store_path();
    if !path.exists() {
        return Vec::new();
    }

    let raw = match fs::read_to_string(&path) {
        Ok(v) => v,
        Err(e) => {
            warn!("failed to read webhook config '{}': {}", path.display(), e);
            return Vec::new();
        }
    };
    if raw.trim().is_empty() {
        return Vec::new();
    }

    match serde_json::from_str::<ApiWebhookStore>(&raw) {
        Ok(store) => store.items.iter().map(api_webhook_item_to_rule).collect(),
        Err(e) => {
            warn!("failed to parse webhook config '{}': {}", path.display(), e);
            Vec::new()
        }
    }
}

async fn dispatch_api_webhook_message_created(
    client: &reqwest::Client,
    items: &[WebhookRule],
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
        for attempt in 0..3u32 {
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
                    if attempt < 2 {
                        let wait_ms = 150_u64 * (1_u64 << attempt);
                        tokio::time::sleep(Duration::from_millis(wait_ms)).await;
                    }
                }
                Err(err) => {
                    last_error = err.to_string();
                    if attempt < 2 {
                        let wait_ms = 150_u64 * (1_u64 << attempt);
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
    max_concurrency: usize,
) -> WebhookDispatchStats {
    if queue.is_empty() {
        return WebhookDispatchStats::default();
    }

    let mut set = tokio::task::JoinSet::new();
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrency.max(1)));
    let shared_items = std::sync::Arc::new(items.to_vec());

    for event in queue.drain(..) {
        let client_clone = client.clone();
        let items_clone = shared_items.clone();
        let semaphore_clone = semaphore.clone();
        set.spawn(async move {
            let _permit = semaphore_clone.acquire_owned().await.ok();
            dispatch_api_webhook_message_created(&client_clone, items_clone.as_slice(), &event)
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
        sender_platform_id: sender_id.unwrap_or_else(|| "unknown".to_string()),
        sender_name,
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

    let mut stats = ImportParseStats::default();
    stats.messages_received = parsed_chat.messages.len();

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
        messages.push(ParsedMessage {
            sender_platform_id: format!("{}:{}", platform, sender),
            sender_name: msg.sender_name.clone(),
            ts: msg.timestamp,
            msg_type: analysis_message_type_to_code(&msg.msg_type),
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
            name: parsed_chat.chat_name,
            platform,
            chat_type: analysis_chat_type_to_text(&parsed_chat.chat_type),
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

fn signature_by_platform(
    sender_platform_id: &str,
    ts: i64,
    msg_type: i64,
    content: Option<&str>,
) -> String {
    format!(
        "{}|{}|{}|{}",
        sender_platform_id,
        ts,
        msg_type,
        normalized_content_for_signature(content)
    )
}

fn signature_by_sender_id(sender_id: i64, ts: i64, msg_type: i64, content: Option<&str>) -> String {
    format!(
        "{}|{}|{}|{}",
        sender_id,
        ts,
        msg_type,
        normalized_content_for_signature(content)
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

    let webhook_items = read_api_webhook_items();
    let webhook_client = if webhook_items.is_empty() {
        None
    } else {
        Some(
            reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .build()
                .map_err(|e| ApiError::Http(e.to_string()))?,
        )
    };
    let mut webhook_stats = WebhookDispatchStats::default();
    let mut webhook_queue: Vec<WebhookMessageCreatedEvent> = Vec::new();

    let mut processed: i32 = 0;
    let write_result = async {
        for msg in payload_messages {
            let sender_id = repo
                .get_or_create_member(&msg.sender_platform_id, msg.sender_name.as_deref())
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?;

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
                webhook_queue.push(event);
                if webhook_queue.len() >= 64 {
                    let stats =
                        dispatch_api_webhook_batch(client, &webhook_items, &mut webhook_queue, 8)
                            .await;
                    merge_webhook_dispatch_stats(&mut webhook_stats, &stats);
                }
            }

            processed += 1;
            if processed % 200 == 0 {
                let _ = repo.update_progress(progress_id, processed, "saving").await;
            }
        }

        if let Some(client) = webhook_client.as_ref() {
            let stats =
                dispatch_api_webhook_batch(client, &webhook_items, &mut webhook_queue, 8).await;
            merge_webhook_dispatch_stats(&mut webhook_stats, &stats);
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
        "diagnostics": import_diagnostics_json(&detected.id, &stats),
        "webhookSummary": webhook_stats
    }))
}

// ==================== Handler Implementations ====================

#[instrument]
async fn check_migration() -> Result<Json<serde_json::Value>, ApiError> {
    // For now, return that migrations are up to date
    Ok(Json(serde_json::json!({
        "needsMigration": false,
        "currentVersion": 3
    })))
}

#[instrument]
async fn run_migration() -> Result<Json<serde_json::Value>, ApiError> {
    // Migrations are handled automatically on startup
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Migrations already applied"
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

    for file_path in file_paths {
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

    for file_path in file_paths {
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

            let signature =
                signature_by_sender_id(sender_id, msg.ts, msg.msg_type, msg.content.as_deref());
            if !merged_seen.insert(signature) {
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
    Query(_filter): Query<TimeFilter>, // 忽略过滤器，保持兼容性
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
        WHERE msg.meta_id = ?1 AND COALESCE(m.account_name, '') != '系统消息'
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
            serde_json::json!({
                "id": node.id,
                "name": node.name,
                "messageCount": node.value,
                "symbolSize": node.symbol_size,
                "degree": degree,
                "normalizedDegree": normalized_degree
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

    let mut adjacency: HashMap<String, HashSet<String>> = HashMap::new();
    for node in &mention_graph.nodes {
        adjacency.entry(node.name.clone()).or_default();
    }
    for link in &mention_graph.links {
        adjacency
            .entry(link.source.clone())
            .or_default()
            .insert(link.target.clone());
        adjacency
            .entry(link.target.clone())
            .or_default()
            .insert(link.source.clone());
    }

    let mut visited: HashSet<String> = HashSet::new();
    let mut communities: Vec<serde_json::Value> = Vec::new();
    for node in &mention_graph.nodes {
        if visited.contains(&node.name) {
            continue;
        }
        let mut stack = vec![node.name.clone()];
        let mut size = 0usize;
        while let Some(current) = stack.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }
            size += 1;
            if let Some(neighbors) = adjacency.get(&current) {
                for neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        stack.push(neighbor.clone());
                    }
                }
            }
        }

        let community_id = communities.len() as i64 + 1;
        communities.push(serde_json::json!({
            "id": community_id,
            "name": format!("Community {}", community_id),
            "size": size as i64
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
            "communityCount": communities.len() as i64
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
async fn get_members(
    Path(session_id): Path<String>,
) -> Result<Json<Vec<MemberResponse>>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    // 直接查询会话中的成员
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
        WHERE msg.meta_id = ?1 AND COALESCE(m.account_name, '') != '系统消息'
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

    // 转换aliases字段从JSON字符串到Vec<String>
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

#[instrument]
async fn execute_sql(
    Path(session_id): Path<String>,
    Json(req): Json<ExecuteSqlRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // 1) Parse and validate input SQL
    let sql = req.sql.trim().to_string();
    let sql_upper = sql.to_ascii_uppercase();
    if !sql_upper.starts_with("SELECT") {
        return Err(ApiError::InvalidRequest(
            "Only SELECT statements are allowed".to_string(),
        ));
    }

    // 2) Parse meta_id from session_id
    let _meta_id: i64 = match session_id.parse::<i64>() {
        Ok(v) => v,
        Err(_) => return Err(ApiError::InvalidRequest("Invalid session_id".to_string())),
    };

    // 3) Acquire DB pool
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    // 4) Execute and time the query
    let start = std::time::Instant::now();
    let rows = sqlx::query(&sql)
        .fetch_all(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
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
        "limited": false
    });

    Ok(Json(result))
}

#[instrument]
async fn get_schema(Path(_session_id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    // Get database connection pool
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

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

    for table in tables {
        // Get column information for this table using PRAGMA table_info
        #[derive(Debug, sqlx::FromRow)]
        struct ColumnRow {
            cid: i64,
            name: String,
            type_: String,
            notnull: i64,
            dflt_value: Option<String>,
            pk: i64,
        }

        let columns: Vec<ColumnRow> = sqlx::query_as("PRAGMA table_info(?)")
            .bind(&table.name)
            .fetch_all(&*pool)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        let column_info: Vec<serde_json::Value> = columns
            .iter()
            .map(|col| {
                serde_json::json!({
                    "cid": col.cid,
                    "name": col.name,
                    "type": col.type_,
                    "notnull": col.notnull == 1,
                    "dflt_value": col.dflt_value,
                    "pk": col.pk == 1,
                })
            })
            .collect();

        schema.push(serde_json::json!({
            "name": table.name,
            "columns": column_info,
        }));
    }

    Ok(Json(serde_json::json!(schema)))
}

#[derive(Debug, Deserialize)]
struct AnalyzeIncrementalImportRequest {
    file_path: String,
}

#[instrument]
async fn analyze_incremental_import(
    Path(session_id): Path<String>,
    Json(req): Json<AnalyzeIncrementalImportRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))?;
    let source_fingerprint = build_source_checkpoint_fingerprint(&req.file_path)?;
    let source_kind = format!("api-incremental-{}", meta_id);

    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let repo = crate::database::Repository::new(pool.clone());
    let unchanged = repo
        .source_checkpoint_is_unchanged(
            source_kind.as_str(),
            req.file_path.as_str(),
            source_fingerprint.fingerprint.as_str(),
        )
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    if unchanged {
        return Ok(Json(serde_json::json!({
            "newMessageCount": 0,
            "duplicateCount": 0,
            "totalInFile": 0,
            "checkpointSkipped": true
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
                "error": "error.unrecognized_format",
                "newMessageCount": 0,
                "duplicateCount": 0,
                "totalInFile": 0
            })));
        }
    };

    #[derive(Debug, sqlx::FromRow)]
    struct ExistingRow {
        sender_platform_id: String,
        ts: i64,
        msg_type: i64,
        content: Option<String>,
    }

    let existing_rows: Vec<ExistingRow> = sqlx::query_as(
        r#"
        SELECT
            m.platform_id as sender_platform_id,
            msg.ts as ts,
            msg.msg_type as msg_type,
            msg.content as content
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
        existing_signatures.insert(signature_by_platform(
            &row.sender_platform_id,
            row.ts,
            row.msg_type,
            row.content.as_deref(),
        ));
    }

    let mut duplicate_count = 0usize;
    let mut new_count = 0usize;
    for msg in payload.messages {
        let sig = signature_by_platform(
            &msg.sender_platform_id,
            msg.ts,
            msg.msg_type,
            msg.content.as_deref(),
        );
        if existing_signatures.contains(&sig) {
            duplicate_count += 1;
        } else {
            new_count += 1;
        }
    }

    Ok(Json(serde_json::json!({
        "newMessageCount": new_count,
        "duplicateCount": duplicate_count,
        "totalInFile": stats.messages_received
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
    let checkpoint_unchanged = repo
        .source_checkpoint_is_unchanged(
            source_kind.as_str(),
            req.file_path.as_str(),
            source_fingerprint.fingerprint.as_str(),
        )
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    if checkpoint_unchanged {
        return Ok(Json(serde_json::json!({
            "success": true,
            "newMessageCount": 0,
            "duplicateCount": 0,
            "totalInFile": 0,
            "checkpointSkipped": true
        })));
    }

    let progress_id = repo
        .create_import_progress(&ImportProgress {
            id: 0,
            file_path: req.file_path.clone(),
            total_messages: Some(0),
            processed_messages: Some(0),
            status: Some("detecting".to_string()),
            started_at: Some(now_ts()),
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
                "error": "error.unrecognized_format"
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
            "error": "error.no_messages"
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
    let payload_messages = payload.messages;

    #[derive(Debug, sqlx::FromRow)]
    struct ExistingRow {
        sender_id: i64,
        ts: i64,
        msg_type: i64,
        content: Option<String>,
    }
    let existing_rows: Vec<ExistingRow> = sqlx::query_as(
        r#"
        SELECT sender_id, ts, msg_type, content
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
        existing_signatures.insert(signature_by_sender_id(
            row.sender_id,
            row.ts,
            row.msg_type,
            row.content.as_deref(),
        ));
    }

    let webhook_items = read_api_webhook_items();
    let webhook_client = if webhook_items.is_empty() {
        None
    } else {
        Some(
            reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .build()
                .map_err(|e| ApiError::Http(e.to_string()))?,
        )
    };
    let mut webhook_stats = WebhookDispatchStats::default();
    let mut webhook_queue: Vec<WebhookMessageCreatedEvent> = Vec::new();

    let mut processed = 0i32;
    let mut duplicate_count = 0usize;
    let mut new_count = 0usize;
    let write_result = async {
        for msg in payload_messages {
            let sender_id = repo
                .get_or_create_member(&msg.sender_platform_id, msg.sender_name.as_deref())
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?;

            let signature =
                signature_by_sender_id(sender_id, msg.ts, msg.msg_type, msg.content.as_deref());
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
                webhook_queue.push(event);
                if webhook_queue.len() >= 64 {
                    let stats =
                        dispatch_api_webhook_batch(client, &webhook_items, &mut webhook_queue, 8)
                            .await;
                    merge_webhook_dispatch_stats(&mut webhook_stats, &stats);
                }
            }

            existing_signatures.insert(signature);
            new_count += 1;
            processed += 1;
            if processed % 200 == 0 {
                let _ = repo.update_progress(progress_id, processed, "saving").await;
            }
        }

        if let Some(client) = webhook_client.as_ref() {
            let stats =
                dispatch_api_webhook_batch(client, &webhook_items, &mut webhook_queue, 8).await;
            merge_webhook_dispatch_stats(&mut webhook_stats, &stats);
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
        "newMessageCount": new_count,
        "duplicateCount": duplicate_count,
        "totalInFile": stats.messages_received,
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

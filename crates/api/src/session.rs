//! Session API module for Xenobot HTTP API.
//!
//! Provides HTTP endpoints equivalent to Xenobot's `sessionApi` IPC methods.
//! This API handles chat session indexing, segmentation, and summarization.

use axum::{
    extract::{Path, Query},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::{collections::HashMap, sync::Arc};
use tracing::instrument;

use crate::ApiError;

const DEFAULT_GAP_THRESHOLD: i64 = 1800;
const MIN_SUMMARY_MESSAGES: usize = 3;

/// Session API router.
pub fn router() -> Router {
    Router::new()
        .route("/generate/:session_id", post(generate))
        .route("/has-index/:session_id", get(has_index))
        .route("/stats/:session_id", get(get_stats))
        .route("/clear/:session_id", post(clear))
        .route(
            "/update-gap-threshold/:session_id",
            post(update_gap_threshold),
        )
        .route("/sessions/:session_id", get(get_sessions))
        .route(
            "/generate-summary/:session_id/:chat_session_id",
            post(generate_summary),
        )
        .route("/generate-summaries/:session_id", post(generate_summaries))
        .route(
            "/check-can-generate-summary/:session_id",
            post(check_can_generate_summary),
        )
        .route("/by-time-range/:session_id", get(get_by_time_range))
        .route("/recent/:session_id", get(get_recent))
}

// Request/Response types

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    #[serde(default)]
    pub gap_threshold: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub session_count: u64,
    pub has_index: bool,
    pub gap_threshold: u64,
    pub total_messages: u64,
    pub avg_messages_per_session: f64,
    pub min_gap_threshold: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGapThresholdRequest {
    pub gap_threshold: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionItem {
    pub id: u64,
    pub start_ts: u64,
    pub end_ts: u64,
    pub message_count: u64,
    pub first_message_id: u64,
    pub summary: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSummaryRequest {
    pub locale: Option<String>,
    #[serde(default)]
    pub force_regenerate: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSummaryResponse {
    pub success: bool,
    pub summary: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSummariesRequest {
    pub chat_session_ids: Vec<u64>,
    pub locale: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateSummariesResponse {
    pub success: u64,
    pub failed: u64,
    pub skipped: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckCanGenerateSummaryRequest {
    pub chat_session_ids: Vec<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckCanGenerateSummaryResponse {
    pub results: std::collections::HashMap<u64, CheckResult>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    pub can_generate: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetByTimeRangeRequest {
    pub start_ts: u64,
    pub end_ts: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetRecentRequest {
    pub limit: Option<u64>,
}

#[derive(Debug, FromRow)]
struct MessagePointRow {
    id: i64,
    ts: i64,
}

#[derive(Debug, FromRow)]
struct ChatSessionRow {
    id: i64,
    start_ts: i64,
    end_ts: i64,
    message_count: Option<i64>,
    first_message_id: Option<i64>,
    summary: Option<String>,
}

#[derive(Debug, FromRow)]
struct SummaryMessageRow {
    ts: i64,
    sender_name: String,
    content: Option<String>,
}

#[derive(Debug)]
struct SessionBucket {
    start_ts: i64,
    end_ts: i64,
    message_ids: Vec<i64>,
}

fn parse_meta_id(session_id: &str) -> Result<i64, ApiError> {
    session_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("Invalid session ID".to_string()))
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

async fn get_pool() -> Result<Arc<SqlitePool>, ApiError> {
    crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))
}

async fn ensure_chat_exists(pool: &SqlitePool, meta_id: i64) -> Result<(), ApiError> {
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM meta WHERE id = ?1")
        .bind(meta_id)
        .fetch_one(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    if exists == 0 {
        return Err(ApiError::NotFound("Session not found".to_string()));
    }
    Ok(())
}

async fn clear_sessions_for_meta(pool: &SqlitePool, meta_id: i64) -> Result<(), ApiError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    sqlx::query(
        r#"
        DELETE FROM message_context
        WHERE session_id IN (SELECT id FROM chat_session WHERE meta_id = ?1)
        "#,
    )
    .bind(meta_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    sqlx::query("DELETE FROM chat_session WHERE meta_id = ?1")
        .bind(meta_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    Ok(())
}

async fn resolve_gap_threshold(
    pool: &SqlitePool,
    meta_id: i64,
    requested: Option<u64>,
) -> Result<i64, ApiError> {
    if let Some(v) = requested {
        let effective = (v as i64).max(1);
        sqlx::query("UPDATE meta SET session_gap_threshold = ?1 WHERE id = ?2")
            .bind(effective)
            .bind(meta_id)
            .execute(pool)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
        return Ok(effective);
    }

    let stored = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(session_gap_threshold, ?2) FROM meta WHERE id = ?1",
    )
    .bind(meta_id)
    .bind(DEFAULT_GAP_THRESHOLD)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(stored.unwrap_or(DEFAULT_GAP_THRESHOLD).max(1))
}

async fn load_message_points(
    pool: &SqlitePool,
    meta_id: i64,
) -> Result<Vec<MessagePointRow>, ApiError> {
    sqlx::query_as::<_, MessagePointRow>(
        r#"
        SELECT id, ts
        FROM message
        WHERE meta_id = ?1
        ORDER BY ts ASC, id ASC
        "#,
    )
    .bind(meta_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))
}

fn build_session_buckets(messages: &[MessagePointRow], gap_threshold: i64) -> Vec<SessionBucket> {
    if messages.is_empty() {
        return Vec::new();
    }

    let mut buckets = Vec::new();
    let mut current = SessionBucket {
        start_ts: messages[0].ts,
        end_ts: messages[0].ts,
        message_ids: vec![messages[0].id],
    };
    let mut prev_ts = messages[0].ts;

    for msg in messages.iter().skip(1) {
        let need_new = msg.ts - prev_ts > gap_threshold;
        if need_new {
            buckets.push(current);
            current = SessionBucket {
                start_ts: msg.ts,
                end_ts: msg.ts,
                message_ids: vec![msg.id],
            };
        } else {
            current.end_ts = msg.ts;
            current.message_ids.push(msg.id);
        }
        prev_ts = msg.ts;
    }
    buckets.push(current);
    buckets
}

async fn persist_session_buckets(
    pool: &SqlitePool,
    meta_id: i64,
    buckets: &[SessionBucket],
) -> Result<u64, ApiError> {
    if buckets.is_empty() {
        return Ok(0);
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    for bucket in buckets {
        let inserted = sqlx::query(
            r#"
            INSERT INTO chat_session (meta_id, start_ts, end_ts, message_count, is_manual, summary)
            VALUES (?1, ?2, ?3, ?4, 0, NULL)
            "#,
        )
        .bind(meta_id)
        .bind(bucket.start_ts)
        .bind(bucket.end_ts)
        .bind(bucket.message_ids.len() as i64)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
        let chat_session_id = inserted.last_insert_rowid();

        for message_id in &bucket.message_ids {
            sqlx::query(
                r#"
                INSERT INTO message_context (message_id, session_id, topic_id)
                VALUES (?1, ?2, NULL)
                "#,
            )
            .bind(*message_id)
            .bind(chat_session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
        }
    }

    tx.commit()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(buckets.len() as u64)
}

fn to_chat_session_item(row: ChatSessionRow) -> ChatSessionItem {
    ChatSessionItem {
        id: row.id.max(0) as u64,
        start_ts: row.start_ts.max(0) as u64,
        end_ts: row.end_ts.max(0) as u64,
        message_count: row.message_count.unwrap_or(0).max(0) as u64,
        first_message_id: row.first_message_id.unwrap_or(0).max(0) as u64,
        summary: row.summary,
    }
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let char_count = input.chars().count();
    if char_count <= max_chars {
        return input.to_string();
    }
    input.chars().take(max_chars).collect::<String>()
}

fn normalize_locale(locale: &str) -> &str {
    if locale.starts_with("en") {
        "en"
    } else {
        "zh-CN"
    }
}

fn summary_too_few_reason(locale: &str) -> String {
    if normalize_locale(locale) == "en" {
        format!(
            "This session has fewer than {} meaningful messages.",
            MIN_SUMMARY_MESSAGES
        )
    } else {
        format!("该会话有效消息少于{}条，无需生成摘要", MIN_SUMMARY_MESSAGES)
    }
}

fn build_local_summary(locale: &str, rows: &[SummaryMessageRow]) -> Result<String, String> {
    let meaningful: Vec<&SummaryMessageRow> = rows
        .iter()
        .filter(|m| {
            m.content
                .as_deref()
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false)
        })
        .collect();

    if meaningful.len() < MIN_SUMMARY_MESSAGES {
        return Err(summary_too_few_reason(locale));
    }

    let mut participant_counts: HashMap<String, usize> = HashMap::new();
    for row in rows {
        *participant_counts
            .entry(row.sender_name.clone())
            .or_insert(0) += 1;
    }
    let mut participants: Vec<(String, usize)> = participant_counts.into_iter().collect();
    participants.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let top_participants: Vec<String> = participants
        .into_iter()
        .take(3)
        .map(|(name, _)| name)
        .collect();
    let participant_text = if top_participants.is_empty() {
        if normalize_locale(locale) == "en" {
            "multiple participants".to_string()
        } else {
            "多位成员".to_string()
        }
    } else if normalize_locale(locale) == "en" {
        top_participants.join(", ")
    } else {
        top_participants.join("、")
    };

    let mut highlights = Vec::new();
    for row in meaningful.iter().take(8) {
        if let Some(content) = row.content.as_deref() {
            let cleaned = content.trim().replace('\n', " ");
            if cleaned.is_empty() {
                continue;
            }
            let snippet = truncate_chars(&cleaned, 36);
            if !highlights.iter().any(|h: &String| h == &snippet) {
                highlights.push(snippet);
            }
        }
        if highlights.len() >= 2 {
            break;
        }
    }

    let mut summary = if normalize_locale(locale) == "en" {
        let detail = if highlights.is_empty() {
            "Mainly routine discussion.".to_string()
        } else {
            format!("Key points: {}.", highlights.join("; "))
        };
        format!(
            "{} messages, mainly from {}. {}",
            rows.len(),
            participant_text,
            detail
        )
    } else {
        let detail = if highlights.is_empty() {
            "主要围绕日常交流展开。".to_string()
        } else {
            format!("主要内容：{}。", highlights.join("；"))
        };
        format!(
            "共{}条消息，主要参与者：{}。{}",
            rows.len(),
            participant_text,
            detail
        )
    };

    let max_len = 220;
    if summary.chars().count() > max_len {
        summary = format!("{}...", truncate_chars(&summary, max_len.saturating_sub(3)));
    }

    Ok(summary)
}

async fn load_summary_messages(
    pool: &SqlitePool,
    meta_id: i64,
    chat_session_id: i64,
) -> Result<Vec<SummaryMessageRow>, ApiError> {
    sqlx::query_as::<_, SummaryMessageRow>(
        r#"
        SELECT
            m.ts,
            COALESCE(
                m.sender_group_nickname,
                m.sender_account_name,
                mb.group_nickname,
                mb.account_name,
                mb.platform_id,
                'unknown'
            ) as sender_name,
            m.content
        FROM message_context mc
        JOIN message m ON m.id = mc.message_id
        LEFT JOIN member mb ON mb.id = m.sender_id
        WHERE mc.session_id = ?1
          AND m.meta_id = ?2
        ORDER BY m.ts ASC, m.id ASC
        "#,
    )
    .bind(chat_session_id)
    .bind(meta_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))
}

async fn generate_summary_internal(
    pool: &SqlitePool,
    meta_id: i64,
    chat_session_id: i64,
    locale: &str,
    force_regenerate: bool,
) -> Result<GenerateSummaryResponse, ApiError> {
    let existing = sqlx::query_scalar::<_, Option<String>>(
        "SELECT summary FROM chat_session WHERE id = ?1 AND meta_id = ?2",
    )
    .bind(chat_session_id)
    .bind(meta_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    let existing_summary = match existing {
        None => {
            return Ok(GenerateSummaryResponse {
                success: false,
                summary: None,
                error: Some("session not found".to_string()),
            })
        }
        Some(v) => v,
    };

    if !force_regenerate {
        if let Some(summary) = existing_summary {
            if !summary.trim().is_empty() {
                return Ok(GenerateSummaryResponse {
                    success: true,
                    summary: Some(summary),
                    error: None,
                });
            }
        }
    }

    let rows = load_summary_messages(pool, meta_id, chat_session_id).await?;
    let summary = match build_local_summary(locale, &rows) {
        Ok(s) => s,
        Err(reason) => {
            return Ok(GenerateSummaryResponse {
                success: false,
                summary: None,
                error: Some(reason),
            })
        }
    };

    sqlx::query("UPDATE chat_session SET summary = ?1 WHERE id = ?2 AND meta_id = ?3")
        .bind(&summary)
        .bind(chat_session_id)
        .bind(meta_id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(GenerateSummaryResponse {
        success: true,
        summary: Some(summary),
        error: None,
    })
}

async fn check_summary_generatable(
    pool: &SqlitePool,
    meta_id: i64,
    chat_session_id: i64,
) -> Result<CheckResult, ApiError> {
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM chat_session WHERE id = ?1 AND meta_id = ?2",
    )
    .bind(chat_session_id)
    .bind(meta_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    if exists == 0 {
        return Ok(CheckResult {
            can_generate: false,
            reason: Some("会话不存在".to_string()),
        });
    }

    let valid_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM message_context mc
        JOIN message m ON m.id = mc.message_id
        WHERE mc.session_id = ?1
          AND m.meta_id = ?2
          AND LENGTH(TRIM(COALESCE(m.content, ''))) > 0
        "#,
    )
    .bind(chat_session_id)
    .bind(meta_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    if valid_count < MIN_SUMMARY_MESSAGES as i64 {
        return Ok(CheckResult {
            can_generate: false,
            reason: Some(format!("有效消息不足{}条", MIN_SUMMARY_MESSAGES)),
        });
    }

    Ok(CheckResult {
        can_generate: true,
        reason: None,
    })
}

#[axum::debug_handler]
#[instrument]
pub async fn generate(
    Path(session_id): Path<String>,
    Json(req): Json<GenerateRequest>,
) -> Result<Json<u64>, ApiError> {
    let meta_id = parse_meta_id(&session_id)?;
    let pool = get_pool().await?;
    ensure_chat_exists(&pool, meta_id).await?;

    let gap_threshold = resolve_gap_threshold(&pool, meta_id, req.gap_threshold).await?;
    let messages = load_message_points(&pool, meta_id).await?;

    clear_sessions_for_meta(&pool, meta_id).await?;

    if messages.is_empty() {
        return Ok(Json(0));
    }

    let buckets = build_session_buckets(&messages, gap_threshold);
    let session_count = persist_session_buckets(&pool, meta_id, &buckets).await?;

    Ok(Json(session_count))
}

#[axum::debug_handler]
#[instrument]
pub async fn has_index(Path(session_id): Path<String>) -> Result<Json<bool>, ApiError> {
    let meta_id = parse_meta_id(&session_id)?;
    let pool = get_pool().await?;
    ensure_chat_exists(&pool, meta_id).await?;

    let count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM chat_session WHERE meta_id = ?1")
            .bind(meta_id)
            .fetch_one(&*pool)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(count > 0))
}

#[axum::debug_handler]
#[instrument]
pub async fn get_stats(Path(session_id): Path<String>) -> Result<Json<SessionStats>, ApiError> {
    let meta_id = parse_meta_id(&session_id)?;
    let pool = get_pool().await?;
    ensure_chat_exists(&pool, meta_id).await?;

    let session_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM chat_session WHERE meta_id = ?1")
            .bind(meta_id)
            .fetch_one(&*pool)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

    let total_messages =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
            .bind(meta_id)
            .fetch_one(&*pool)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

    let gap_threshold = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(session_gap_threshold, ?2) FROM meta WHERE id = ?1",
    )
    .bind(meta_id)
    .bind(DEFAULT_GAP_THRESHOLD)
    .fetch_optional(&*pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?
    .unwrap_or(DEFAULT_GAP_THRESHOLD)
    .max(1) as u64;

    let session_count_u64 = session_count.max(0) as u64;
    let total_messages_u64 = total_messages.max(0) as u64;
    let avg_messages_per_session = if session_count_u64 > 0 {
        total_messages_u64 as f64 / session_count_u64 as f64
    } else {
        0.0
    };

    Ok(Json(SessionStats {
        session_count: session_count_u64,
        has_index: session_count_u64 > 0,
        gap_threshold,
        total_messages: total_messages_u64,
        avg_messages_per_session,
        min_gap_threshold: Some(gap_threshold),
    }))
}

#[axum::debug_handler]
#[instrument]
pub async fn clear(Path(session_id): Path<String>) -> Result<Json<bool>, ApiError> {
    let meta_id = parse_meta_id(&session_id)?;
    let pool = get_pool().await?;
    ensure_chat_exists(&pool, meta_id).await?;
    clear_sessions_for_meta(&pool, meta_id).await?;
    Ok(Json(true))
}

#[axum::debug_handler]
#[instrument]
pub async fn update_gap_threshold(
    Path(session_id): Path<String>,
    Json(req): Json<UpdateGapThresholdRequest>,
) -> Result<Json<bool>, ApiError> {
    let meta_id = parse_meta_id(&session_id)?;
    let pool = get_pool().await?;
    ensure_chat_exists(&pool, meta_id).await?;

    let value = req.gap_threshold.map(|v| (v as i64).max(1));
    sqlx::query("UPDATE meta SET session_gap_threshold = ?1 WHERE id = ?2")
        .bind(value)
        .bind(meta_id)
        .execute(&*pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(true))
}

#[axum::debug_handler]
#[instrument]
pub async fn get_sessions(
    Path(session_id): Path<String>,
) -> Result<Json<Vec<ChatSessionItem>>, ApiError> {
    let meta_id = parse_meta_id(&session_id)?;
    let pool = get_pool().await?;
    ensure_chat_exists(&pool, meta_id).await?;

    let rows = sqlx::query_as::<_, ChatSessionRow>(
        r#"
        SELECT
            cs.id,
            cs.start_ts,
            cs.end_ts,
            cs.message_count,
            cs.summary,
            (
              SELECT mc.message_id
              FROM message_context mc
              WHERE mc.session_id = cs.id
              ORDER BY mc.message_id ASC
              LIMIT 1
            ) as first_message_id
        FROM chat_session cs
        WHERE cs.meta_id = ?1
        ORDER BY cs.start_ts ASC
        "#,
    )
    .bind(meta_id)
    .fetch_all(&*pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(rows.into_iter().map(to_chat_session_item).collect()))
}

#[axum::debug_handler]
#[instrument]
pub async fn generate_summary(
    Path((session_id, chat_session_id)): Path<(String, u64)>,
    Json(req): Json<GenerateSummaryRequest>,
) -> Result<Json<GenerateSummaryResponse>, ApiError> {
    let meta_id = parse_meta_id(&session_id)?;
    let pool = get_pool().await?;
    ensure_chat_exists(&pool, meta_id).await?;

    let locale = req.locale.unwrap_or_else(|| "zh-CN".to_string());
    let result = generate_summary_internal(
        &pool,
        meta_id,
        chat_session_id as i64,
        &locale,
        req.force_regenerate,
    )
    .await?;
    Ok(Json(result))
}

#[axum::debug_handler]
#[instrument]
pub async fn generate_summaries(
    Path(session_id): Path<String>,
    Json(req): Json<GenerateSummariesRequest>,
) -> Result<Json<GenerateSummariesResponse>, ApiError> {
    let meta_id = parse_meta_id(&session_id)?;
    let pool = get_pool().await?;
    ensure_chat_exists(&pool, meta_id).await?;

    let locale = req.locale.unwrap_or_else(|| "zh-CN".to_string());
    let mut success = 0u64;
    let mut failed = 0u64;
    let mut skipped = 0u64;

    for chat_session_id in req.chat_session_ids {
        let result =
            generate_summary_internal(&pool, meta_id, chat_session_id as i64, &locale, false)
                .await?;
        if result.success {
            success += 1;
        } else if result
            .error
            .as_deref()
            .map(|e| e.contains("少于") || e.contains("less than"))
            .unwrap_or(false)
        {
            skipped += 1;
        } else {
            failed += 1;
        }
    }

    Ok(Json(GenerateSummariesResponse {
        success,
        failed,
        skipped,
    }))
}

#[axum::debug_handler]
#[instrument]
pub async fn check_can_generate_summary(
    Path(session_id): Path<String>,
    Json(req): Json<CheckCanGenerateSummaryRequest>,
) -> Result<Json<HashMap<u64, CheckResult>>, ApiError> {
    let meta_id = parse_meta_id(&session_id)?;
    let pool = get_pool().await?;
    ensure_chat_exists(&pool, meta_id).await?;

    let mut results = HashMap::new();
    for chat_session_id in req.chat_session_ids {
        let check = check_summary_generatable(&pool, meta_id, chat_session_id as i64).await?;
        results.insert(chat_session_id, check);
    }
    Ok(Json(results))
}

#[axum::debug_handler]
#[instrument]
pub async fn get_by_time_range(
    Path(session_id): Path<String>,
    Query(req): Query<GetByTimeRangeRequest>,
) -> Result<Json<Vec<ChatSessionItem>>, ApiError> {
    if req.start_ts > req.end_ts {
        return Err(ApiError::InvalidRequest(
            "startTs must be less than or equal to endTs".to_string(),
        ));
    }

    let meta_id = parse_meta_id(&session_id)?;
    let pool = get_pool().await?;
    ensure_chat_exists(&pool, meta_id).await?;

    let rows = sqlx::query_as::<_, ChatSessionRow>(
        r#"
        SELECT
            cs.id,
            cs.start_ts,
            cs.end_ts,
            cs.message_count,
            cs.summary,
            (
              SELECT mc.message_id
              FROM message_context mc
              WHERE mc.session_id = cs.id
              ORDER BY mc.message_id ASC
              LIMIT 1
            ) as first_message_id
        FROM chat_session cs
        WHERE cs.meta_id = ?1
          AND cs.start_ts >= ?2
          AND cs.start_ts <= ?3
        ORDER BY cs.start_ts DESC
        "#,
    )
    .bind(meta_id)
    .bind(req.start_ts as i64)
    .bind(req.end_ts as i64)
    .fetch_all(&*pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(rows.into_iter().map(to_chat_session_item).collect()))
}

#[axum::debug_handler]
#[instrument]
pub async fn get_recent(
    Path(session_id): Path<String>,
    Query(req): Query<GetRecentRequest>,
) -> Result<Json<Vec<ChatSessionItem>>, ApiError> {
    let meta_id = parse_meta_id(&session_id)?;
    let pool = get_pool().await?;
    ensure_chat_exists(&pool, meta_id).await?;

    let limit = req.limit.unwrap_or(20).max(1).min(500) as i64;

    let rows = sqlx::query_as::<_, ChatSessionRow>(
        r#"
        SELECT
            cs.id,
            cs.start_ts,
            cs.end_ts,
            cs.message_count,
            cs.summary,
            (
              SELECT mc.message_id
              FROM message_context mc
              WHERE mc.session_id = cs.id
              ORDER BY mc.message_id ASC
              LIMIT 1
            ) as first_message_id
        FROM chat_session cs
        WHERE cs.meta_id = ?1
        ORDER BY cs.start_ts DESC
        LIMIT ?2
        "#,
    )
    .bind(meta_id)
    .bind(limit)
    .fetch_all(&*pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(rows.into_iter().map(to_chat_session_item).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_session_buckets_gap_split() {
        let messages = vec![
            MessagePointRow { id: 1, ts: 100 },
            MessagePointRow { id: 2, ts: 120 },
            MessagePointRow { id: 3, ts: 900 },
            MessagePointRow { id: 4, ts: 950 },
        ];

        let buckets = build_session_buckets(&messages, 300);
        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].message_ids, vec![1, 2]);
        assert_eq!(buckets[1].message_ids, vec![3, 4]);
    }

    #[test]
    fn test_build_local_summary_requires_meaningful_messages() {
        let rows = vec![
            SummaryMessageRow {
                ts: 1,
                sender_name: "A".to_string(),
                content: Some("".to_string()),
            },
            SummaryMessageRow {
                ts: 2,
                sender_name: "B".to_string(),
                content: Some("   ".to_string()),
            },
        ];

        let result = build_local_summary("zh-CN", &rows);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_local_summary_success() {
        let rows = vec![
            SummaryMessageRow {
                ts: 1,
                sender_name: "Alice".to_string(),
                content: Some("今天讨论发布计划".to_string()),
            },
            SummaryMessageRow {
                ts: 2,
                sender_name: "Bob".to_string(),
                content: Some("我负责测试和回归".to_string()),
            },
            SummaryMessageRow {
                ts: 3,
                sender_name: "Alice".to_string(),
                content: Some("晚上前提交最终版本".to_string()),
            },
        ];

        let result = build_local_summary("zh-CN", &rows).expect("summary should be generated");
        assert!(result.contains("共3条消息"));
        assert!(result.contains("Alice"));
    }
}

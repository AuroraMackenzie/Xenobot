//! AI API module for Xenobot HTTP API.
//!
//! Provides HTTP endpoints equivalent to Xenobot's `aiApi` IPC methods.

use axum::{
    extract::{Path, Query},
    response::{sse::Event, Sse},
    routing::{delete, get, post},
    Json, Router,
};
use futures::stream;
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Row, SqlitePool};
use std::{
    collections::{BTreeSet, HashSet},
    convert::Infallible,
    fs,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::instrument;

use crate::ApiError;

/// AI API router.
pub fn router() -> Router {
    Router::new()
        .route("/search-messages", post(search_messages))
        .route("/message-context", post(get_message_context))
        .route("/recent-messages", post(get_recent_messages))
        .route("/all-recent-messages", post(get_all_recent_messages))
        .route("/conversation-between", post(get_conversation_between))
        .route("/messages-before", post(get_messages_before))
        .route("/messages-after", post(get_messages_after))
        .route(
            "/filter-messages-with-context",
            post(filter_messages_with_context),
        )
        .route(
            "/multiple-sessions-messages",
            post(get_multiple_sessions_messages),
        )
        .route(
            "/export-filter-result-to-file",
            post(export_filter_result_to_file),
        )
        .route("/export-progress", get(export_progress_sse))
        .route("/conversations", post(create_conversation))
        .route("/conversations", get(get_conversations))
        .route("/conversations/:conversation_id", get(get_conversation))
        .route(
            "/conversations/:conversation_id/title",
            post(update_conversation_title),
        )
        .route(
            "/conversations/:conversation_id",
            delete(delete_conversation),
        )
        .route(
            "/conversations/:conversation_id/messages",
            post(add_message),
        )
        .route(
            "/conversations/:conversation_id/messages",
            get(get_messages),
        )
        .route("/messages/:message_id", delete(delete_message))
        .route("/show-ai-log-file", get(show_ai_log_file))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SearchMessageResult {
    id: i64,
    sender_name: String,
    sender_platform_id: String,
    sender_aliases: Vec<String>,
    sender_avatar: Option<String>,
    content: String,
    timestamp: i64,
    #[serde(rename = "type")]
    msg_type: i64,
    reply_to_message_id: Option<String>,
    reply_to_content: Option<String>,
    reply_to_sender_name: Option<String>,
    is_hit: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AIConversation {
    id: String,
    session_id: String,
    title: Option<String>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AIMessage {
    id: String,
    role: String,
    content: String,
    timestamp: i64,
    data_keywords: Option<Vec<String>>,
    data_message_count: Option<u32>,
    content_blocks: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportProgress {
    stage: String,
    percentage: u32,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchMessagesRequest {
    session_id: String,
    keywords: Vec<String>,
    filter: Option<TimeFilter>,
    limit: Option<u32>,
    offset: Option<u32>,
    sender_id: Option<i32>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TimeFilter {
    start_ts: Option<i64>,
    end_ts: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetMessageContextRequest {
    session_id: String,
    message_ids: Vec<i64>,
    context_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetRecentMessagesRequest {
    session_id: String,
    filter: Option<TimeFilter>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetConversationBetweenRequest {
    session_id: String,
    member_id1: i32,
    member_id2: i32,
    filter: Option<TimeFilter>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetMessagesBeforeRequest {
    session_id: String,
    before_id: i64,
    limit: Option<u32>,
    filter: Option<TimeFilter>,
    sender_id: Option<i32>,
    keywords: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetMessagesAfterRequest {
    session_id: String,
    after_id: i64,
    limit: Option<u32>,
    filter: Option<TimeFilter>,
    sender_id: Option<i32>,
    keywords: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FilterMessagesWithContextRequest {
    session_id: String,
    keywords: Option<Vec<String>>,
    time_filter: Option<TimeFilter>,
    sender_ids: Option<Vec<i32>>,
    context_size: Option<u32>,
    page: Option<u32>,
    page_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetMultipleSessionsMessagesRequest {
    session_id: String,
    chat_session_ids: Vec<i32>,
    page: Option<u32>,
    page_size: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportFilterResultToFileRequest {
    session_id: String,
    session_name: String,
    output_dir: String,
    filter_mode: String,
    keywords: Option<Vec<String>>,
    time_filter: Option<TimeFilter>,
    sender_ids: Option<Vec<i32>>,
    context_size: Option<u32>,
    chat_session_ids: Option<Vec<i32>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateConversationRequest {
    session_id: String,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationsQuery {
    session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateConversationTitleRequest {
    title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddMessageRequest {
    role: String,
    content: String,
    data_keywords: Option<Vec<String>>,
    data_message_count: Option<u32>,
    content_blocks: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct ConversationPayload {
    title: Option<String>,
    messages: Vec<AIMessage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FilterBlock {
    start_ts: i64,
    end_ts: i64,
    messages: Vec<SearchMessageResult>,
    hit_count: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FilterStats {
    total_messages: u64,
    hit_messages: u64,
    total_chars: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PaginationInfo {
    page: u32,
    page_size: u32,
    total_blocks: u32,
    total_hits: u32,
    has_more: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FilterResponse {
    blocks: Vec<FilterBlock>,
    stats: FilterStats,
    pagination: PaginationInfo,
}

#[derive(Debug)]
struct MessageRow {
    id: i64,
    sender_name: String,
    sender_platform_id: String,
    sender_aliases_raw: Option<String>,
    sender_avatar: Option<String>,
    content: Option<String>,
    timestamp: i64,
    msg_type: i64,
    reply_to_message_id: Option<String>,
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

fn parse_meta_id(value: &str) -> Result<i64, ApiError> {
    value
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("invalid session_id".to_string()))
}

async fn get_pool() -> Result<Arc<SqlitePool>, ApiError> {
    crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))
}

fn parse_aliases(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    if raw.trim().is_empty() {
        return Vec::new();
    }
    if let Ok(v) = serde_json::from_str::<Vec<String>>(raw) {
        return v;
    }
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn normalize_keywords(keywords: Option<Vec<String>>) -> Vec<String> {
    keywords
        .unwrap_or_default()
        .into_iter()
        .map(|k| k.trim().to_ascii_lowercase())
        .filter(|k| !k.is_empty())
        .collect()
}

fn message_is_hit(msg: &SearchMessageResult, keywords: &[String]) -> bool {
    if keywords.is_empty() {
        return true;
    }
    let text = msg.content.to_ascii_lowercase();
    keywords.iter().any(|k| text.contains(k))
}

fn row_to_search_result(row: MessageRow, is_hit: bool) -> SearchMessageResult {
    SearchMessageResult {
        id: row.id,
        sender_name: row.sender_name,
        sender_platform_id: row.sender_platform_id,
        sender_aliases: parse_aliases(row.sender_aliases_raw.as_deref()),
        sender_avatar: row.sender_avatar,
        content: row.content.unwrap_or_default(),
        timestamp: row.timestamp,
        msg_type: row.msg_type,
        reply_to_message_id: row.reply_to_message_id,
        reply_to_content: None,
        reply_to_sender_name: None,
        is_hit,
    }
}

async fn query_messages(
    pool: &SqlitePool,
    meta_id: i64,
    filter: Option<&TimeFilter>,
    sender_ids: Option<&[i32]>,
    keywords: &[String],
    limit: Option<u32>,
    offset: Option<u32>,
    before_id: Option<i64>,
    after_id: Option<i64>,
    descending: bool,
) -> Result<Vec<SearchMessageResult>, ApiError> {
    let mut qb: QueryBuilder<sqlx::Sqlite> = QueryBuilder::new(
        r#"
        SELECT
            msg.id as id,
            COALESCE(msg.sender_group_nickname, msg.sender_account_name, m.group_nickname, m.account_name, m.platform_id, '') as sender_name,
            COALESCE(m.platform_id, '') as sender_platform_id,
            m.aliases as sender_aliases_raw,
            m.avatar as sender_avatar,
            msg.content as content,
            msg.ts as timestamp,
            msg.msg_type as msg_type,
            msg.reply_to_message_id as reply_to_message_id
        FROM message msg
        LEFT JOIN member m ON m.id = msg.sender_id
        WHERE msg.meta_id = "#,
    );
    qb.push_bind(meta_id);

    if let Some(filter) = filter {
        if let Some(start_ts) = filter.start_ts {
            qb.push(" AND msg.ts >= ").push_bind(start_ts);
        }
        if let Some(end_ts) = filter.end_ts {
            qb.push(" AND msg.ts <= ").push_bind(end_ts);
        }
    }

    if let Some(sender_ids) = sender_ids {
        if !sender_ids.is_empty() {
            qb.push(" AND msg.sender_id IN (");
            let mut separated = qb.separated(", ");
            for sender_id in sender_ids {
                separated.push_bind(*sender_id);
            }
            separated.push_unseparated(")");
        }
    }

    if let Some(before_id) = before_id {
        qb.push(" AND msg.id < ").push_bind(before_id);
    }
    if let Some(after_id) = after_id {
        qb.push(" AND msg.id > ").push_bind(after_id);
    }

    if !keywords.is_empty() {
        qb.push(" AND (");
        let mut separated = qb.separated(" OR ");
        for keyword in keywords {
            separated.push("LOWER(COALESCE(msg.content, '')) LIKE ");
            separated.push_bind(format!("%{}%", keyword));
        }
        separated.push_unseparated(")");
    }

    qb.push(" ORDER BY msg.ts ");
    if descending {
        qb.push("DESC");
    } else {
        qb.push("ASC");
    }
    qb.push(", msg.id ");
    if descending {
        qb.push("DESC");
    } else {
        qb.push("ASC");
    }

    if let Some(limit) = limit {
        qb.push(" LIMIT ").push_bind(limit as i64);
    }
    if let Some(offset) = offset {
        qb.push(" OFFSET ").push_bind(offset as i64);
    }

    let rows = qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|row| {
            row_to_search_result(
                MessageRow {
                    id: row.try_get("id").unwrap_or(0),
                    sender_name: row.try_get("sender_name").unwrap_or_default(),
                    sender_platform_id: row.try_get("sender_platform_id").unwrap_or_default(),
                    sender_aliases_raw: row.try_get("sender_aliases_raw").ok(),
                    sender_avatar: row.try_get("sender_avatar").ok(),
                    content: row.try_get("content").ok(),
                    timestamp: row.try_get("timestamp").unwrap_or(0),
                    msg_type: row.try_get("msg_type").unwrap_or(0),
                    reply_to_message_id: row.try_get("reply_to_message_id").ok(),
                },
                true,
            )
        })
        .collect())
}

fn paginate_blocks(
    blocks: Vec<FilterBlock>,
    page: u32,
    page_size: u32,
    total_hits: u32,
) -> FilterResponse {
    let total_blocks = blocks.len() as u32;
    let start = ((page.saturating_sub(1)) * page_size) as usize;
    let end = (start + page_size as usize).min(blocks.len());
    let paged = if start >= blocks.len() {
        Vec::new()
    } else {
        blocks[start..end].to_vec()
    };
    let has_more = end < blocks.len();

    let mut total_messages = 0_u64;
    let mut total_chars = 0_u64;
    for block in &blocks {
        total_messages += block.messages.len() as u64;
        for msg in &block.messages {
            total_chars += msg.content.chars().count() as u64;
        }
    }

    FilterResponse {
        blocks: paged,
        stats: FilterStats {
            total_messages,
            hit_messages: total_hits as u64,
            total_chars,
        },
        pagination: PaginationInfo {
            page,
            page_size,
            total_blocks,
            total_hits,
            has_more,
        },
    }
}

fn build_context_blocks(
    mut messages: Vec<SearchMessageResult>,
    keywords: &[String],
    context_size: usize,
) -> (Vec<FilterBlock>, u32) {
    messages.sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then_with(|| a.id.cmp(&b.id)));
    if messages.is_empty() {
        return (Vec::new(), 0);
    }

    let hit_indices: Vec<usize> = messages
        .iter()
        .enumerate()
        .filter_map(|(idx, msg)| message_is_hit(msg, keywords).then_some(idx))
        .collect();
    if hit_indices.is_empty() {
        return (Vec::new(), 0);
    }

    let mut ranges = Vec::new();
    for idx in &hit_indices {
        let start = idx.saturating_sub(context_size);
        let end = (*idx + context_size).min(messages.len().saturating_sub(1));
        ranges.push((start, end));
    }
    ranges.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in ranges {
        if let Some((_, last_end)) = merged.last_mut() {
            if start <= *last_end + 1 {
                *last_end = (*last_end).max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    let hit_set: HashSet<usize> = hit_indices.into_iter().collect();
    let mut blocks = Vec::new();
    for (start, end) in merged {
        let mut block_messages = Vec::new();
        let mut hit_count = 0_u64;
        for idx in start..=end {
            let mut msg = messages[idx].clone();
            let is_hit = hit_set.contains(&idx);
            msg.is_hit = is_hit;
            if is_hit {
                hit_count += 1;
            }
            block_messages.push(msg);
        }
        let start_ts = block_messages.first().map(|m| m.timestamp).unwrap_or(0);
        let end_ts = block_messages.last().map(|m| m.timestamp).unwrap_or(0);
        blocks.push(FilterBlock {
            start_ts,
            end_ts,
            messages: block_messages,
            hit_count,
        });
    }

    let total_hits = hit_set.len() as u32;
    (blocks, total_hits)
}

fn sanitize_filename(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if matches!(
            ch,
            '/' | '\\' | '?' | '%' | '*' | ':' | '|' | '"' | '<' | '>'
        ) {
            out.push('_');
        } else {
            out.push(ch);
        }
    }
    out.trim().to_string()
}

async fn load_conversation_payload(
    pool: &SqlitePool,
    conversation_id: &str,
) -> Result<(i64, String, i64, i64, ConversationPayload), ApiError> {
    let conv_id = conversation_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("invalid conversation id".to_string()))?;
    let row = sqlx::query(
        "SELECT id, session_id, messages, created_at, updated_at FROM conversations WHERE id = ?1",
    )
    .bind(conv_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?
    .ok_or_else(|| ApiError::NotFound("conversation not found".to_string()))?;

    let id: i64 = row.try_get("id").unwrap_or(conv_id);
    let session_id: String = row.try_get("session_id").unwrap_or_default();
    let messages_json: String = row.try_get("messages").unwrap_or_else(|_| "{}".to_string());
    let created_at: i64 = row.try_get("created_at").unwrap_or(now_ts());
    let updated_at: i64 = row.try_get("updated_at").unwrap_or(now_ts());

    let payload = if messages_json.trim().starts_with('{') {
        serde_json::from_str::<ConversationPayload>(&messages_json).unwrap_or_default()
    } else {
        ConversationPayload::default()
    };
    Ok((id, session_id, created_at, updated_at, payload))
}

#[instrument]
async fn search_messages(
    Json(req): Json<SearchMessagesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = parse_meta_id(&req.session_id)?;
    let pool = get_pool().await?;
    let keywords = normalize_keywords(Some(req.keywords));
    let limit = req.limit.unwrap_or(100).min(500);
    let offset = req.offset.unwrap_or(0);
    let sender_ids = req.sender_id.map(|v| vec![v]);

    let messages = query_messages(
        pool.as_ref(),
        meta_id,
        req.filter.as_ref(),
        sender_ids.as_deref(),
        &keywords,
        Some(limit),
        Some(offset),
        None,
        None,
        false,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "messages": messages,
        "count": messages.len(),
    })))
}

#[instrument]
async fn get_message_context(
    Json(req): Json<GetMessageContextRequest>,
) -> Result<Json<Vec<SearchMessageResult>>, ApiError> {
    let meta_id = parse_meta_id(&req.session_id)?;
    let pool = get_pool().await?;
    let context_size = req.context_size.unwrap_or(3).min(50) as i64;

    let mut output = Vec::<SearchMessageResult>::new();
    let mut seen = BTreeSet::new();
    for msg_id in req.message_ids {
        let before = query_messages(
            pool.as_ref(),
            meta_id,
            None,
            None,
            &[],
            Some(context_size as u32),
            None,
            Some(msg_id),
            None,
            true,
        )
        .await?;
        for msg in before.into_iter().rev() {
            if seen.insert(msg.id) {
                output.push(msg);
            }
        }

        let target = query_messages(
            pool.as_ref(),
            meta_id,
            None,
            None,
            &[],
            Some(1),
            None,
            None,
            Some(msg_id - 1),
            false,
        )
        .await?;
        for msg in target {
            if msg.id == msg_id && seen.insert(msg.id) {
                output.push(msg);
            }
        }

        if context_size > 0 {
            let after = query_messages(
                pool.as_ref(),
                meta_id,
                None,
                None,
                &[],
                Some(context_size as u32),
                None,
                None,
                Some(msg_id),
                false,
            )
            .await?;
            for msg in after {
                if seen.insert(msg.id) {
                    output.push(msg);
                }
            }
        }
    }

    output.sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then_with(|| a.id.cmp(&b.id)));
    Ok(Json(output))
}

#[instrument]
async fn get_recent_messages(
    Json(req): Json<GetRecentMessagesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = parse_meta_id(&req.session_id)?;
    let pool = get_pool().await?;
    let limit = req.limit.unwrap_or(50).min(500);
    let mut messages = query_messages(
        pool.as_ref(),
        meta_id,
        req.filter.as_ref(),
        None,
        &[],
        Some(limit),
        None,
        None,
        None,
        true,
    )
    .await?;
    messages.reverse();

    Ok(Json(serde_json::json!({
        "messages": messages,
        "count": messages.len(),
    })))
}

#[instrument]
async fn get_all_recent_messages(
    Json(req): Json<GetRecentMessagesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = parse_meta_id(&req.session_id)?;
    let pool = get_pool().await?;
    let limit = req.limit.unwrap_or(100).min(1000);
    let mut messages = query_messages(
        pool.as_ref(),
        meta_id,
        req.filter.as_ref(),
        None,
        &[],
        Some(limit),
        None,
        None,
        None,
        true,
    )
    .await?;
    messages.reverse();

    Ok(Json(serde_json::json!({
        "messages": messages,
        "count": messages.len(),
    })))
}

#[instrument]
async fn get_conversation_between(
    Json(req): Json<GetConversationBetweenRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = parse_meta_id(&req.session_id)?;
    let pool = get_pool().await?;
    let limit = req.limit.unwrap_or(200).min(2000);
    let sender_ids = vec![req.member_id1, req.member_id2];
    let messages = query_messages(
        pool.as_ref(),
        meta_id,
        req.filter.as_ref(),
        Some(&sender_ids),
        &[],
        Some(limit),
        None,
        None,
        None,
        false,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "messages": messages,
        "count": messages.len(),
    })))
}

#[instrument]
async fn get_messages_before(
    Json(req): Json<GetMessagesBeforeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = parse_meta_id(&req.session_id)?;
    let pool = get_pool().await?;
    let keywords = normalize_keywords(req.keywords);
    let limit = req.limit.unwrap_or(50).min(200);
    let sender_ids = req.sender_id.map(|v| vec![v]);

    let mut messages = query_messages(
        pool.as_ref(),
        meta_id,
        req.filter.as_ref(),
        sender_ids.as_deref(),
        &keywords,
        Some(limit + 1),
        None,
        Some(req.before_id),
        None,
        true,
    )
    .await?;
    let has_more = messages.len() > limit as usize;
    messages.truncate(limit as usize);
    messages.reverse();

    Ok(Json(serde_json::json!({
        "messages": messages,
        "hasMore": has_more,
    })))
}

#[instrument]
async fn get_messages_after(
    Json(req): Json<GetMessagesAfterRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = parse_meta_id(&req.session_id)?;
    let pool = get_pool().await?;
    let keywords = normalize_keywords(req.keywords);
    let limit = req.limit.unwrap_or(50).min(200);
    let sender_ids = req.sender_id.map(|v| vec![v]);

    let mut messages = query_messages(
        pool.as_ref(),
        meta_id,
        req.filter.as_ref(),
        sender_ids.as_deref(),
        &keywords,
        Some(limit + 1),
        None,
        None,
        Some(req.after_id),
        false,
    )
    .await?;
    let has_more = messages.len() > limit as usize;
    messages.truncate(limit as usize);

    Ok(Json(serde_json::json!({
        "messages": messages,
        "hasMore": has_more,
    })))
}

#[instrument]
async fn filter_messages_with_context(
    Json(req): Json<FilterMessagesWithContextRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = parse_meta_id(&req.session_id)?;
    let pool = get_pool().await?;
    let keywords = normalize_keywords(req.keywords.clone());
    let page = req.page.unwrap_or(1).max(1);
    let page_size = req.page_size.unwrap_or(50).clamp(1, 200);
    let context_size = req.context_size.unwrap_or(10).min(200) as usize;

    let sender_ids = req.sender_ids.clone();
    let messages = query_messages(
        pool.as_ref(),
        meta_id,
        req.time_filter.as_ref(),
        sender_ids.as_deref(),
        &[],
        Some(10_000),
        None,
        None,
        None,
        false,
    )
    .await?;

    let (blocks, total_hits) = build_context_blocks(messages, &keywords, context_size);
    let response = paginate_blocks(blocks, page, page_size, total_hits);
    Ok(Json(serde_json::to_value(response)?))
}

async fn build_blocks_from_chat_sessions(
    pool: &SqlitePool,
    meta_id: i64,
    chat_session_ids: &[i32],
) -> Result<Vec<FilterBlock>, ApiError> {
    if chat_session_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut qb: QueryBuilder<sqlx::Sqlite> =
        QueryBuilder::new("SELECT id, start_ts, end_ts FROM chat_session WHERE meta_id = ");
    qb.push_bind(meta_id);
    qb.push(" AND id IN (");
    let mut separated = qb.separated(", ");
    for id in chat_session_ids {
        separated.push_bind(*id);
    }
    separated.push_unseparated(") ORDER BY start_ts ASC, id ASC");

    let sessions = qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let mut blocks = Vec::new();
    for session in sessions {
        let chat_session_id: i64 = session.try_get("id").unwrap_or(0);
        let start_ts: i64 = session.try_get("start_ts").unwrap_or(0);
        let end_ts: i64 = session.try_get("end_ts").unwrap_or(0);

        let mut qb_msg: QueryBuilder<sqlx::Sqlite> = QueryBuilder::new(
            r#"
            SELECT
                msg.id as id,
                COALESCE(msg.sender_group_nickname, msg.sender_account_name, m.group_nickname, m.account_name, m.platform_id, '') as sender_name,
                COALESCE(m.platform_id, '') as sender_platform_id,
                m.aliases as sender_aliases_raw,
                m.avatar as sender_avatar,
                msg.content as content,
                msg.ts as timestamp,
                msg.msg_type as msg_type,
                msg.reply_to_message_id as reply_to_message_id
            FROM message msg
            LEFT JOIN member m ON m.id = msg.sender_id
            LEFT JOIN message_context ctx ON ctx.message_id = msg.id
            WHERE msg.meta_id = "#,
        );
        qb_msg.push_bind(meta_id);
        qb_msg
            .push(" AND (ctx.session_id = ")
            .push_bind(chat_session_id)
            .push(" OR (msg.ts >= ")
            .push_bind(start_ts)
            .push(" AND msg.ts <= ")
            .push_bind(end_ts)
            .push(")) ORDER BY msg.ts ASC, msg.id ASC");

        let rows = qb_msg
            .build()
            .fetch_all(pool)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
        let messages: Vec<SearchMessageResult> = rows
            .into_iter()
            .map(|row| {
                row_to_search_result(
                    MessageRow {
                        id: row.try_get("id").unwrap_or(0),
                        sender_name: row.try_get("sender_name").unwrap_or_default(),
                        sender_platform_id: row.try_get("sender_platform_id").unwrap_or_default(),
                        sender_aliases_raw: row.try_get("sender_aliases_raw").ok(),
                        sender_avatar: row.try_get("sender_avatar").ok(),
                        content: row.try_get("content").ok(),
                        timestamp: row.try_get("timestamp").unwrap_or(0),
                        msg_type: row.try_get("msg_type").unwrap_or(0),
                        reply_to_message_id: row.try_get("reply_to_message_id").ok(),
                    },
                    false,
                )
            })
            .collect();

        let block_start = messages.first().map(|m| m.timestamp).unwrap_or(start_ts);
        let block_end = messages.last().map(|m| m.timestamp).unwrap_or(end_ts);
        blocks.push(FilterBlock {
            start_ts: block_start,
            end_ts: block_end,
            hit_count: 0,
            messages,
        });
    }
    Ok(blocks)
}

#[instrument]
async fn get_multiple_sessions_messages(
    Json(req): Json<GetMultipleSessionsMessagesRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = parse_meta_id(&req.session_id)?;
    let pool = get_pool().await?;
    let page = req.page.unwrap_or(1).max(1);
    let page_size = req.page_size.unwrap_or(50).clamp(1, 200);
    let blocks =
        build_blocks_from_chat_sessions(pool.as_ref(), meta_id, &req.chat_session_ids).await?;
    let response = paginate_blocks(blocks, page, page_size, 0);
    Ok(Json(serde_json::to_value(response)?))
}

#[instrument]
async fn export_filter_result_to_file(
    Json(req): Json<ExportFilterResultToFileRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let meta_id = parse_meta_id(&req.session_id)?;
    let pool = get_pool().await?;
    let blocks = if req.filter_mode.eq_ignore_ascii_case("session") {
        let ids = req.chat_session_ids.unwrap_or_default();
        build_blocks_from_chat_sessions(pool.as_ref(), meta_id, &ids).await?
    } else {
        let messages = query_messages(
            pool.as_ref(),
            meta_id,
            req.time_filter.as_ref(),
            req.sender_ids.as_deref(),
            &[],
            Some(20_000),
            None,
            None,
            None,
            false,
        )
        .await?;
        let keywords = normalize_keywords(req.keywords);
        let context_size = req.context_size.unwrap_or(10).min(200) as usize;
        let (blocks, _) = build_context_blocks(messages, &keywords, context_size);
        blocks
    };

    let output_dir = PathBuf::from(req.output_dir);
    fs::create_dir_all(&output_dir)?;
    let session_name = sanitize_filename(req.session_name.trim());
    let file_name = format!("{}_feed_pack_{}.md", session_name, now_ts());
    let output_path = output_dir.join(file_name);

    let mut lines = Vec::new();
    lines.push(format!("# {}", req.session_name));
    lines.push(String::new());
    lines.push(format!("GeneratedAt: {}", now_ts()));
    lines.push(String::new());
    for (index, block) in blocks.iter().enumerate() {
        lines.push(format!(
            "## Block {} ({} - {})",
            index + 1,
            block.start_ts,
            block.end_ts
        ));
        for msg in &block.messages {
            let marker = if msg.is_hit { "*" } else { "-" };
            lines.push(format!(
                "{} [{}] {}: {}",
                marker, msg.timestamp, msg.sender_name, msg.content
            ));
        }
        lines.push(String::new());
    }

    fs::write(&output_path, lines.join("\n"))?;
    Ok(Json(serde_json::json!({
        "success": true,
        "filePath": output_path.to_string_lossy().to_string(),
    })))
}

#[instrument]
async fn export_progress_sse() -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let progress = ExportProgress {
        stage: "done".to_string(),
        percentage: 100,
        message: "idle".to_string(),
    };
    let payload = serde_json::to_string(&progress).unwrap_or_else(|_| "{}".to_string());
    let stream = stream::iter(vec![Ok(Event::default().data(payload))]);
    Sse::new(stream)
}

#[instrument]
async fn create_conversation(
    Json(req): Json<CreateConversationRequest>,
) -> Result<Json<AIConversation>, ApiError> {
    let pool = get_pool().await?;
    let ts = now_ts();
    let payload = ConversationPayload {
        title: req.title.clone(),
        messages: Vec::new(),
    };
    let payload_json = serde_json::to_string(&payload)?;

    let session_id = req.session_id.clone();
    let res = sqlx::query(
        "INSERT INTO conversations (session_id, messages, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(session_id.clone())
    .bind(payload_json)
    .bind(ts)
    .bind(ts)
    .execute(pool.as_ref())
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    let id = res.last_insert_rowid().to_string();
    Ok(Json(AIConversation {
        id,
        session_id,
        title: req.title,
        created_at: ts,
        updated_at: ts,
    }))
}

#[instrument]
async fn get_conversations(
    Query(query): Query<ConversationsQuery>,
) -> Result<Json<Vec<AIConversation>>, ApiError> {
    let pool = get_pool().await?;
    let rows = sqlx::query(
        "SELECT id, session_id, messages, created_at, updated_at FROM conversations WHERE session_id = ?1 ORDER BY updated_at DESC, id DESC",
    )
    .bind(query.session_id)
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    let conversations = rows
        .into_iter()
        .map(|row| {
            let id: i64 = row.try_get("id").unwrap_or(0);
            let session_id: String = row.try_get("session_id").unwrap_or_default();
            let created_at: i64 = row.try_get("created_at").unwrap_or(0);
            let updated_at: i64 = row.try_get("updated_at").unwrap_or(0);
            let payload_raw: String = row.try_get("messages").unwrap_or_else(|_| "{}".to_string());
            let payload =
                serde_json::from_str::<ConversationPayload>(&payload_raw).unwrap_or_default();

            AIConversation {
                id: id.to_string(),
                session_id,
                title: payload.title,
                created_at,
                updated_at,
            }
        })
        .collect();

    Ok(Json(conversations))
}

#[instrument]
async fn get_conversation(
    Path(conversation_id): Path<String>,
) -> Result<Json<AIConversation>, ApiError> {
    let pool = get_pool().await?;
    let (id, session_id, created_at, updated_at, payload) =
        load_conversation_payload(pool.as_ref(), &conversation_id).await?;
    Ok(Json(AIConversation {
        id: id.to_string(),
        session_id,
        title: payload.title,
        created_at,
        updated_at,
    }))
}

#[instrument]
async fn update_conversation_title(
    Path(conversation_id): Path<String>,
    Json(req): Json<UpdateConversationTitleRequest>,
) -> Result<Json<bool>, ApiError> {
    let pool = get_pool().await?;
    let (id, _session_id, _created_at, _updated_at, mut payload) =
        load_conversation_payload(pool.as_ref(), &conversation_id).await?;
    payload.title = if req.title.trim().is_empty() {
        None
    } else {
        Some(req.title.trim().to_string())
    };
    let payload_json = serde_json::to_string(&payload)?;
    sqlx::query("UPDATE conversations SET messages = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(payload_json)
        .bind(now_ts())
        .bind(id)
        .execute(pool.as_ref())
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    Ok(Json(true))
}

#[instrument]
async fn delete_conversation(Path(conversation_id): Path<String>) -> Result<Json<bool>, ApiError> {
    let conv_id = conversation_id
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("invalid conversation id".to_string()))?;
    let pool = get_pool().await?;
    let res = sqlx::query("DELETE FROM conversations WHERE id = ?1")
        .bind(conv_id)
        .execute(pool.as_ref())
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    Ok(Json(res.rows_affected() > 0))
}

#[instrument]
async fn add_message(
    Path(conversation_id): Path<String>,
    Json(req): Json<AddMessageRequest>,
) -> Result<Json<AIMessage>, ApiError> {
    let pool = get_pool().await?;
    let (id, _session_id, _created_at, _updated_at, mut payload) =
        load_conversation_payload(pool.as_ref(), &conversation_id).await?;

    let message = AIMessage {
        id: format!("msg_{}", now_nanos()),
        role: req.role,
        content: req.content,
        timestamp: now_ts(),
        data_keywords: req.data_keywords,
        data_message_count: req.data_message_count,
        content_blocks: req.content_blocks,
    };
    payload.messages.push(message.clone());

    sqlx::query("UPDATE conversations SET messages = ?1, updated_at = ?2 WHERE id = ?3")
        .bind(serde_json::to_string(&payload)?)
        .bind(now_ts())
        .bind(id)
        .execute(pool.as_ref())
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(Json(message))
}

#[instrument]
async fn get_messages(
    Path(conversation_id): Path<String>,
) -> Result<Json<Vec<AIMessage>>, ApiError> {
    let pool = get_pool().await?;
    let (_, _, _, _, payload) = load_conversation_payload(pool.as_ref(), &conversation_id).await?;
    Ok(Json(payload.messages))
}

#[instrument]
async fn delete_message(Path(message_id): Path<String>) -> Result<Json<bool>, ApiError> {
    let pool = get_pool().await?;
    let rows = sqlx::query("SELECT id, messages FROM conversations")
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    for row in rows {
        let conv_id: i64 = row.try_get("id").unwrap_or(0);
        let raw: String = row.try_get("messages").unwrap_or_else(|_| "{}".to_string());
        let mut payload = serde_json::from_str::<ConversationPayload>(&raw).unwrap_or_default();
        let before = payload.messages.len();
        payload.messages.retain(|m| m.id != message_id);
        if payload.messages.len() != before {
            sqlx::query("UPDATE conversations SET messages = ?1, updated_at = ?2 WHERE id = ?3")
                .bind(serde_json::to_string(&payload)?)
                .bind(now_ts())
                .bind(conv_id)
                .execute(pool.as_ref())
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?;
            return Ok(Json(true));
        }
    }
    Ok(Json(false))
}

#[instrument]
async fn show_ai_log_file() -> Result<Json<serde_json::Value>, ApiError> {
    let log_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot")
        .join("logs");
    fs::create_dir_all(&log_dir)?;
    let log_file = log_dir.join("ai.log");
    if !log_file.exists() {
        fs::write(&log_file, "")?;
    }
    Ok(Json(serde_json::json!({
        "success": true,
        "path": log_file.to_string_lossy().to_string(),
    })))
}

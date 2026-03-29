//! Explicit memory-entry API for persisted summaries and recall-friendly records.

use axum::{
    extract::{Path, Query},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::sync::Arc;

use crate::{database::Repository, ApiError};

const SESSION_SUMMARY_MEMORY_KIND: &str = "session_summary";

/// Memory API router.
pub fn router() -> Router {
    Router::new()
        .route("/sessions/:session_id/entries", get(list_memory_entries))
        .route(
            "/sessions/:session_id/sync-session-summaries",
            post(sync_session_summaries),
        )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryListQuery {
    kind: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntryItem {
    pub id: u64,
    pub meta_id: u64,
    pub chat_session_id: Option<u64>,
    pub memory_kind: String,
    pub title: Option<String>,
    pub content: String,
    pub tags: Vec<String>,
    pub source_label: Option<String>,
    pub importance: i64,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryListResponse {
    pub items: Vec<MemoryEntryItem>,
    pub count: u64,
    pub limit: u64,
    pub offset: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSessionSummariesResponse {
    pub scanned: u64,
    pub upserted: u64,
    pub skipped: u64,
}

#[derive(Debug, FromRow)]
struct SessionMemorySourceRow {
    chat_session_id: i64,
    start_ts: i64,
    end_ts: i64,
    summary: String,
    chat_name: String,
    platform: String,
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

fn parse_tags(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw)
        .unwrap_or_default()
        .into_iter()
        .filter(|item| !item.trim().is_empty())
        .collect()
}

fn build_memory_title(chat_name: &str, chat_session_id: i64) -> String {
    format!("Session Summary #{} · {}", chat_session_id, chat_name)
}

fn build_session_summary_tags(platform: &str) -> String {
    serde_json::to_string(&vec!["session-summary", "auto-summary", platform])
        .unwrap_or_else(|_| "[\"session-summary\",\"auto-summary\"]".to_string())
}

fn build_session_summary_importance(summary: &str) -> i64 {
    let base = 55i64;
    let bonus = (summary.chars().count() as i64 / 24).clamp(0, 25);
    (base + bonus).clamp(40, 90)
}

async fn load_session_memory_source(
    pool: &SqlitePool,
    meta_id: i64,
    chat_session_id: i64,
) -> Result<Option<SessionMemorySourceRow>, ApiError> {
    sqlx::query_as::<_, SessionMemorySourceRow>(
        r#"
        SELECT
            cs.id as chat_session_id,
            cs.start_ts,
            cs.end_ts,
            COALESCE(cs.summary, '') as summary,
            meta.name as chat_name,
            meta.platform as platform
        FROM chat_session cs
        JOIN meta ON meta.id = cs.meta_id
        WHERE cs.id = ?1 AND cs.meta_id = ?2
        "#,
    )
    .bind(chat_session_id)
    .bind(meta_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))
}

pub(crate) async fn upsert_session_summary_memory(
    pool: &SqlitePool,
    meta_id: i64,
    chat_session_id: i64,
    summary: &str,
) -> Result<bool, ApiError> {
    let trimmed = summary.trim();
    if trimmed.is_empty() {
        return Ok(false);
    }

    let Some(source) = load_session_memory_source(pool, meta_id, chat_session_id).await? else {
        return Err(ApiError::NotFound(
            "Chat session not found for memory sync".to_string(),
        ));
    };

    let repo = Repository::new(Arc::new(pool.clone()));
    let title = build_memory_title(&source.chat_name, source.chat_session_id);
    let tags = build_session_summary_tags(&source.platform);
    let source_label = format!(
        "chat_session:{}:{}:{}",
        source.chat_session_id, source.start_ts, source.end_ts
    );

    repo.upsert_session_memory_entry(
        meta_id,
        chat_session_id,
        SESSION_SUMMARY_MEMORY_KIND,
        Some(title.as_str()),
        trimmed,
        &tags,
        Some(source_label.as_str()),
        build_session_summary_importance(trimmed),
        now_ts(),
    )
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(true)
}

async fn list_memory_entries(
    Path(session_id): Path<String>,
    Query(query): Query<MemoryListQuery>,
) -> Result<Json<MemoryListResponse>, ApiError> {
    let meta_id = parse_meta_id(&session_id)?;
    let pool = get_pool().await?;
    ensure_chat_exists(&pool, meta_id).await?;

    let repo = Repository::new(pool);
    let limit = query.limit.unwrap_or(50).clamp(1, 200) as i32;
    let offset = query.offset.unwrap_or(0) as i32;
    let rows = repo
        .list_memory_entries(meta_id, query.kind.as_deref(), limit, offset)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let items = rows
        .into_iter()
        .map(|row| MemoryEntryItem {
            id: row.id as u64,
            meta_id: row.meta_id as u64,
            chat_session_id: row.chat_session_id.map(|v| v as u64),
            memory_kind: row.memory_kind,
            title: row.title,
            content: row.content,
            tags: parse_tags(&row.tags),
            source_label: row.source_label,
            importance: row.importance,
            created_at: row.created_at as u64,
            updated_at: row.updated_at as u64,
        })
        .collect::<Vec<_>>();

    Ok(Json(MemoryListResponse {
        count: items.len() as u64,
        items,
        limit: limit as u64,
        offset: offset as u64,
    }))
}

async fn sync_session_summaries(
    Path(session_id): Path<String>,
) -> Result<Json<SyncSessionSummariesResponse>, ApiError> {
    let meta_id = parse_meta_id(&session_id)?;
    let pool = get_pool().await?;
    ensure_chat_exists(&pool, meta_id).await?;

    let rows = sqlx::query_as::<_, SessionMemorySourceRow>(
        r#"
        SELECT
            cs.id as chat_session_id,
            cs.start_ts,
            cs.end_ts,
            COALESCE(cs.summary, '') as summary,
            meta.name as chat_name,
            meta.platform as platform
        FROM chat_session cs
        JOIN meta ON meta.id = cs.meta_id
        WHERE cs.meta_id = ?1
        ORDER BY cs.start_ts DESC, cs.id DESC
        "#,
    )
    .bind(meta_id)
    .fetch_all(&*pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    let mut response = SyncSessionSummariesResponse {
        scanned: rows.len() as u64,
        upserted: 0,
        skipped: 0,
    };

    for row in rows {
        if row.summary.trim().is_empty() {
            response.skipped = response.skipped.saturating_add(1);
            continue;
        }
        if upsert_session_summary_memory(&pool, meta_id, row.chat_session_id, &row.summary).await? {
            response.upserted = response.upserted.saturating_add(1);
        } else {
            response.skipped = response.skipped.saturating_add(1);
        }
    }

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use axum::body::{to_bytes, Body};
    use axum::http::{Method, Request, StatusCode};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower::util::ServiceExt;
    use xenobot_core::config::DatabaseConfig;

    use crate::database::{ChatMeta, ChatSession, Message, MessageContext, Repository};

    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn unique_test_root() -> std::path::PathBuf {
        let epoch_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("xenobot_memory_api_{}_{}", epoch_nanos, seq))
    }

    async fn get_json(
        app: &axum::Router,
        path: &str,
    ) -> Result<(StatusCode, serde_json::Value), Box<dyn std::error::Error>> {
        let request = Request::builder()
            .method(Method::GET)
            .uri(path)
            .body(Body::empty())?;
        let response = app.clone().oneshot(request).await?;
        let status = response.status();
        let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body_json = serde_json::from_slice::<serde_json::Value>(&body_bytes)?;
        Ok((status, body_json))
    }

    async fn post_json(
        app: &axum::Router,
        path: &str,
        payload: serde_json::Value,
    ) -> Result<(StatusCode, serde_json::Value), Box<dyn std::error::Error>> {
        let request = Request::builder()
            .method(Method::POST)
            .uri(path)
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))?;
        let response = app.clone().oneshot(request).await?;
        let status = response.status();
        let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body_json = serde_json::from_slice::<serde_json::Value>(&body_bytes)?;
        Ok((status, body_json))
    }

    async fn setup_session_fixture(
        with_summary: bool,
    ) -> Result<(std::path::PathBuf, i64, i64), Box<dyn std::error::Error>> {
        let test_root = unique_test_root();
        std::fs::create_dir_all(&test_root)?;
        let db_path = test_root.join("xenobot_memory.db");

        let mut db_config = DatabaseConfig::default();
        db_config.sqlite_path = db_path;
        crate::database::init_database_with_config(&db_config).await?;

        let pool = crate::database::get_pool().await?;
        let repo = Repository::new(pool.clone());

        let meta_id = repo
            .create_chat(&ChatMeta {
                id: 0,
                name: "Memory Verification".to_string(),
                platform: "wechat".to_string(),
                chat_type: "group".to_string(),
                imported_at: 1_700_000_000,
                group_id: None,
                group_avatar: None,
                owner_id: None,
                schema_version: 3,
                session_gap_threshold: 1800,
            })
            .await?;

        let alice = repo
            .get_or_create_member("wechat:alice", Some("Alice"))
            .await?;
        let bob = repo.get_or_create_member("wechat:bob", Some("Bob")).await?;

        let messages = [
            (alice, "Launch readiness looks good."),
            (bob, "I still want one more checklist pass."),
            (alice, "Please remember the blocker about the webhook."),
            (bob, "We should keep the summary inside memory."),
        ];

        let mut message_ids = Vec::new();
        for (idx, (sender_id, content)) in messages.iter().enumerate() {
            let message_id = repo
                .create_message(&Message {
                    id: 0,
                    sender_id: *sender_id,
                    sender_account_name: None,
                    sender_group_nickname: None,
                    ts: 1_700_000_100 + idx as i64,
                    msg_type: 0,
                    content: Some((*content).to_string()),
                    reply_to_message_id: None,
                    platform_message_id: None,
                    meta_id,
                })
                .await?;
            message_ids.push(message_id);
        }

        let chat_session_id = repo
            .create_chat_session(&ChatSession {
                id: 0,
                meta_id,
                start_ts: 1_700_000_100,
                end_ts: 1_700_000_150,
                message_count: Some(message_ids.len() as i64),
                is_manual: Some(false),
                summary: with_summary.then(|| {
                    "Two members aligned on launch readiness while tracking a webhook blocker."
                        .to_string()
                }),
            })
            .await?;

        for message_id in message_ids {
            repo.create_message_context(&MessageContext {
                message_id,
                session_id: chat_session_id,
                topic_id: None,
            })
            .await?;
        }

        Ok((test_root, meta_id, chat_session_id))
    }

    #[tokio::test]
    async fn generate_summary_route_persists_memory_entry() -> Result<(), Box<dyn std::error::Error>>
    {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let (test_root, meta_id, chat_session_id) = setup_session_fixture(false).await?;

        let app = crate::router::build_router(&crate::config::ApiConfig::default());
        let (summary_status, summary_body) = post_json(
            &app,
            &format!("/session/generate-summary/{}/{}", meta_id, chat_session_id),
            serde_json::json!({
                "locale": "en-US",
                "forceRegenerate": true
            }),
        )
        .await?;
        assert_eq!(summary_status, StatusCode::OK);
        assert!(summary_body["summary"]
            .as_str()
            .is_some_and(|value| !value.trim().is_empty()));

        let (memory_status, memory_body) =
            get_json(&app, &format!("/memory/sessions/{}/entries", meta_id)).await?;
        assert_eq!(memory_status, StatusCode::OK);
        assert_eq!(memory_body["count"].as_u64().unwrap_or(0), 1);
        assert_eq!(memory_body["items"][0]["memoryKind"], "session_summary");
        assert_eq!(
            memory_body["items"][0]["chatSessionId"]
                .as_u64()
                .unwrap_or(0),
            chat_session_id as u64
        );
        assert!(memory_body["items"][0]["content"]
            .as_str()
            .is_some_and(|value| !value.trim().is_empty()));

        let _ = std::fs::remove_dir_all(&test_root);
        Ok(())
    }

    #[tokio::test]
    async fn sync_session_summaries_backfills_memory_entries(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let (test_root, meta_id, _chat_session_id) = setup_session_fixture(true).await?;

        let app = crate::router::build_router(&crate::config::ApiConfig::default());
        let (sync_status, sync_body) = post_json(
            &app,
            &format!("/memory/sessions/{}/sync-session-summaries", meta_id),
            serde_json::json!({}),
        )
        .await?;
        assert_eq!(sync_status, StatusCode::OK);
        assert_eq!(sync_body["upserted"].as_u64().unwrap_or(0), 1);

        let (memory_status, memory_body) =
            get_json(&app, &format!("/memory/sessions/{}/entries", meta_id)).await?;
        assert_eq!(memory_status, StatusCode::OK);
        assert_eq!(memory_body["count"].as_u64().unwrap_or(0), 1);
        assert!(memory_body["items"][0]["tags"]
            .as_array()
            .is_some_and(|tags| tags
                .iter()
                .any(|item| item.as_str() == Some("session-summary"))));

        let _ = std::fs::remove_dir_all(&test_root);
        Ok(())
    }
}

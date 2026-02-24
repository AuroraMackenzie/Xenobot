use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use tower::util::ServiceExt;
use xenobot_api::ai;
use xenobot_api::database::repository::{ChatMeta, Message};
use xenobot_api::database::Repository;
use xenobot_core::config::DatabaseConfig;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct WorkingDirGuard {
    previous: PathBuf,
}

impl WorkingDirGuard {
    fn change_to(path: &std::path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        let previous = std::env::current_dir()?;
        std::env::set_current_dir(path)?;
        Ok(Self { previous })
    }
}

impl Drop for WorkingDirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.previous);
    }
}

fn unique_test_root() -> PathBuf {
    let epoch_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("xenobot_api_semantic_{}_{}", epoch_nanos, seq))
}

async fn post_json(
    app: &axum::Router,
    path: &str,
    payload: serde_json::Value,
) -> Result<(StatusCode, serde_json::Value), Box<dyn std::error::Error>> {
    let request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))?;
    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let body_json = serde_json::from_slice::<serde_json::Value>(&body_bytes)?;
    Ok((status, body_json))
}

#[tokio::test]
async fn test_semantic_search_endpoint_returns_ranked_messages_and_rewrite(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_semantic.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());

    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "Semantic Session".to_string(),
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

    let sender_id = repo
        .get_or_create_member("u_alice", Some("Alice"))
        .await?;

    let rows = [
        "database migration checkpoint incremental import finished",
        "incremental import checkpoint failed and retried",
        "beach travel photo sunset and holiday music",
    ];

    for (idx, content) in rows.iter().enumerate() {
        repo.create_message(&Message {
            id: 0,
            sender_id,
            sender_account_name: Some("Alice".to_string()),
            sender_group_nickname: Some("Alice".to_string()),
            ts: 1_700_000_100 + idx as i64,
            msg_type: 0,
            content: Some((*content).to_string()),
            reply_to_message_id: None,
            platform_message_id: None,
            meta_id,
        })
        .await?;
    }

    let app = ai::router();
    let (status, resp) = post_json(
        &app,
        "/semantic-search-messages",
        serde_json::json!({
            "sessionId": meta_id.to_string(),
            "query": "聊天记录 msg 增量导入 checkpoint",
            "threshold": 0.0,
            "limit": 5
        }),
    )
    .await?;

    assert_eq!(status, StatusCode::OK);
    let rewritten = resp["queryRewritten"].as_str().unwrap_or_default();
    assert!(rewritten.contains("聊天"));
    assert!(rewritten.contains("message"));

    let messages = resp["messages"]
        .as_array()
        .expect("messages should be an array");
    assert!(!messages.is_empty());

    let has_expected_hit = messages.iter().any(|m| {
        let content = m["content"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase();
        content.contains("incremental") || content.contains("checkpoint")
    });
    assert!(has_expected_hit);
    assert!(messages[0]["similarity"].as_f64().unwrap_or(-1.0) >= 0.0);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

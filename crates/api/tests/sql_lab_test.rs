use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::util::ServiceExt;
use xenobot_api::chat;
use xenobot_api::database::repository::ChatMeta;
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
    std::env::temp_dir().join(format!("xenobot_api_sql_lab_{}_{}", epoch_nanos, seq))
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
async fn test_execute_sql_enforces_read_only_policy(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_sql_lab.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let session_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "SQL Lab Session".to_string(),
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

    let app = chat::router();
    let endpoint = format!("/sessions/{session_id}/execute-sql");

    let (status, select_resp) = post_json(
        &app,
        &endpoint,
        json!({
            "sql": format!("SELECT id, name, platform FROM meta WHERE id = {}", session_id)
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(select_resp["rowCount"].as_u64().unwrap_or(0), 1);

    let (status, cte_resp) = post_json(
        &app,
        &endpoint,
        json!({
            "sql": format!("WITH src AS (SELECT id, name FROM meta WHERE id = {}) SELECT name FROM src", session_id)
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cte_resp["rowCount"].as_u64().unwrap_or(0), 1);

    let (status, literal_resp) = post_json(
        &app,
        &endpoint,
        json!({
            "sql": "SELECT 'DROP TABLE meta' AS preview"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(literal_resp["rowCount"].as_u64().unwrap_or(0), 1);

    let (status, mutation_resp) = post_json(
        &app,
        &endpoint,
        json!({
            "sql": "DELETE FROM meta WHERE id = 1"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let mutation_error = mutation_resp["error"].as_str().unwrap_or_default();
    assert!(
        mutation_error.contains("Only SELECT")
            || mutation_error.contains("read-only"),
        "unexpected mutation error: {}",
        mutation_error
    );

    let (status, multi_stmt_resp) = post_json(
        &app,
        &endpoint,
        json!({
            "sql": "SELECT 1; SELECT 2"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        multi_stmt_resp["error"]
            .as_str()
            .unwrap_or_default()
            .contains("Multiple SQL statements")
    );

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

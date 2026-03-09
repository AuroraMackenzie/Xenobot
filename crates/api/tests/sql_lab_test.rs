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

struct EnvVarGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(prev) = self.previous.as_ref() {
            std::env::set_var(self.key, prev);
        } else {
            std::env::remove_var(self.key);
        }
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

async fn get_json(
    app: &axum::Router,
    path: &str,
) -> Result<(StatusCode, serde_json::Value), Box<dyn std::error::Error>> {
    let request = Request::builder()
        .method("GET")
        .uri(path)
        .body(Body::empty())?;
    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let body_json = serde_json::from_slice::<serde_json::Value>(&body_bytes)?;
    Ok((status, body_json))
}

#[tokio::test]
async fn test_execute_sql_enforces_read_only_policy() -> Result<(), Box<dyn std::error::Error>> {
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
        mutation_error.contains("Only SELECT") || mutation_error.contains("read-only"),
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
    assert!(multi_stmt_resp["error"]
        .as_str()
        .unwrap_or_default()
        .contains("Multiple SQL statements"));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_execute_sql_returns_not_found_for_missing_session(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_sql_lab_missing_session.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let app = chat::router();
    let endpoint = "/sessions/999999/execute-sql";
    let (status, resp) = post_json(
        &app,
        endpoint,
        json!({
            "sql": "SELECT 1 AS ok"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(resp["error"]
        .as_str()
        .unwrap_or_default()
        .contains("session 999999 not found"));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_execute_sql_caps_large_result_sets_with_limited_flag(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_sql_lab_limit.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let session_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "SQL Limit Session".to_string(),
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
    let (status, resp) = post_json(
        &app,
        &endpoint,
        json!({
            "sql": "WITH RECURSIVE cnt(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM cnt WHERE x < 6000) SELECT x FROM cnt"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["limited"], true);
    assert_eq!(resp["rowCount"].as_u64().unwrap_or(0), 5000);
    assert_eq!(
        resp["columns"].as_array().map(|arr| arr.len()).unwrap_or(0),
        1
    );

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_generate_sql_returns_safe_query_that_can_be_executed(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_sql_lab_generate.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let session_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "SQL Generate Session".to_string(),
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
    let generate_endpoint = format!("/sessions/{session_id}/generate-sql");
    let (status, generate_resp) = post_json(
        &app,
        &generate_endpoint,
        json!({
            "prompt": "统计包含\"hello\"的发言数量",
            "maxRows": 50
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(generate_resp["success"], true);

    let generated_sql = generate_resp["sql"]
        .as_str()
        .expect("generate sql should return sql string");
    assert!(generated_sql.to_ascii_uppercase().contains("SELECT"));
    assert!(
        generated_sql.contains(&format!("msg.meta_id = {}", session_id)),
        "generated SQL should be scoped to the current session"
    );

    let execute_endpoint = format!("/sessions/{session_id}/execute-sql");
    let (exec_status, _) = post_json(
        &app,
        &execute_endpoint,
        json!({
            "sql": generated_sql
        }),
    )
    .await?;
    assert_eq!(exec_status, StatusCode::OK);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_generate_sql_supports_hourly_and_daily_intents(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_sql_lab_generate_time_intents.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let session_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "SQL Generate Time Intent Session".to_string(),
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
    let endpoint = format!("/sessions/{session_id}/generate-sql");

    let (hourly_status, hourly_resp) = post_json(
        &app,
        &endpoint,
        json!({
            "prompt": "按小时统计消息数量",
            "maxRows": 24
        }),
    )
    .await?;
    assert_eq!(hourly_status, StatusCode::OK);
    let hourly_sql = hourly_resp["sql"].as_str().unwrap_or_default();
    assert!(hourly_sql.contains("strftime('%H'"));
    assert!(hourly_sql.contains("GROUP BY hour_bucket"));
    assert!(hourly_sql.contains(&format!("msg.meta_id = {}", session_id)));

    let (daily_status, daily_resp) = post_json(
        &app,
        &endpoint,
        json!({
            "prompt": "按天统计消息数量",
            "maxRows": 31
        }),
    )
    .await?;
    assert_eq!(daily_status, StatusCode::OK);
    let daily_sql = daily_resp["sql"].as_str().unwrap_or_default();
    assert!(daily_sql.contains("date(datetime(msg.ts"));
    assert!(daily_sql.contains("GROUP BY day_bucket"));
    assert!(daily_sql.contains(&format!("msg.meta_id = {}", session_id)));

    let (weekday_status, weekday_resp) = post_json(
        &app,
        &endpoint,
        json!({
            "prompt": "按星期统计消息数量",
            "maxRows": 7
        }),
    )
    .await?;
    assert_eq!(weekday_status, StatusCode::OK);
    let weekday_sql = weekday_resp["sql"].as_str().unwrap_or_default();
    assert!(weekday_sql.contains("strftime('%w'"));
    assert!(weekday_sql.contains("GROUP BY weekday_bucket"));
    assert!(weekday_sql.contains(&format!("msg.meta_id = {}", session_id)));

    let (monthly_status, monthly_resp) = post_json(
        &app,
        &endpoint,
        json!({
            "prompt": "按月统计消息数量",
            "maxRows": 12
        }),
    )
    .await?;
    assert_eq!(monthly_status, StatusCode::OK);
    let monthly_sql = monthly_resp["sql"].as_str().unwrap_or_default();
    assert!(monthly_sql.contains("strftime('%Y-%m'"));
    assert!(monthly_sql.contains("GROUP BY month_bucket"));
    assert!(monthly_sql.contains(&format!("msg.meta_id = {}", session_id)));

    let (yearly_status, yearly_resp) = post_json(
        &app,
        &endpoint,
        json!({
            "prompt": "按年统计消息数量",
            "maxRows": 10
        }),
    )
    .await?;
    assert_eq!(yearly_status, StatusCode::OK);
    let yearly_sql = yearly_resp["sql"].as_str().unwrap_or_default();
    assert!(yearly_sql.contains("strftime('%Y'"));
    assert!(yearly_sql.contains("GROUP BY year_bucket"));
    assert!(yearly_sql.contains(&format!("msg.meta_id = {}", session_id)));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_generate_sql_supports_type_length_and_mention_intents(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_sql_lab_generate_content_intents.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let session_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "SQL Generate Content Intent Session".to_string(),
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
    let endpoint = format!("/sessions/{session_id}/generate-sql");

    let (type_status, type_resp) = post_json(
        &app,
        &endpoint,
        json!({
            "prompt": "消息类型分布",
            "maxRows": 10
        }),
    )
    .await?;
    assert_eq!(type_status, StatusCode::OK);
    let type_sql = type_resp["sql"].as_str().unwrap_or_default();
    assert!(type_sql.contains("msg.msg_type"));
    assert!(type_sql.contains("GROUP BY msg.msg_type"));
    assert!(type_sql.contains(&format!("msg.meta_id = {}", session_id)));

    let (length_status, length_resp) = post_json(
        &app,
        &endpoint,
        json!({
            "prompt": "最长消息",
            "maxRows": 20
        }),
    )
    .await?;
    assert_eq!(length_status, StatusCode::OK);
    let length_sql = length_resp["sql"].as_str().unwrap_or_default();
    assert!(length_sql.contains("length(msg.content) AS content_length"));
    assert!(length_sql.contains("ORDER BY content_length DESC"));
    assert!(length_sql.contains(&format!("msg.meta_id = {}", session_id)));

    let (mention_status, mention_resp) = post_json(
        &app,
        &endpoint,
        json!({
            "prompt": "最近@提及消息",
            "maxRows": 30
        }),
    )
    .await?;
    assert_eq!(mention_status, StatusCode::OK);
    let mention_sql = mention_resp["sql"].as_str().unwrap_or_default();
    assert!(mention_sql.contains("msg.content LIKE '%@%'"));
    assert!(mention_sql.contains("ORDER BY msg.ts DESC"));
    assert!(mention_sql.contains(&format!("msg.meta_id = {}", session_id)));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_generate_sql_keyword_intent_without_quotes_does_not_add_like_filter(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_sql_lab_generate_keyword_warning.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let session_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "SQL Generate Keyword Warning Session".to_string(),
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
    let endpoint = format!("/sessions/{session_id}/generate-sql");
    let (status, resp) = post_json(
        &app,
        &endpoint,
        json!({
            "prompt": "search messages by keyword hello",
            "maxRows": 100
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let generated_sql = resp["sql"].as_str().unwrap_or_default();
    assert!(
        !generated_sql.contains("LIKE"),
        "sql should not include LIKE filter when keyword is unquoted"
    );
    assert!(generated_sql.contains(&format!("msg.meta_id = {}", session_id)));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_generate_sql_fallback_intent_keeps_session_scope() -> Result<(), Box<dyn std::error::Error>>
{
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_sql_lab_generate_fallback_scope.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let session_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "SQL Generate Fallback Scope Session".to_string(),
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
    let endpoint = format!("/sessions/{session_id}/generate-sql");
    let (status, resp) = post_json(
        &app,
        &endpoint,
        json!({
            "prompt": "show me something unusual with no known intent words",
            "maxRows": 20
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let generated_sql = resp["sql"].as_str().unwrap_or_default();
    assert!(generated_sql.contains(&format!("msg.meta_id = {}", session_id)));
    assert_eq!(resp["strategy"], "rule_based_safe_sql");

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_generate_sql_rejects_empty_prompt() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_sql_lab_generate_empty_prompt.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let session_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "SQL Generate Empty Prompt Session".to_string(),
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
    let endpoint = format!("/sessions/{session_id}/generate-sql");
    let (status, resp) = post_json(
        &app,
        &endpoint,
        json!({
            "prompt": "   "
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(resp["error"]
        .as_str()
        .unwrap_or_default()
        .contains("Prompt cannot be empty"));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_get_schema_returns_core_table_definitions() -> Result<(), Box<dyn std::error::Error>>
{
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_sql_lab_schema.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let session_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "SQL Schema Session".to_string(),
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
    let endpoint = format!("/sessions/{session_id}/schema");
    let (status, schema_resp) = get_json(&app, &endpoint).await?;
    assert_eq!(status, StatusCode::OK);
    let tables = schema_resp
        .as_array()
        .expect("schema response should be table array");
    assert!(!tables.is_empty());
    assert!(tables.iter().any(|table| table["name"] == "meta"));
    let meta_table = tables
        .iter()
        .find(|table| table["name"] == "meta")
        .expect("meta table should exist");
    let columns = meta_table["columns"]
        .as_array()
        .expect("meta table should include columns");
    assert!(columns.iter().any(|c| c["name"] == "id" && c["pk"] == true));
    assert!(columns
        .iter()
        .any(|c| c["name"] == "name" && c["type"].as_str().is_some()));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_get_schema_detailed_returns_summary_indexes_and_row_count(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_sql_lab_schema_detailed.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let session_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "SQL Schema Detailed Session".to_string(),
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
    let endpoint = format!("/sessions/{session_id}/schema?detailed=true&includeRowCount=true");
    let (status, schema_resp) = get_json(&app, &endpoint).await?;
    assert_eq!(status, StatusCode::OK);

    let tables = schema_resp["tables"]
        .as_array()
        .expect("detailed schema response should include tables");
    assert!(!tables.is_empty());
    assert_eq!(schema_resp["includesRowCount"], true);
    assert!(schema_resp["summary"]["tableCount"].as_u64().unwrap_or(0) >= 1);
    assert!(schema_resp["summary"]["columnCount"].as_u64().unwrap_or(0) >= 1);

    let meta_table = tables
        .iter()
        .find(|table| table["name"] == "meta")
        .expect("meta table should exist in detailed schema");
    assert!(meta_table["indexes"].is_array());
    assert!(meta_table["foreignKeys"].is_array());
    assert!(meta_table["rowCount"].as_i64().unwrap_or(-1) >= 1);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_get_schema_returns_not_found_for_missing_session(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_sql_lab_schema_missing_session.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let app = chat::router();
    let endpoint = "/sessions/888888/schema";
    let (status, resp) = get_json(&app, endpoint).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(resp["error"]
        .as_str()
        .unwrap_or_default()
        .contains("session 888888 not found"));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_execute_sql_returns_timeout_for_slow_query() -> Result<(), Box<dyn std::error::Error>>
{
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;
    let _env_guard = EnvVarGuard::set("XENOBOT_SQL_TIMEOUT_MS", "50");

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_sql_lab_timeout.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let session_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "SQL Timeout Session".to_string(),
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
    let (status, resp) = post_json(
        &app,
        &endpoint,
        json!({
            "sql": "WITH RECURSIVE cnt(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM cnt WHERE x < 50000000) SELECT COUNT(*) FROM cnt"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::REQUEST_TIMEOUT);
    assert!(resp["error"]
        .as_str()
        .unwrap_or_default()
        .contains("SQL query exceeded"));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

use std::fs;
use std::path::{Path, PathBuf};
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
    fn change_to(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
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
    std::env::temp_dir().join(format!("xenobot_api_incremental_{}_{}", epoch_nanos, seq))
}

fn write_json_file(
    path: &Path,
    value: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = serde_json::to_string_pretty(value)?;
    fs::write(path, content)?;
    Ok(())
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
async fn test_incremental_endpoints_return_session_not_found_for_missing_session(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_incremental_missing_session.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let app = chat::router();
    let missing_session_id = 999_999_i64;
    let analyze_path = format!("/sessions/{missing_session_id}/analyze-incremental-import");
    let import_path = format!("/sessions/{missing_session_id}/incremental-import");

    let (status, analyze_resp) = post_json(
        &app,
        &analyze_path,
        json!({ "file_path": "/tmp/nonexistent-authorized-export.json" }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(analyze_resp["success"], false);
    assert_eq!(analyze_resp["error"], "error.session_not_found");

    let (status, import_resp) = post_json(
        &app,
        &import_path,
        json!({ "file_path": "/tmp/nonexistent-authorized-export.json" }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(import_resp["success"], false);
    assert_eq!(import_resp["error"], "error.session_not_found");

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_incremental_checkpoint_fast_skip_and_failed_writeback(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_test.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "Incremental Session".to_string(),
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

    let valid_export_path = test_root.join("authorized_export.json");
    write_json_file(
        &valid_export_path,
        &json!({
            "name": "Authorized Export",
            "type": "group",
            "messages": [
                {
                    "sender_id": "alice",
                    "sender_name": "Alice",
                    "timestamp": 1700000001,
                    "msg_type": 0,
                    "content": "hello incremental import"
                }
            ]
        }),
    )?;
    let valid_export = valid_export_path.to_string_lossy().to_string();

    let app = chat::router();
    let analyze_path = format!("/sessions/{meta_id}/analyze-incremental-import");
    let import_path = format!("/sessions/{meta_id}/incremental-import");

    let (status, first_analyze) =
        post_json(&app, &analyze_path, json!({ "file_path": valid_export })).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_analyze["newMessageCount"], 1);
    assert_eq!(first_analyze["duplicateCount"], 0);
    assert_eq!(first_analyze["totalInFile"], 1);
    assert!(first_analyze.get("checkpointSkipped").is_none());

    let (status, first_import) =
        post_json(&app, &import_path, json!({ "file_path": valid_export })).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_import["success"], true);
    assert_eq!(first_import["newMessageCount"], 1);
    assert_eq!(first_import["duplicateCount"], 0);

    let (status, second_analyze) =
        post_json(&app, &analyze_path, json!({ "file_path": valid_export })).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second_analyze["success"], true);
    assert_eq!(second_analyze["sessionId"], meta_id.to_string());
    assert_eq!(second_analyze["checkpointSkipped"], true);
    assert_eq!(second_analyze["newMessageCount"], 0);
    assert_eq!(second_analyze["duplicateCount"], 0);
    assert_eq!(second_analyze["totalInFile"], 0);
    assert_eq!(second_analyze["lastCheckpoint"]["status"], "completed");
    assert!(second_analyze["checkpointMeta"]["fingerprint"]
        .as_str()
        .unwrap_or_default()
        .starts_with("v2:"));

    let (status, second_import) =
        post_json(&app, &import_path, json!({ "file_path": valid_export })).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second_import["success"], true);
    assert_eq!(second_import["sessionId"], meta_id.to_string());
    assert_eq!(second_import["checkpointSkipped"], true);
    assert_eq!(second_import["newMessageCount"], 0);
    assert_eq!(second_import["duplicateCount"], 0);
    assert_eq!(second_import["lastCheckpoint"]["status"], "completed");
    assert!(second_import["checkpointMeta"]["fingerprint"]
        .as_str()
        .unwrap_or_default()
        .starts_with("v2:"));

    let source_kind = format!("api-incremental-{meta_id}");
    let completed_checkpoint = repo
        .get_import_source_checkpoint(&source_kind, &valid_export)
        .await?
        .expect("completed checkpoint should exist");
    assert_eq!(completed_checkpoint.status, "completed");
    assert_eq!(completed_checkpoint.last_inserted_messages, 1);
    assert_eq!(completed_checkpoint.last_duplicate_messages, 0);

    let message_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
        .bind(meta_id)
        .fetch_one(&*pool)
        .await?;
    assert_eq!(message_count, 1);

    write_json_file(
        &valid_export_path,
        &json!({
            "name": "Authorized Export",
            "type": "group",
            "messages": [
                {
                    "sender_id": "alice",
                    "sender_name": "Alice",
                    "timestamp": 1700000001,
                    "msg_type": 0,
                    "content": "hello incremental import"
                },
                {
                    "sender_id": "alice",
                    "sender_name": "Alice",
                    "timestamp": 1700000002,
                    "msg_type": 0,
                    "content": "new delta message"
                }
            ]
        }),
    )?;

    let (status, analyze_after_change) =
        post_json(&app, &analyze_path, json!({ "file_path": valid_export })).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(analyze_after_change["newMessageCount"], 1);
    assert_eq!(analyze_after_change["duplicateCount"], 1);
    assert_eq!(analyze_after_change["totalInFile"], 2);
    assert!(analyze_after_change.get("checkpointSkipped").is_none());

    let (status, import_after_change) =
        post_json(&app, &import_path, json!({ "file_path": valid_export })).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(import_after_change["success"], true);
    assert_eq!(import_after_change["newMessageCount"], 1);
    assert_eq!(import_after_change["duplicateCount"], 1);
    assert_eq!(import_after_change["totalInFile"], 2);
    assert!(import_after_change.get("checkpointSkipped").is_none());

    let checkpoint_after_change = repo
        .get_import_source_checkpoint(&source_kind, &valid_export)
        .await?
        .expect("checkpoint should be updated after delta import");
    assert_eq!(checkpoint_after_change.status, "completed");
    assert_eq!(checkpoint_after_change.last_inserted_messages, 1);
    assert_eq!(checkpoint_after_change.last_duplicate_messages, 1);

    let message_count_after_change: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
            .bind(meta_id)
            .fetch_one(&*pool)
            .await?;
    assert_eq!(message_count_after_change, 2);

    let invalid_export_path = test_root.join("broken_payload.bin");
    fs::write(&invalid_export_path, "not_json_payload")?;
    let invalid_export = invalid_export_path.to_string_lossy().to_string();

    let (status, analyze_failed) =
        post_json(&app, &analyze_path, json!({ "file_path": invalid_export })).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(analyze_failed["error"], "error.unrecognized_format");
    let checkpoint_after_analyze_failure = repo
        .get_import_source_checkpoint(&source_kind, &invalid_export)
        .await?
        .expect("failed checkpoint should be created by analyze");
    assert_eq!(checkpoint_after_analyze_failure.status, "failed");
    assert_eq!(
        checkpoint_after_analyze_failure.error_message.as_deref(),
        Some("analyze parse failed")
    );

    let (status, import_failed) =
        post_json(&app, &import_path, json!({ "file_path": invalid_export })).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(import_failed["success"], false);
    assert_eq!(import_failed["error"], "error.unrecognized_format");

    let checkpoint_after_import_failure = repo
        .get_import_source_checkpoint(&source_kind, &invalid_export)
        .await?
        .expect("failed checkpoint should be updated by incremental import");
    assert_eq!(checkpoint_after_import_failure.status, "failed");
    assert_eq!(
        checkpoint_after_import_failure.error_message.as_deref(),
        Some("unrecognized format")
    );

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_incremental_import_rejects_stale_expected_fingerprint(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_incremental_stale_fingerprint.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());

    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "Incremental Stale Fingerprint".to_string(),
            platform: "telegram".to_string(),
            chat_type: "group".to_string(),
            imported_at: 1_700_000_100,
            group_id: None,
            group_avatar: None,
            owner_id: None,
            schema_version: 3,
            session_gap_threshold: 1800,
        })
        .await?;

    let export_path = test_root.join("stale_expected_fingerprint_export.json");
    write_json_file(
        &export_path,
        &json!({
            "name": "Stale Fingerprint Session",
            "type": "group",
            "messages": [
                {
                    "sender_id": "alice",
                    "sender_name": "Alice",
                    "timestamp": 1700001001,
                    "msg_type": 0,
                    "content": "message v1"
                }
            ]
        }),
    )?;
    let export_file = export_path.to_string_lossy().to_string();
    let app = chat::router();
    let analyze_path = format!("/sessions/{meta_id}/analyze-incremental-import");
    let import_path = format!("/sessions/{meta_id}/incremental-import");

    let (status, analyze_resp) =
        post_json(&app, &analyze_path, json!({ "file_path": export_file })).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(analyze_resp["success"], true);
    let expected_fingerprint = analyze_resp["sourceFingerprint"]
        .as_str()
        .expect("analyze response should include sourceFingerprint")
        .to_string();
    assert_eq!(
        expected_fingerprint,
        analyze_resp["checkpointMeta"]["fingerprint"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    );

    // Mutate the file after analyze, before import.
    write_json_file(
        &export_path,
        &json!({
            "name": "Stale Fingerprint Session",
            "type": "group",
            "messages": [
                {
                    "sender_id": "alice",
                    "sender_name": "Alice",
                    "timestamp": 1700001001,
                    "msg_type": 0,
                    "content": "message v1"
                },
                {
                    "sender_id": "alice",
                    "sender_name": "Alice",
                    "timestamp": 1700001002,
                    "msg_type": 0,
                    "content": "message v2"
                }
            ]
        }),
    )?;

    let (status, stale_import_resp) = post_json(
        &app,
        &import_path,
        json!({
            "filePath": export_file,
            "expectedFingerprint": expected_fingerprint
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stale_import_resp["success"], false);
    assert_eq!(
        stale_import_resp["error"],
        "error.source_changed_since_analyze"
    );
    assert!(
        stale_import_resp["sourceFingerprint"]
            .as_str()
            .unwrap_or_default()
            != stale_import_resp["expectedFingerprint"]
                .as_str()
                .unwrap_or_default()
    );

    // No write should happen when fingerprint mismatches.
    let count_after_reject: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
            .bind(meta_id)
            .fetch_one(&*pool)
            .await?;
    assert_eq!(count_after_reject, 0);

    // Import should succeed once expectedFingerprint matches current file.
    let (status, analyze_latest) =
        post_json(&app, &analyze_path, json!({ "filePath": export_file })).await?;
    assert_eq!(status, StatusCode::OK);
    let latest_fingerprint = analyze_latest["sourceFingerprint"]
        .as_str()
        .expect("latest analyze should include sourceFingerprint");

    let (status, import_ok) = post_json(
        &app,
        &import_path,
        json!({
            "file_path": export_file,
            "expected_fingerprint": latest_fingerprint
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(import_ok["success"], true);
    assert_eq!(import_ok["newMessageCount"], 2);
    assert_eq!(import_ok["duplicateCount"], 0);

    let count_after_success: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
            .bind(meta_id)
            .fetch_one(&*pool)
            .await?;
    assert_eq!(count_after_success, 2);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_platform_matrix_incremental_regression() -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_platform_matrix.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let platform_matrix = vec![
        ("wechat", "wechat"),
        ("whatsapp", "whatsapp"),
        ("line", "line"),
        ("qq", "qq"),
        ("telegram", "telegram"),
        ("discord", "discord"),
        ("instagram", "instagram"),
        ("imessage", "imessage"),
        ("messenger", "messenger"),
        ("kakaotalk", "kakaotalk"),
        ("slack", "slack"),
        ("teams", "teams"),
        ("signal", "signal"),
        ("skype", "skype"),
        ("googlechat", "googlechat"),
        ("zoom", "zoom"),
        ("viber", "viber"),
    ];

    for (idx, (path_token, expected_platform)) in platform_matrix.iter().enumerate() {
        let export_path = test_root.join(format!("{}_authorized_export_{}.json", path_token, idx));
        let base_ts = 1_800_000_000_i64 + (idx as i64) * 10;
        write_json_file(
            &export_path,
            &json!({
                "name": format!("{} Session {}", expected_platform, idx),
                "type": "group",
                "messages": [
                    {
                        "sender_id": format!("{}_sender", expected_platform),
                        "sender_name": format!("{} User", expected_platform),
                        "timestamp": base_ts,
                        "msg_type": 0,
                        "content": format!("{} message A", expected_platform)
                    }
                ]
            }),
        )?;
        let export_file = export_path.to_string_lossy().to_string();

        let (status, detect_resp) =
            post_json(&app, "/detect-format", json!({ "file_path": export_file })).await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(detect_resp["platform"], *expected_platform);
        assert_eq!(detect_resp["format"], "json-chat");
        assert_eq!(detect_resp["parserSource"], "builtin");

        let (status, import_resp) =
            post_json(&app, "/import", json!({ "file_path": export_file })).await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(import_resp["success"], true);
        let session_id = import_resp["sessionId"]
            .as_str()
            .and_then(|v| v.parse::<i64>().ok())
            .expect("import should return sessionId");

        let chat_meta = repo
            .get_chat(session_id)
            .await?
            .expect("session must exist after import");
        assert_eq!(chat_meta.platform, *expected_platform);

        let analyze_path = format!("/sessions/{session_id}/analyze-incremental-import");
        let incremental_path = format!("/sessions/{session_id}/incremental-import");

        let (status, analyze_first) =
            post_json(&app, &analyze_path, json!({ "file_path": export_file })).await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(analyze_first["newMessageCount"], 0);
        assert_eq!(analyze_first["duplicateCount"], 1);
        assert_eq!(analyze_first["totalInFile"], 1);

        let (status, incremental_first) =
            post_json(&app, &incremental_path, json!({ "file_path": export_file })).await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(incremental_first["success"], true);
        assert_eq!(incremental_first["newMessageCount"], 0);
        assert_eq!(incremental_first["duplicateCount"], 1);

        let (status, incremental_second) =
            post_json(&app, &incremental_path, json!({ "file_path": export_file })).await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(incremental_second["success"], true);
        assert_eq!(incremental_second["checkpointSkipped"], true);

        write_json_file(
            &export_path,
            &json!({
                "name": format!("{} Session {}", expected_platform, idx),
                "type": "group",
                "messages": [
                    {
                        "sender_id": format!("{}_sender", expected_platform),
                        "sender_name": format!("{} User", expected_platform),
                        "timestamp": base_ts,
                        "msg_type": 0,
                        "content": format!("{} message A", expected_platform)
                    },
                    {
                        "sender_id": format!("{}_sender", expected_platform),
                        "sender_name": format!("{} User", expected_platform),
                        "timestamp": base_ts + 1,
                        "msg_type": 0,
                        "content": format!("{} message B", expected_platform)
                    }
                ]
            }),
        )?;

        let (status, incremental_after_change) =
            post_json(&app, &incremental_path, json!({ "file_path": export_file })).await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(incremental_after_change["success"], true);
        assert_eq!(incremental_after_change["newMessageCount"], 1);
        assert_eq!(incremental_after_change["duplicateCount"], 1);
        assert_eq!(incremental_after_change["totalInFile"], 2);
        assert!(incremental_after_change.get("checkpointSkipped").is_none());

        let msg_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
            .bind(session_id)
            .fetch_one(&*pool)
            .await?;
        assert_eq!(msg_count, 2);
    }

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_supported_formats_include_all_17_platform_entries(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let app = chat::router();
    let (status, body) = get_json(&app, "/supported-formats").await?;
    assert_eq!(status, StatusCode::OK);

    let formats = body
        .as_array()
        .expect("supported-formats must return an array");
    let ids: std::collections::HashSet<String> = formats
        .iter()
        .filter_map(|item| item.get("id"))
        .filter_map(|id| id.as_str())
        .map(|id| id.to_string())
        .collect();

    let required_ids = vec![
        "wechat-json",
        "whatsapp-native-txt",
        "line-native-txt",
        "qq-native-txt",
        "telegram-native-json",
        "discord-export-json",
        "instagram-export-json",
        "imessage-export-json",
        "messenger-export-json",
        "kakaotalk-export-json",
        "slack-export-json",
        "teams-export-json",
        "signal-export-json",
        "skype-export-json",
        "googlechat-export-json",
        "zoom-export-json",
        "viber-export-json",
    ];

    for required in required_ids {
        assert!(
            ids.contains(required),
            "missing required supported format: {required}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_scan_multi_chat_and_import_with_chat_index() -> Result<(), Box<dyn std::error::Error>>
{
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_multichat.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let export_path = test_root.join("telegram_multi_chat_export.json");
    write_json_file(
        &export_path,
        &json!({
            "chats": {
                "list": [
                    {
                        "id": 1,
                        "name": "Room Alpha",
                        "type": "group",
                        "messages": [
                            {
                                "sender_id": "alpha_a",
                                "sender_name": "Alpha A",
                                "timestamp": 1900000001,
                                "msg_type": 0,
                                "content": "alpha message 1"
                            }
                        ]
                    },
                    {
                        "id": 2,
                        "name": "Room Beta",
                        "type": "private",
                        "messages": [
                            {
                                "sender_id": "beta_a",
                                "sender_name": "Beta A",
                                "timestamp": 1900000010,
                                "msg_type": 0,
                                "content": "beta message 1"
                            },
                            {
                                "sender_id": "beta_b",
                                "sender_name": "Beta B",
                                "timestamp": 1900000011,
                                "msg_type": 0,
                                "content": "beta message 2"
                            }
                        ]
                    }
                ]
            }
        }),
    )?;
    let export_file = export_path.to_string_lossy().to_string();

    let (status, scan_resp) = post_json(
        &app,
        "/scan-multi-chat-file",
        json!({ "file_path": export_file }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(scan_resp["success"], true);
    let chats = scan_resp["chats"]
        .as_array()
        .expect("scan response should include chats array");
    assert_eq!(chats.len(), 2);
    assert_eq!(chats[0]["name"], "Room Alpha");
    assert_eq!(chats[1]["name"], "Room Beta");

    let (status, import_resp) = post_json(
        &app,
        "/import-with-options",
        json!({
            "file_path": export_file,
            "format_options": {
                "chatIndex": 1
            }
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(import_resp["success"], true);
    let session_id = import_resp["sessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("import should return sessionId");

    let chat_meta = repo
        .get_chat(session_id)
        .await?
        .expect("session must exist after import");
    assert_eq!(chat_meta.name, "Room Beta");
    assert_eq!(chat_meta.platform, "telegram");
    assert_eq!(chat_meta.chat_type, "private");

    let message_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
        .bind(session_id)
        .fetch_one(&*pool)
        .await?;
    assert_eq!(message_count, 2);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_merged_mode_writes_single_session_and_checkpoints(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let file_a_path = test_root.join("telegram_authorized_batch_a.json");
    let file_b_path = test_root.join("telegram_authorized_batch_b.json");
    write_json_file(
        &file_a_path,
        &json!({
            "name": "Batch A",
            "type": "group",
            "messages": [
                {
                    "sender_id": "tg_user_1",
                    "sender_name": "TG User 1",
                    "timestamp": 2000000001,
                    "msg_type": 0,
                    "content": "message shared"
                }
            ]
        }),
    )?;
    write_json_file(
        &file_b_path,
        &json!({
            "name": "Batch B",
            "type": "group",
            "messages": [
                {
                    "sender_id": "tg_user_1",
                    "sender_name": "TG User 1",
                    "timestamp": 2000000001,
                    "msg_type": 0,
                    "content": "message shared"
                },
                {
                    "sender_id": "tg_user_2",
                    "sender_name": "TG User 2",
                    "timestamp": 2000000002,
                    "msg_type": 0,
                    "content": "message unique"
                }
            ]
        }),
    )?;

    let file_a = file_a_path.to_string_lossy().to_string();
    let file_b = file_b_path.to_string_lossy().to_string();
    let (status, batch_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [file_a, file_b],
            "merge": true,
            "mergedSessionName": "Merged Authorized Batch"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(batch_resp["success"], true);
    assert_eq!(batch_resp["mode"], "merged");
    assert_eq!(batch_resp["importedFiles"], 2);
    assert_eq!(batch_resp["failedFiles"], 0);
    assert_eq!(batch_resp["totalInsertedMessages"], 2);
    assert_eq!(batch_resp["totalDuplicateMessages"], 1);

    let session_id = batch_resp["mergedSessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("merged batch import should return mergedSessionId");
    let chat_meta = repo
        .get_chat(session_id)
        .await?
        .expect("merged chat must exist");
    assert_eq!(chat_meta.name, "Merged Authorized Batch");
    assert_eq!(chat_meta.platform, "telegram");

    let message_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
        .bind(session_id)
        .fetch_one(&*pool)
        .await?;
    assert_eq!(message_count, 2);

    let checkpoint_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM import_source_checkpoint WHERE source_kind = ?1 AND meta_id = ?2",
    )
    .bind("api-import-batch-merged")
    .bind(session_id)
    .fetch_one(&*pool)
    .await?;
    assert_eq!(checkpoint_count, 2);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_merged_mode_orders_messages_by_timestamp_across_sources(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_ordering.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let app = chat::router();

    let late_path = test_root.join("telegram_batch_late.json");
    let middle_path = test_root.join("telegram_batch_middle.json");
    let early_path = test_root.join("telegram_batch_early.json");

    write_json_file(
        &late_path,
        &json!({
            "name": "Late Source",
            "type": "group",
            "messages": [
                {
                    "sender_id": "tg_late",
                    "sender_name": "Late User",
                    "timestamp": 2300000003u64,
                    "msg_type": 0,
                    "content": "late message"
                }
            ]
        }),
    )?;
    write_json_file(
        &middle_path,
        &json!({
            "name": "Middle Source",
            "type": "group",
            "messages": [
                {
                    "sender_id": "tg_middle",
                    "sender_name": "Middle User",
                    "timestamp": 2300000002u64,
                    "msg_type": 0,
                    "content": "middle message"
                }
            ]
        }),
    )?;
    write_json_file(
        &early_path,
        &json!({
            "name": "Early Source",
            "type": "group",
            "messages": [
                {
                    "sender_id": "tg_early",
                    "sender_name": "Early User",
                    "timestamp": 2300000001u64,
                    "msg_type": 0,
                    "content": "early message"
                }
            ]
        }),
    )?;

    let (status, batch_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [
                late_path.to_string_lossy().to_string(),
                middle_path.to_string_lossy().to_string(),
                early_path.to_string_lossy().to_string()
            ],
            "merge": true,
            "mergedSessionName": "Merged Ordering Check"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(batch_resp["success"], true);
    assert_eq!(batch_resp["mode"], "merged");
    assert_eq!(batch_resp["totalInsertedMessages"], 3);
    assert_eq!(batch_resp["totalDuplicateMessages"], 0);

    let session_id = batch_resp["mergedSessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("merged import should return mergedSessionId");

    let inserted_timestamps: Vec<i64> =
        sqlx::query_scalar("SELECT ts FROM message WHERE meta_id = ?1 ORDER BY id ASC")
            .bind(session_id)
            .fetch_all(&*pool)
            .await?;
    assert_eq!(
        inserted_timestamps,
        vec![2300000001, 2300000002, 2300000003]
    );

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_merged_mode_checkpoint_skip_without_new_session(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_checkpoint_skip.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let file_path = test_root.join("telegram_authorized_batch_single.json");
    write_json_file(
        &file_path,
        &json!({
            "name": "Checkpoint Skip Batch",
            "type": "group",
            "messages": [
                {
                    "sender_id": "tg_ckpt_1",
                    "sender_name": "TG CKPT 1",
                    "timestamp": 2200000001u64,
                    "msg_type": 0,
                    "content": "checkpoint base message"
                }
            ]
        }),
    )?;
    let file = file_path.to_string_lossy().to_string();

    let (status, first_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [file],
            "merge": true,
            "mergedSessionName": "Merged Checkpoint Skip"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_resp["success"], true);
    assert_eq!(first_resp["importedFiles"], 1);
    assert_eq!(first_resp["skippedFiles"], 0);
    let first_session_id = first_resp["mergedSessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("first merged import should create a session");

    let first_meta_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM meta")
        .fetch_one(&*pool)
        .await?;
    assert_eq!(first_meta_count, 1);

    let (status, second_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [file.clone()],
            "merge": true,
            "mergedSessionName": "Merged Checkpoint Skip"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second_resp["success"], true);
    assert_eq!(second_resp["checkpointOnly"], true);
    assert_eq!(second_resp["importedFiles"], 0);
    assert_eq!(second_resp["failedFiles"], 0);
    assert_eq!(second_resp["skippedFiles"], 1);
    assert_eq!(second_resp["mergedSessionId"], serde_json::Value::Null);
    let items = second_resp["items"]
        .as_array()
        .expect("items should be present on second merged import");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["checkpointSkipped"], true);

    let second_meta_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM meta")
        .fetch_one(&*pool)
        .await?;
    assert_eq!(second_meta_count, 1);

    let checkpoint = repo
        .get_import_source_checkpoint("api-import-batch-merged", &file)
        .await?
        .expect("merged checkpoint should exist");
    assert_eq!(checkpoint.status, "completed");
    assert_eq!(checkpoint.meta_id, Some(first_session_id));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_merged_mode_mixed_failure_and_retry_reconciliation(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_mixed_reconcile.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let ok_file_path = test_root.join("telegram_batch_ok.json");
    let flaky_file_path = test_root.join("telegram_batch_flaky.json");

    write_json_file(
        &ok_file_path,
        &json!({
            "name": "Batch OK",
            "type": "group",
            "messages": [
                {
                    "sender_id": "tg_ok_user",
                    "sender_name": "OK User",
                    "timestamp": 2400000001u64,
                    "msg_type": 0,
                    "content": "ok line"
                }
            ]
        }),
    )?;
    fs::write(&flaky_file_path, b"this-is-not-json")?;

    let ok_file = ok_file_path.to_string_lossy().to_string();
    let flaky_file = flaky_file_path.to_string_lossy().to_string();

    let (status, first_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [ok_file, flaky_file],
            "merge": true,
            "mergedSessionName": "Merged Mixed Failure"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_resp["mode"], "merged");
    assert_eq!(first_resp["success"], true);
    assert_eq!(first_resp["importedFiles"], 1);
    assert_eq!(first_resp["failedFiles"], 1);
    assert_eq!(first_resp["skippedFiles"], 0);
    assert_eq!(first_resp["totalInsertedMessages"], 1);

    let first_session_id = first_resp["mergedSessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("first merged response should include session id");

    let first_message_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
            .bind(first_session_id)
            .fetch_one(&*pool)
            .await?;
    assert_eq!(first_message_count, 1);

    let ok_checkpoint_first = repo
        .get_import_source_checkpoint(
            "api-import-batch-merged",
            ok_file_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("ok file checkpoint should exist");
    assert_eq!(ok_checkpoint_first.status, "completed");
    assert_eq!(ok_checkpoint_first.meta_id, Some(first_session_id));

    let flaky_checkpoint_first = repo
        .get_import_source_checkpoint(
            "api-import-batch-merged",
            flaky_file_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("flaky file checkpoint should exist");
    assert_eq!(flaky_checkpoint_first.status, "failed");
    assert!(flaky_checkpoint_first.meta_id.is_none());

    write_json_file(
        &flaky_file_path,
        &json!({
            "name": "Batch Flaky Recovered",
            "type": "group",
            "messages": [
                {
                    "sender_id": "tg_flaky_user",
                    "sender_name": "Recovered User",
                    "timestamp": 2400000010u64,
                    "msg_type": 0,
                    "content": "recovered line"
                }
            ]
        }),
    )?;

    let (status, retry_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [flaky_file_path.to_string_lossy().to_string()],
            "merge": true,
            "mergedSessionName": "Merged Mixed Failure Retry"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retry_resp["mode"], "merged");
    assert_eq!(retry_resp["success"], true);
    assert_eq!(retry_resp["importedFiles"], 1);
    assert_eq!(retry_resp["failedFiles"], 0);
    assert_eq!(retry_resp["skippedFiles"], 0);
    assert_eq!(retry_resp["totalInsertedMessages"], 1);

    let retry_session_id = retry_resp["mergedSessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("retry merged response should include session id");
    assert_ne!(retry_session_id, first_session_id);

    let flaky_checkpoint_retry = repo
        .get_import_source_checkpoint(
            "api-import-batch-merged",
            flaky_file_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("flaky file checkpoint should exist after retry");
    assert_eq!(flaky_checkpoint_retry.status, "completed");
    assert_eq!(flaky_checkpoint_retry.meta_id, Some(retry_session_id));
    assert_eq!(flaky_checkpoint_retry.last_inserted_messages, 1);
    assert_eq!(flaky_checkpoint_retry.last_duplicate_messages, 0);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_merged_mode_does_not_apply_separate_retry_loop(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_merged_retry_boundary.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let repo = Repository::new(xenobot_api::database::get_pool().await?);
    let app = chat::router();

    let ok_path = test_root.join("telegram_merged_retry_boundary_ok.json");
    let bad_path = test_root.join("telegram_merged_retry_boundary_bad.json");
    write_json_file(
        &ok_path,
        &json!({
            "name": "Merged Retry Boundary OK",
            "type": "group",
            "messages": [
                {
                    "sender_id": "ok_u",
                    "sender_name": "Ok U",
                    "timestamp": 2660000001u64,
                    "msg_type": 0,
                    "content": "ok"
                }
            ]
        }),
    )?;
    fs::write(&bad_path, b"bad-json-payload")?;

    let (status, resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [
                ok_path.to_string_lossy().to_string(),
                bad_path.to_string_lossy().to_string()
            ],
            "merge": true,
            "retryFailed": true,
            "maxRetries": 8,
            "mergedSessionName": "Merged Retry Boundary"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resp["mode"], "merged");
    assert_eq!(resp["success"], true);
    assert_eq!(resp["importedFiles"], 1);
    assert_eq!(resp["failedFiles"], 1);
    assert_eq!(resp["skippedFiles"], 0);

    let items = resp["items"]
        .as_array()
        .expect("merged response should include per-file items");
    assert_eq!(items.len(), 2);

    let ok_item = items
        .iter()
        .find(|item| item["filePath"] == ok_path.to_string_lossy().to_string())
        .expect("ok item should exist");
    assert_eq!(ok_item["success"], true);
    assert!(
        ok_item.get("attemptsUsed").is_none(),
        "merged mode should not expose separate-mode retry counters"
    );

    let failed_item = items
        .iter()
        .find(|item| item["filePath"] == bad_path.to_string_lossy().to_string())
        .expect("failed item should exist");
    assert_eq!(failed_item["success"], false);
    assert!(
        failed_item.get("attemptsUsed").is_none(),
        "merged mode should not expose separate-mode retry counters"
    );

    let failed_checkpoint = repo
        .get_import_source_checkpoint(
            "api-import-batch-merged",
            bad_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("failed merged source checkpoint should exist");
    assert_eq!(failed_checkpoint.status, "failed");
    assert!(failed_checkpoint.meta_id.is_none());

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_separate_mode_retry_and_checkpoint_skip(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_separate.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let valid_path = test_root.join("wechat_authorized_batch_valid.json");
    let invalid_path = test_root.join("wechat_authorized_batch_invalid.json");
    write_json_file(
        &valid_path,
        &json!({
            "name": "Separate Batch Valid",
            "type": "group",
            "messages": [
                {
                    "sender_id": "wx_u1",
                    "sender_name": "WX U1",
                    "timestamp": 2100000001,
                    "msg_type": 0,
                    "content": "valid message"
                }
            ]
        }),
    )?;
    fs::write(&invalid_path, "not_a_valid_json_payload")?;

    let valid_file = valid_path.to_string_lossy().to_string();
    let invalid_file = invalid_path.to_string_lossy().to_string();
    let (status, first_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [valid_file, invalid_file],
            "merge": false,
            "retryFailed": true,
            "maxRetries": 2
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_resp["mode"], "separate");
    assert_eq!(first_resp["success"], true);
    assert_eq!(first_resp["totalFiles"], 2);
    assert_eq!(first_resp["importedFiles"], 1);
    assert_eq!(first_resp["failedFiles"], 1);
    assert_eq!(first_resp["skippedFiles"], 0);
    let first_items = first_resp["items"]
        .as_array()
        .expect("separate batch should return items array");
    assert_eq!(first_items.len(), 2);

    let invalid_item = first_items
        .iter()
        .find(|item| item["filePath"] == invalid_file)
        .expect("invalid file item should exist");
    assert_eq!(invalid_item["attemptsUsed"], 3);
    assert_eq!(invalid_item["result"]["success"], false);

    let valid_item = first_items
        .iter()
        .find(|item| item["filePath"] == valid_file)
        .expect("valid file item should exist");
    assert_eq!(valid_item["attemptsUsed"], 1);
    assert_eq!(valid_item["result"]["success"], true);

    let valid_checkpoint = repo
        .get_import_source_checkpoint("api-import-batch-separate", &valid_file)
        .await?
        .expect("valid source checkpoint should exist");
    assert_eq!(valid_checkpoint.status, "completed");
    assert_eq!(valid_checkpoint.platform.as_deref(), Some("wechat"));
    assert_eq!(
        valid_checkpoint.chat_name.as_deref(),
        Some("Separate Batch Valid")
    );

    let invalid_checkpoint = repo
        .get_import_source_checkpoint("api-import-batch-separate", &invalid_file)
        .await?
        .expect("invalid source checkpoint should exist");
    assert_eq!(invalid_checkpoint.status, "failed");
    assert_eq!(invalid_checkpoint.platform.as_deref(), Some("wechat"));

    let (status, second_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [valid_file],
            "merge": false,
            "retryFailed": true,
            "maxRetries": 2
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second_resp["success"], true);
    assert_eq!(second_resp["importedFiles"], 0);
    assert_eq!(second_resp["failedFiles"], 0);
    assert_eq!(second_resp["skippedFiles"], 1);
    let second_items = second_resp["items"]
        .as_array()
        .expect("second separate batch should return items array");
    assert_eq!(second_items.len(), 1);
    assert_eq!(second_items[0]["checkpointSkipped"], true);
    assert_eq!(second_items[0]["attemptsUsed"], 0);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_separate_mode_failed_checkpoint_can_recover_after_file_fix(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_separate_recovery.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let flaky_path = test_root.join("wechat_authorized_batch_recoverable.json");
    fs::write(&flaky_path, "invalid_json_payload")?;
    let flaky_file = flaky_path.to_string_lossy().to_string();

    let (status, failed_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [flaky_file],
            "merge": false,
            "retryFailed": true,
            "maxRetries": 1
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(failed_resp["mode"], "separate");
    assert_eq!(failed_resp["success"], false);
    assert_eq!(failed_resp["importedFiles"], 0);
    assert_eq!(failed_resp["failedFiles"], 1);
    let failed_items = failed_resp["items"]
        .as_array()
        .expect("failed separate response should include items");
    assert_eq!(failed_items.len(), 1);
    assert_eq!(failed_items[0]["attemptsUsed"], 2);
    assert_eq!(failed_items[0]["result"]["success"], false);

    let checkpoint_failed = repo
        .get_import_source_checkpoint("api-import-batch-separate", &flaky_file)
        .await?
        .expect("failed checkpoint should exist");
    assert_eq!(checkpoint_failed.status, "failed");
    assert!(checkpoint_failed.meta_id.is_none());
    let failed_fingerprint = checkpoint_failed.fingerprint.clone();

    write_json_file(
        &flaky_path,
        &json!({
            "name": "Recovered Separate Source",
            "type": "group",
            "messages": [
                {
                    "sender_id": "wx_recover",
                    "sender_name": "WX Recover",
                    "timestamp": 2110000001,
                    "msg_type": 0,
                    "content": "recovered message"
                }
            ]
        }),
    )?;

    let (status, recovered_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [flaky_file],
            "merge": false,
            "retryFailed": true,
            "maxRetries": 1
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(recovered_resp["mode"], "separate");
    assert_eq!(recovered_resp["success"], true);
    assert_eq!(recovered_resp["importedFiles"], 1);
    assert_eq!(recovered_resp["failedFiles"], 0);
    let recovered_items = recovered_resp["items"]
        .as_array()
        .expect("recovered separate response should include items");
    assert_eq!(recovered_items.len(), 1);
    assert_eq!(recovered_items[0]["attemptsUsed"], 1);
    assert_eq!(recovered_items[0]["result"]["success"], true);

    let recovered_session_id = recovered_items[0]["result"]["sessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("successful recovery should include sessionId");

    let checkpoint_recovered = repo
        .get_import_source_checkpoint("api-import-batch-separate", &flaky_file)
        .await?
        .expect("recovered checkpoint should exist");
    assert_eq!(checkpoint_recovered.status, "completed");
    assert_eq!(checkpoint_recovered.meta_id, Some(recovered_session_id));
    assert_eq!(checkpoint_recovered.last_inserted_messages, 1);
    assert_eq!(checkpoint_recovered.last_duplicate_messages, 0);
    assert_ne!(checkpoint_recovered.fingerprint, failed_fingerprint);

    let message_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
        .bind(recovered_session_id)
        .fetch_one(&*pool)
        .await?;
    assert_eq!(message_count, 1);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_separate_mode_retry_boundary_controls_attempts(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_retry_boundary.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let repo = Repository::new(xenobot_api::database::get_pool().await?);
    let app = chat::router();

    let invalid_path = test_root.join("wechat_authorized_batch_retry_boundary_invalid.json");
    fs::write(&invalid_path, "still_invalid_payload")?;
    let invalid_file = invalid_path.to_string_lossy().to_string();

    let (status, no_retry_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [invalid_file],
            "merge": false,
            "retryFailed": false,
            "maxRetries": 99
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(no_retry_resp["success"], false);
    let no_retry_item = no_retry_resp["items"]
        .as_array()
        .and_then(|arr| arr.first())
        .expect("no-retry response should include one item");
    assert_eq!(no_retry_item["attemptsUsed"], 1);
    assert_eq!(no_retry_item["result"]["success"], false);

    let (status, zero_retry_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [invalid_file],
            "merge": false,
            "retryFailed": true,
            "maxRetries": 0
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(zero_retry_resp["success"], false);
    let zero_retry_item = zero_retry_resp["items"]
        .as_array()
        .and_then(|arr| arr.first())
        .expect("zero-retry response should include one item");
    assert_eq!(zero_retry_item["attemptsUsed"], 1);
    assert_eq!(zero_retry_item["result"]["success"], false);

    let (status, bounded_retry_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [invalid_file],
            "merge": false,
            "retryFailed": true,
            "maxRetries": 2
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bounded_retry_resp["success"], false);
    let bounded_retry_item = bounded_retry_resp["items"]
        .as_array()
        .and_then(|arr| arr.first())
        .expect("bounded-retry response should include one item");
    assert_eq!(bounded_retry_item["attemptsUsed"], 3);
    assert_eq!(bounded_retry_item["result"]["success"], false);

    let checkpoint = repo
        .get_import_source_checkpoint("api-import-batch-separate", &invalid_file)
        .await?
        .expect("retry boundary checkpoint should exist");
    assert_eq!(checkpoint.status, "failed");
    assert!(checkpoint.meta_id.is_none());

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_separate_mode_skips_duplicate_input_paths(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_separate_duplicate_paths.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let valid_path = test_root.join("wechat_authorized_batch_duplicate.json");
    write_json_file(
        &valid_path,
        &json!({
            "name": "Separate Duplicate Path",
            "type": "group",
            "messages": [
                {
                    "sender_id": "wx_u1",
                    "sender_name": "WX U1",
                    "timestamp": 2100000101,
                    "msg_type": 0,
                    "content": "duplicate path baseline"
                }
            ]
        }),
    )?;

    let valid_file = valid_path.to_string_lossy().to_string();
    let (status, batch_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [valid_file, valid_file],
            "merge": false,
            "retryFailed": true,
            "maxRetries": 1
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(batch_resp["mode"], "separate");
    assert_eq!(batch_resp["success"], true);
    assert_eq!(batch_resp["importedFiles"], 1);
    assert_eq!(batch_resp["failedFiles"], 0);
    assert_eq!(batch_resp["skippedFiles"], 1);

    let items = batch_resp["items"]
        .as_array()
        .expect("separate batch should return items");
    assert_eq!(items.len(), 2);
    let duplicate_item = items
        .iter()
        .find(|item| item["duplicateInputSkipped"] == true)
        .expect("duplicate input item should exist");
    assert_eq!(duplicate_item["attemptsUsed"], 0);

    let imported_item = items
        .iter()
        .find(|item| item["result"]["success"] == true && item["duplicateInputSkipped"] != true)
        .expect("imported item should exist");
    assert_eq!(imported_item["attemptsUsed"], 1);
    let session_id = imported_item["result"]["sessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("successful separate import should include sessionId");

    let message_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
        .bind(session_id)
        .fetch_one(&*pool)
        .await?;
    assert_eq!(message_count, 1);

    let checkpoint = repo
        .get_import_source_checkpoint("api-import-batch-separate", &valid_file)
        .await?
        .expect("checkpoint should exist for imported file");
    assert_eq!(checkpoint.status, "completed");
    assert_eq!(checkpoint.last_inserted_messages, 1);
    assert_eq!(checkpoint.last_duplicate_messages, 0);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_merged_mode_skips_duplicate_input_paths(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_merged_duplicate_paths.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let export_path = test_root.join("wechat_merged_duplicate_path.json");
    write_json_file(
        &export_path,
        &json!({
            "name": "Merged Duplicate Path",
            "type": "group",
            "messages": [
                {
                    "sender_id": "wx_u1",
                    "sender_name": "WX U1",
                    "timestamp": 2100000201,
                    "msg_type": 0,
                    "content": "merged duplicate path baseline"
                }
            ]
        }),
    )?;
    let export_file = export_path.to_string_lossy().to_string();

    let (status, batch_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [export_file, export_file],
            "merge": true,
            "mergedSessionName": "Merged Duplicate Path"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(batch_resp["mode"], "merged");
    assert_eq!(batch_resp["success"], true);
    assert_eq!(batch_resp["importedFiles"], 1);
    assert_eq!(batch_resp["failedFiles"], 0);
    assert_eq!(batch_resp["skippedFiles"], 1);
    assert_eq!(batch_resp["totalInsertedMessages"], 1);
    assert_eq!(batch_resp["totalDuplicateMessages"], 0);

    let items = batch_resp["items"]
        .as_array()
        .expect("merged batch should return items");
    assert_eq!(items.len(), 2);
    assert!(items
        .iter()
        .any(|item| item["duplicateInputSkipped"] == true && item["success"] == true));

    let session_id = batch_resp["mergedSessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("merged import should include mergedSessionId");
    let message_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
        .bind(session_id)
        .fetch_one(&*pool)
        .await?;
    assert_eq!(message_count, 1);

    let checkpoint = repo
        .get_import_source_checkpoint("api-import-batch-merged", &export_file)
        .await?
        .expect("merged checkpoint should exist for imported file");
    assert_eq!(checkpoint.status, "completed");
    assert_eq!(checkpoint.last_inserted_messages, 1);
    assert_eq!(checkpoint.last_duplicate_messages, 0);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_merged_mode_dedup_and_checkpoint_consistency_in_single_batch(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_merged_dedup_checkpoint.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let file_a_path = test_root.join("telegram_merged_consistency_a.json");
    let file_b_path = test_root.join("telegram_merged_consistency_b.json");

    write_json_file(
        &file_a_path,
        &json!({
            "name": "Merged Consistency A",
            "type": "group",
            "messages": [
                {
                    "sender_id": "tg_consistency_1",
                    "sender_name": "User A1",
                    "timestamp": 2500000001u64,
                    "msg_type": 0,
                    "content": "m1"
                },
                {
                    "sender_id": "tg_consistency_2",
                    "sender_name": "User A2",
                    "timestamp": 2500000002u64,
                    "msg_type": 0,
                    "content": "m2"
                }
            ]
        }),
    )?;
    write_json_file(
        &file_b_path,
        &json!({
            "name": "Merged Consistency B",
            "type": "group",
            "messages": [
                {
                    "sender_id": "tg_consistency_2",
                    "sender_name": "User A2",
                    "timestamp": 2500000002u64,
                    "msg_type": 0,
                    "content": "m2"
                },
                {
                    "sender_id": "tg_consistency_3",
                    "sender_name": "User B1",
                    "timestamp": 2500000003u64,
                    "msg_type": 0,
                    "content": "m3"
                }
            ]
        }),
    )?;

    let file_a = file_a_path.to_string_lossy().to_string();
    let file_b = file_b_path.to_string_lossy().to_string();
    let (status, batch_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [file_a, file_b, file_b],
            "merge": true,
            "mergedSessionName": "Merged Consistency Regression"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(batch_resp["mode"], "merged");
    assert_eq!(batch_resp["success"], true);
    assert_eq!(batch_resp["totalFiles"], 3);
    assert_eq!(batch_resp["importedFiles"], 2);
    assert_eq!(batch_resp["failedFiles"], 0);
    assert_eq!(batch_resp["skippedFiles"], 1);
    assert_eq!(batch_resp["totalInsertedMessages"], 3);
    assert_eq!(batch_resp["totalDuplicateMessages"], 1);

    let items = batch_resp["items"]
        .as_array()
        .expect("merged consistency response should include items");
    assert_eq!(items.len(), 3);

    let imported_a = items
        .iter()
        .find(|item| item["filePath"] == file_a_path.to_string_lossy().to_string())
        .expect("source A item should exist");
    assert_eq!(imported_a["success"], true);
    assert_eq!(imported_a["insertedMessages"], 2);
    assert_eq!(imported_a["duplicateMessages"], 0);

    let imported_b = items
        .iter()
        .find(|item| {
            item["filePath"] == file_b_path.to_string_lossy().to_string()
                && item["duplicateInputSkipped"] != true
        })
        .expect("source B imported item should exist");
    assert_eq!(imported_b["success"], true);
    assert_eq!(imported_b["insertedMessages"], 1);
    assert_eq!(imported_b["duplicateMessages"], 1);

    assert!(items
        .iter()
        .any(|item| item["duplicateInputSkipped"] == true && item["success"] == true));

    let session_id = batch_resp["mergedSessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("merged consistency import should include mergedSessionId");
    let message_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
        .bind(session_id)
        .fetch_one(&*pool)
        .await?;
    assert_eq!(message_count, 3);

    let checkpoint_a = repo
        .get_import_source_checkpoint(
            "api-import-batch-merged",
            file_a_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("checkpoint for source A should exist");
    assert_eq!(checkpoint_a.status, "completed");
    assert_eq!(checkpoint_a.meta_id, Some(session_id));
    assert_eq!(checkpoint_a.last_inserted_messages, 2);
    assert_eq!(checkpoint_a.last_duplicate_messages, 0);

    let checkpoint_b = repo
        .get_import_source_checkpoint(
            "api-import-batch-merged",
            file_b_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("checkpoint for source B should exist");
    assert_eq!(checkpoint_b.status, "completed");
    assert_eq!(checkpoint_b.meta_id, Some(session_id));
    assert_eq!(checkpoint_b.last_inserted_messages, 1);
    assert_eq!(checkpoint_b.last_duplicate_messages, 1);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_merged_mode_preserves_distinct_platform_message_ids(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_merged_platform_message_id.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let app = chat::router();

    let file_a_path = test_root.join("whatsapp_merged_pm_id_a.json");
    let file_b_path = test_root.join("whatsapp_merged_pm_id_b.json");

    write_json_file(
        &file_a_path,
        &json!({
            "name": "Merged PM-ID A",
            "type": "group",
            "messages": [
                {
                    "sender_id": "wa_pm_id_sender",
                    "sender_name": "Sender A",
                    "timestamp": 2600000001u64,
                    "msg_type": 0,
                    "content": "same-content",
                    "platform_message_id": "wa-msg-1"
                }
            ]
        }),
    )?;
    write_json_file(
        &file_b_path,
        &json!({
            "name": "Merged PM-ID B",
            "type": "group",
            "messages": [
                {
                    "sender_id": "wa_pm_id_sender",
                    "sender_name": "Sender A",
                    "timestamp": 2600000001u64,
                    "msg_type": 0,
                    "content": "same-content",
                    "platform_message_id": "wa-msg-2"
                }
            ]
        }),
    )?;

    let (status, batch_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [
                file_a_path.to_string_lossy().to_string(),
                file_b_path.to_string_lossy().to_string()
            ],
            "merge": true,
            "mergedSessionName": "Merged PM-ID Distinctness"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(batch_resp["success"], true);
    assert_eq!(batch_resp["mode"], "merged");
    assert_eq!(batch_resp["totalInsertedMessages"], 2);
    assert_eq!(batch_resp["totalDuplicateMessages"], 0);

    let session_id = batch_resp["mergedSessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("merged import should return mergedSessionId");

    let total_messages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
        .bind(session_id)
        .fetch_one(&*pool)
        .await?;
    assert_eq!(total_messages, 2);

    let distinct_platform_message_ids: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT platform_message_id) FROM message WHERE meta_id = ?1",
    )
    .bind(session_id)
    .fetch_one(&*pool)
    .await?;
    assert_eq!(distinct_platform_message_ids, 2);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_merged_mode_rerun_is_idempotent_with_checkpoint_only(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_merged_idempotent_rerun.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let file_a_path = test_root.join("telegram_merged_idempotent_a.json");
    let file_b_path = test_root.join("telegram_merged_idempotent_b.json");

    write_json_file(
        &file_a_path,
        &json!({
            "name": "Merged Idempotent A",
            "type": "group",
            "messages": [
                {
                    "sender_id": "tg_idempotent_1",
                    "sender_name": "ID User 1",
                    "timestamp": 2550000001u64,
                    "msg_type": 0,
                    "content": "same-shared"
                }
            ]
        }),
    )?;
    write_json_file(
        &file_b_path,
        &json!({
            "name": "Merged Idempotent B",
            "type": "group",
            "messages": [
                {
                    "sender_id": "tg_idempotent_1",
                    "sender_name": "ID User 1",
                    "timestamp": 2550000001u64,
                    "msg_type": 0,
                    "content": "same-shared"
                },
                {
                    "sender_id": "tg_idempotent_2",
                    "sender_name": "ID User 2",
                    "timestamp": 2550000002u64,
                    "msg_type": 0,
                    "content": "unique-after-shared"
                }
            ]
        }),
    )?;

    let file_a = file_a_path.to_string_lossy().to_string();
    let file_b = file_b_path.to_string_lossy().to_string();

    let (status, first_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [file_a, file_b],
            "merge": true,
            "mergedSessionName": "Merged Idempotent Baseline"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_resp["success"], true);
    assert_eq!(first_resp["mode"], "merged");
    assert_eq!(first_resp["importedFiles"], 2);
    assert_eq!(first_resp["failedFiles"], 0);
    assert_eq!(first_resp["skippedFiles"], 0);
    assert_eq!(first_resp["totalInsertedMessages"], 2);
    assert_eq!(first_resp["totalDuplicateMessages"], 1);

    let first_session_id = first_resp["mergedSessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("first merged run should include mergedSessionId");

    let first_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
        .bind(first_session_id)
        .fetch_one(&*pool)
        .await?;
    assert_eq!(first_count, 2);

    let (status, second_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [
                file_b_path.to_string_lossy().to_string(),
                file_a_path.to_string_lossy().to_string()
            ],
            "merge": true,
            "mergedSessionName": "Merged Idempotent Rerun"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second_resp["success"], true);
    assert_eq!(second_resp["mode"], "merged");
    assert_eq!(second_resp["checkpointOnly"], true);
    assert_eq!(second_resp["importedFiles"], 0);
    assert_eq!(second_resp["failedFiles"], 0);
    assert_eq!(second_resp["skippedFiles"], 2);
    assert_eq!(
        second_resp["totalInsertedMessages"].as_i64().unwrap_or(0),
        0
    );
    assert_eq!(
        second_resp["totalDuplicateMessages"].as_i64().unwrap_or(0),
        0
    );
    assert_eq!(second_resp["mergedSessionId"], serde_json::Value::Null);

    let second_items = second_resp["items"]
        .as_array()
        .expect("second merged run should include items");
    assert_eq!(second_items.len(), 2);
    assert!(second_items
        .iter()
        .all(|item| item["success"] == true && item["checkpointSkipped"] == true));

    let count_after_rerun: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
            .bind(first_session_id)
            .fetch_one(&*pool)
            .await?;
    assert_eq!(count_after_rerun, 2);

    let checkpoint_a = repo
        .get_import_source_checkpoint(
            "api-import-batch-merged",
            file_a_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("checkpoint A should exist");
    assert_eq!(checkpoint_a.status, "completed");
    assert_eq!(checkpoint_a.meta_id, Some(first_session_id));
    assert_eq!(checkpoint_a.last_inserted_messages, 1);
    assert_eq!(checkpoint_a.last_duplicate_messages, 0);

    let checkpoint_b = repo
        .get_import_source_checkpoint(
            "api-import-batch-merged",
            file_b_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("checkpoint B should exist");
    assert_eq!(checkpoint_b.status, "completed");
    assert_eq!(checkpoint_b.meta_id, Some(first_session_id));
    assert_eq!(checkpoint_b.last_inserted_messages, 1);
    assert_eq!(checkpoint_b.last_duplicate_messages, 1);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_whatsapp_native_txt_uses_analysis_parser_sender_identity(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_whatsapp_native_txt.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let export_path = test_root.join("whatsapp_native_chat.txt");
    fs::write(
        &export_path,
        "[12/25/2024, 09:30:01] Alice: hello\n[12/25/2024, 09:31:05] Bob: hi",
    )?;
    let export_file = export_path.to_string_lossy().to_string();

    let (status, detect_resp) =
        post_json(&app, "/detect-format", json!({ "file_path": export_file })).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detect_resp["platform"], "whatsapp");
    assert_eq!(detect_resp["parserSource"], "analysis");
    assert_eq!(detect_resp["format"], "analysis-whatsapp");

    let (status, import_resp) =
        post_json(&app, "/import", json!({ "file_path": export_file })).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(import_resp["success"], true);
    let session_id = import_resp["sessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("import should return sessionId");

    let chat_meta = repo
        .get_chat(session_id)
        .await?
        .expect("session must exist after import");
    assert_eq!(chat_meta.platform, "whatsapp");

    let distinct_senders: i64 =
        sqlx::query_scalar("SELECT COUNT(DISTINCT sender_id) FROM message WHERE meta_id = ?1")
            .bind(session_id)
            .fetch_one(&*pool)
            .await?;
    assert_eq!(distinct_senders, 2);

    let text_importer_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM member WHERE platform_id = 'text-importer'")
            .fetch_one(&*pool)
            .await?;
    assert_eq!(text_importer_rows, 0);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

fn analysis_platform_fixtures() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    vec![
        (
            "wechat",
            "json",
            r#"[{"msg_id":"1","type":1,"is_sender":false,"sender_name":"Alice","sender_id":"wxid_alice","create_time":1735813230,"content":"hello wechat"}]"#,
            "analysis-wechat",
        ),
        (
            "whatsapp",
            "txt",
            "[01/02/2025, 10:20:30] Alice: hello whatsapp",
            "analysis-whatsapp",
        ),
        (
            "line",
            "txt",
            "2025/01/02 10:20:30 Alice hello line",
            "analysis-line",
        ),
        (
            "qq",
            "txt",
            "[2025-01-02 10:20:30] Alice hello qq",
            "analysis-qq",
        ),
        (
            "telegram",
            "json",
            r#"{"name":"tg","messages":[{"from":"Alice","date":"2025-01-02T10:20:30Z","text":"hello telegram"}]}"#,
            "analysis-telegram",
        ),
        (
            "discord",
            "json",
            r#"[{"ID":"1","Timestamp":"2025-01-02T10:20:30Z","Author":{"ID":"u1","Name":"Alice"},"Content":"hello discord"}]"#,
            "analysis-discord",
        ),
        (
            "instagram",
            "json",
            r#"[{"sender":"Alice","timestamp":1735813230,"content":"hello instagram"}]"#,
            "analysis-instagram",
        ),
        (
            "imessage",
            "json",
            r#"[{"text":"hello imessage","sender":"Alice","date":"2025-01-02T10:20:30Z"}]"#,
            "analysis-imessage",
        ),
        (
            "messenger",
            "json",
            r#"[{"sender_name":"Alice","timestamp_ms":1735813230000,"content":"hello messenger"}]"#,
            "analysis-messenger",
        ),
        (
            "kakaotalk",
            "json",
            r#"[{"sender":"Alice","message":"hello kakao","date":"2025-01-02 10:20:30"}]"#,
            "analysis-kakaotalk",
        ),
        (
            "slack",
            "json",
            r#"[{"user":"U1","ts":"1735813230.000200","text":"hello slack"}]"#,
            "analysis-slack",
        ),
        (
            "teams",
            "json",
            r#"[{"from":"Alice","date":"2025-01-02T10:20:30Z","content":"hello teams"}]"#,
            "analysis-teams",
        ),
        (
            "signal",
            "json",
            r#"[{"sender":"Alice","timestamp":1735813230000,"body":"hello signal"}]"#,
            "analysis-signal",
        ),
        (
            "skype",
            "json",
            r#"[{"sender":"Alice","datetime":"2025-01-02T10:20:30Z","msg_content":"hello skype"}]"#,
            "analysis-skype",
        ),
        (
            "googlechat",
            "json",
            r#"[{"sender":{"name":"users/1","display_name":"Alice"},"create_time":"2025-01-02T10:20:30Z","text":"hello googlechat"}]"#,
            "analysis-googlechat",
        ),
        (
            "zoom",
            "json",
            r#"[{"sender":"Alice","timestamp":"2025-01-02T10:20:30Z","message":"hello zoom"}]"#,
            "analysis-zoom",
        ),
        (
            "viber",
            "json",
            r#"[{"sender":"Alice","date_time":"2025-01-02T10:20:30Z","text":"hello viber"}]"#,
            "analysis-viber",
        ),
    ]
}

#[tokio::test]
async fn test_detect_format_uses_analysis_parser_across_all_17_platform_fixtures(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_detect_analysis_matrix.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let app = chat::router();

    let fixtures = analysis_platform_fixtures();

    for (platform, ext, content, expected_format) in fixtures {
        let path = test_root.join(format!("{}_analysis_fixture.{}", platform, ext));
        fs::write(&path, content)?;
        let export_file = path.to_string_lossy().to_string();

        let (status, detect_resp) =
            post_json(&app, "/detect-format", json!({ "file_path": export_file })).await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(detect_resp["platform"], platform);
        assert_eq!(detect_resp["parserSource"], "analysis");
        assert_eq!(detect_resp["format"], expected_format);
    }

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_uses_analysis_parser_across_all_17_platform_fixtures(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_analysis_matrix.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    for (platform, ext, content, _expected_format) in analysis_platform_fixtures() {
        let path = test_root.join(format!("{}_analysis_import_fixture.{}", platform, ext));
        fs::write(&path, content)?;
        let export_file = path.to_string_lossy().to_string();

        let (status, import_resp) =
            post_json(&app, "/import", json!({ "file_path": export_file })).await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(import_resp["success"], true);

        let session_id = import_resp["sessionId"]
            .as_str()
            .and_then(|v| v.parse::<i64>().ok())
            .expect("import should return sessionId");

        let chat_meta = repo
            .get_chat(session_id)
            .await?
            .expect("session must exist after import");
        assert_eq!(chat_meta.platform, platform);

        let message_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
                .bind(session_id)
                .fetch_one(&*pool)
                .await?;
        assert!(
            message_count >= 1,
            "platform '{}' import should persist at least one message",
            platform
        );
    }

    let text_importer_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM member WHERE platform_id = 'text-importer'")
            .fetch_one(&*pool)
            .await?;
    assert_eq!(
        text_importer_rows, 0,
        "analysis-parser fixture imports should not create text-importer members"
    );

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_incremental_jsonl_partial_parse_no_messages_failure_and_recovery_checkpoint_consistency(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_incremental_jsonl_partial_recovery.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let incremental_path = test_root.join("telegram_incremental_partial.jsonl");
    fs::write(
        &incremental_path,
        concat!(
            "{\"sender_id\":\"tg_u1\",\"sender_name\":\"Alice\",\"timestamp\":2610000001,\"msg_type\":0,\"content\":\"first\"}\n",
            "{\"sender_id\":\"tg_u2\"\n",
            "{\"sender_id\":\"tg_u3\",\"sender_name\":\"Carol\",\"msg_type\":0,\"content\":\"missing ts\"}\n",
            "{\"sender_id\":\"tg_u2\",\"sender_name\":\"Bob\",\"timestamp\":2610000002,\"msg_type\":0,\"content\":\"second\"}\n"
        ),
    )?;
    let incremental_file = incremental_path.to_string_lossy().to_string();

    let (status, import_resp) =
        post_json(&app, "/import", json!({ "file_path": incremental_file })).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(import_resp["success"], true);
    assert_eq!(import_resp["diagnostics"]["detectedFormat"], "jsonl");
    assert_eq!(import_resp["diagnostics"]["messagesReceived"], 4);
    assert_eq!(import_resp["diagnostics"]["messagesWritten"], 2);
    assert_eq!(import_resp["diagnostics"]["messagesSkipped"], 2);

    let session_id = import_resp["sessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("import should return sessionId");
    let import_path = format!("/sessions/{session_id}/incremental-import");

    let (status, first_incremental_resp) = post_json(
        &app,
        &import_path,
        json!({ "file_path": incremental_path.to_string_lossy().to_string() }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_incremental_resp["success"], true);
    assert_eq!(first_incremental_resp["newMessageCount"], 0);
    assert_eq!(first_incremental_resp["duplicateCount"], 2);
    assert_eq!(first_incremental_resp["totalInFile"], 4);

    let source_kind = format!("api-incremental-{session_id}");
    let completed_checkpoint = repo
        .get_import_source_checkpoint(&source_kind, incremental_path.to_string_lossy().as_ref())
        .await?
        .expect("completed incremental checkpoint should exist");
    assert_eq!(completed_checkpoint.status, "completed");
    assert_eq!(completed_checkpoint.last_inserted_messages, 0);
    assert_eq!(completed_checkpoint.last_duplicate_messages, 2);

    fs::write(
        &incremental_path,
        concat!(
            "{\"sender_id\":\"tg_bad\"\n",
            "{\"sender_id\":\"tg_u4\",\"sender_name\":\"Delta\",\"msg_type\":0,\"content\":\"missing ts\"}\n"
        ),
    )?;

    let (status, failed_incremental_resp) = post_json(
        &app,
        &import_path,
        json!({ "file_path": incremental_path.to_string_lossy().to_string() }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(failed_incremental_resp["success"], false);
    assert_eq!(failed_incremental_resp["error"], "error.no_messages");

    let failed_checkpoint = repo
        .get_import_source_checkpoint(&source_kind, incremental_path.to_string_lossy().as_ref())
        .await?
        .expect("failed incremental checkpoint should exist");
    assert_eq!(failed_checkpoint.status, "failed");
    assert_eq!(failed_checkpoint.meta_id, Some(session_id));
    assert_eq!(failed_checkpoint.last_inserted_messages, 0);
    assert_eq!(failed_checkpoint.last_duplicate_messages, 0);
    assert!(failed_checkpoint
        .error_message
        .as_deref()
        .unwrap_or_default()
        .contains("no new messages"));
    assert_ne!(
        failed_checkpoint.fingerprint,
        completed_checkpoint.fingerprint
    );

    fs::write(
        &incremental_path,
        concat!(
            "{\"sender_id\":\"tg_u4\",\"sender_name\":\"Delta\",\"timestamp\":2610000003,\"msg_type\":0,\"content\":\"recovered\"}\n",
            "{\"sender_id\":\"tg_bad\"\n"
        ),
    )?;

    let (status, recovered_incremental_resp) = post_json(
        &app,
        &import_path,
        json!({ "file_path": incremental_path.to_string_lossy().to_string() }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(recovered_incremental_resp["success"], true);
    assert_eq!(recovered_incremental_resp["newMessageCount"], 1);
    assert_eq!(recovered_incremental_resp["duplicateCount"], 0);
    assert_eq!(recovered_incremental_resp["totalInFile"], 2);

    let recovered_checkpoint = repo
        .get_import_source_checkpoint(&source_kind, incremental_path.to_string_lossy().as_ref())
        .await?
        .expect("recovered incremental checkpoint should exist");
    assert_eq!(recovered_checkpoint.status, "completed");
    assert_eq!(recovered_checkpoint.meta_id, Some(session_id));
    assert_eq!(recovered_checkpoint.last_inserted_messages, 1);
    assert_eq!(recovered_checkpoint.last_duplicate_messages, 0);
    assert_ne!(
        recovered_checkpoint.fingerprint,
        failed_checkpoint.fingerprint
    );

    let session_message_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
            .bind(session_id)
            .fetch_one(&*pool)
            .await?;
    assert_eq!(session_message_count, 3);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_merged_mode_jsonl_no_messages_checkpoint_recovery(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_merged_jsonl_no_messages_recovery.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let ok_path = test_root.join("telegram_merged_jsonl_ok.jsonl");
    let no_messages_path = test_root.join("telegram_merged_jsonl_no_messages.jsonl");
    fs::write(
        &ok_path,
        concat!(
            "{\"sender_id\":\"tg_ok\",\"sender_name\":\"OK\",\"timestamp\":2620000001,\"msg_type\":0,\"content\":\"ok\"}\n",
            "{\"sender_id\":\"tg_bad\"\n"
        ),
    )?;
    fs::write(
        &no_messages_path,
        concat!(
            "{\"sender_id\":\"tg_bad\"\n",
            "{\"sender_id\":\"tg_skip\",\"sender_name\":\"Skip\",\"msg_type\":0,\"content\":\"missing ts\"}\n"
        ),
    )?;

    let ok_file = ok_path.to_string_lossy().to_string();
    let no_messages_file = no_messages_path.to_string_lossy().to_string();

    let (status, first_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [ok_file, no_messages_file],
            "merge": true,
            "mergedSessionName": "Merged JSONL NoMessages Recovery"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_resp["mode"], "merged");
    assert_eq!(first_resp["success"], true);
    assert_eq!(first_resp["importedFiles"], 1);
    assert_eq!(first_resp["failedFiles"], 1);
    assert_eq!(first_resp["skippedFiles"], 0);
    assert_eq!(first_resp["totalInsertedMessages"], 1);
    assert_eq!(first_resp["totalDuplicateMessages"], 0);

    let first_items = first_resp["items"]
        .as_array()
        .expect("first merged response should include items");
    assert_eq!(first_items.len(), 2);

    let no_messages_item = first_items
        .iter()
        .find(|item| item["filePath"] == no_messages_path.to_string_lossy().to_string())
        .expect("no-messages source item should exist");
    assert_eq!(no_messages_item["success"], false);
    assert_eq!(no_messages_item["error"], "error.no_messages");

    let ok_item = first_items
        .iter()
        .find(|item| item["filePath"] == ok_path.to_string_lossy().to_string())
        .expect("ok source item should exist");
    assert_eq!(ok_item["success"], true);
    assert_eq!(ok_item["insertedMessages"], 1);
    assert_eq!(ok_item["duplicateMessages"], 0);

    let first_session_id = first_resp["mergedSessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("first merged response should include session id");

    let first_session_messages: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
            .bind(first_session_id)
            .fetch_one(&*pool)
            .await?;
    assert_eq!(first_session_messages, 1);

    let ok_checkpoint_first = repo
        .get_import_source_checkpoint(
            "api-import-batch-merged",
            ok_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("ok checkpoint should exist after first merged import");
    assert_eq!(ok_checkpoint_first.status, "completed");
    assert_eq!(ok_checkpoint_first.meta_id, Some(first_session_id));
    assert_eq!(ok_checkpoint_first.last_inserted_messages, 1);
    assert_eq!(ok_checkpoint_first.last_duplicate_messages, 0);

    let no_messages_checkpoint_first = repo
        .get_import_source_checkpoint(
            "api-import-batch-merged",
            no_messages_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("no-messages checkpoint should exist after first merged import");
    assert_eq!(no_messages_checkpoint_first.status, "failed");
    assert!(no_messages_checkpoint_first.meta_id.is_none());
    assert_eq!(
        no_messages_checkpoint_first.error_message.as_deref(),
        Some("error.no_messages")
    );

    fs::write(
        &no_messages_path,
        concat!(
            "{\"sender_id\":\"tg_recover\",\"sender_name\":\"Recover\",\"timestamp\":2620000010,\"msg_type\":0,\"content\":\"recovered\"}\n",
            "{\"sender_id\":\"tg_bad\"\n"
        ),
    )?;

    let (status, second_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [
                ok_path.to_string_lossy().to_string(),
                no_messages_path.to_string_lossy().to_string()
            ],
            "merge": true,
            "mergedSessionName": "Merged JSONL NoMessages Recovery Retry"
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second_resp["mode"], "merged");
    assert_eq!(second_resp["success"], true);
    assert_eq!(second_resp["importedFiles"], 1);
    assert_eq!(second_resp["failedFiles"], 0);
    assert_eq!(second_resp["skippedFiles"], 1);
    assert_eq!(second_resp["totalInsertedMessages"], 1);
    assert_eq!(second_resp["totalDuplicateMessages"], 0);

    let second_items = second_resp["items"]
        .as_array()
        .expect("second merged response should include items");
    assert_eq!(second_items.len(), 2);
    assert!(second_items.iter().any(|item| {
        item["filePath"] == ok_path.to_string_lossy().to_string()
            && item["success"] == true
            && item["checkpointSkipped"] == true
    }));
    assert!(second_items.iter().any(|item| {
        item["filePath"] == no_messages_path.to_string_lossy().to_string()
            && item["success"] == true
            && item["insertedMessages"] == 1
            && item["duplicateMessages"] == 0
    }));

    let second_session_id = second_resp["mergedSessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("second merged response should include session id");
    assert_ne!(second_session_id, first_session_id);

    let second_session_messages: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
            .bind(second_session_id)
            .fetch_one(&*pool)
            .await?;
    assert_eq!(second_session_messages, 1);

    let no_messages_checkpoint_second = repo
        .get_import_source_checkpoint(
            "api-import-batch-merged",
            no_messages_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("no-messages checkpoint should exist after recovery import");
    assert_eq!(no_messages_checkpoint_second.status, "completed");
    assert_eq!(
        no_messages_checkpoint_second.meta_id,
        Some(second_session_id)
    );
    assert_eq!(no_messages_checkpoint_second.last_inserted_messages, 1);
    assert_eq!(no_messages_checkpoint_second.last_duplicate_messages, 0);

    let ok_checkpoint_second = repo
        .get_import_source_checkpoint(
            "api-import-batch-merged",
            ok_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("ok checkpoint should still exist after retry import");
    assert_eq!(ok_checkpoint_second.status, "completed");
    assert_eq!(ok_checkpoint_second.meta_id, Some(first_session_id));
    assert_eq!(ok_checkpoint_second.last_inserted_messages, 1);
    assert_eq!(ok_checkpoint_second.last_duplicate_messages, 0);

    // Deep consistency regression:
    // Ensure recovery run does not duplicate skipped source payload into the new merged session.
    let total_messages_across_two_sessions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id IN (?1, ?2)")
            .bind(first_session_id)
            .bind(second_session_id)
            .fetch_one(&*pool)
            .await?;
    assert_eq!(
        total_messages_across_two_sessions, 2,
        "expected exactly one message in each merged session after recovery"
    );

    let ok_in_first: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1 AND content = 'ok'")
            .bind(first_session_id)
            .fetch_one(&*pool)
            .await?;
    let ok_in_second: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1 AND content = 'ok'")
            .bind(second_session_id)
            .fetch_one(&*pool)
            .await?;
    assert_eq!(ok_in_first, 1);
    assert_eq!(ok_in_second, 0);

    let recovered_in_first: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM message WHERE meta_id = ?1 AND content = 'recovered'",
    )
    .bind(first_session_id)
    .fetch_one(&*pool)
    .await?;
    let recovered_in_second: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM message WHERE meta_id = ?1 AND content = 'recovered'",
    )
    .bind(second_session_id)
    .fetch_one(&*pool)
    .await?;
    assert_eq!(recovered_in_first, 0);
    assert_eq!(recovered_in_second, 1);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_import_batch_separate_mode_jsonl_no_messages_failure_then_recovery(
) -> Result<(), Box<dyn std::error::Error>> {
    let _test_guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_import_batch_separate_jsonl_no_messages.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;
    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let app = chat::router();

    let flaky_path = test_root.join("telegram_separate_jsonl_no_messages.jsonl");
    fs::write(
        &flaky_path,
        concat!(
            "{\"sender_id\":\"tg_bad\"\n",
            "{\"sender_id\":\"tg_skip\",\"sender_name\":\"Skip\",\"msg_type\":0,\"content\":\"missing ts\"}\n"
        ),
    )?;
    let flaky_file = flaky_path.to_string_lossy().to_string();

    let (status, first_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [flaky_file],
            "merge": false,
            "retryFailed": true,
            "maxRetries": 1
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_resp["mode"], "separate");
    assert_eq!(first_resp["success"], false);
    assert_eq!(first_resp["importedFiles"], 0);
    assert_eq!(first_resp["failedFiles"], 1);
    assert_eq!(first_resp["skippedFiles"], 0);

    let first_item = first_resp["items"]
        .as_array()
        .and_then(|arr| arr.first())
        .expect("first response should contain one item");
    assert_eq!(first_item["attemptsUsed"], 2);
    assert_eq!(first_item["result"]["success"], false);
    assert_eq!(first_item["result"]["error"], "error.no_messages");

    let checkpoint_failed = repo
        .get_import_source_checkpoint(
            "api-import-batch-separate",
            flaky_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("failed separate checkpoint should exist");
    assert_eq!(checkpoint_failed.status, "failed");
    assert_eq!(checkpoint_failed.platform.as_deref(), Some("telegram"));
    assert!(checkpoint_failed.meta_id.is_none());
    let failed_fingerprint = checkpoint_failed.fingerprint.clone();

    fs::write(
        &flaky_path,
        concat!(
            "{\"sender_id\":\"tg_recover\",\"sender_name\":\"Recover\",\"timestamp\":2630000001,\"msg_type\":0,\"content\":\"recovered\"}\n",
            "{\"sender_id\":\"tg_bad\"\n"
        ),
    )?;

    let (status, second_resp) = post_json(
        &app,
        "/import-batch",
        json!({
            "filePaths": [flaky_path.to_string_lossy().to_string()],
            "merge": false,
            "retryFailed": true,
            "maxRetries": 1
        }),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second_resp["mode"], "separate");
    assert_eq!(second_resp["success"], true);
    assert_eq!(second_resp["importedFiles"], 1);
    assert_eq!(second_resp["failedFiles"], 0);
    assert_eq!(second_resp["skippedFiles"], 0);

    let second_item = second_resp["items"]
        .as_array()
        .and_then(|arr| arr.first())
        .expect("second response should contain one item");
    assert_eq!(second_item["attemptsUsed"], 1);
    assert_eq!(second_item["result"]["success"], true);
    let session_id = second_item["result"]["sessionId"]
        .as_str()
        .and_then(|v| v.parse::<i64>().ok())
        .expect("recovered separate import should include sessionId");

    let checkpoint_recovered = repo
        .get_import_source_checkpoint(
            "api-import-batch-separate",
            flaky_path.to_string_lossy().as_ref(),
        )
        .await?
        .expect("recovered separate checkpoint should exist");
    assert_eq!(checkpoint_recovered.status, "completed");
    assert_eq!(checkpoint_recovered.meta_id, Some(session_id));
    assert_eq!(checkpoint_recovered.last_inserted_messages, 1);
    assert_eq!(checkpoint_recovered.last_duplicate_messages, 1);
    assert_ne!(checkpoint_recovered.fingerprint, failed_fingerprint);

    let message_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM message WHERE meta_id = ?1")
        .bind(session_id)
        .fetch_one(&*pool)
        .await?;
    assert_eq!(message_count, 1);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

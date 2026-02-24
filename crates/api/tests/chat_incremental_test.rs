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
    assert_eq!(second_analyze["checkpointSkipped"], true);
    assert_eq!(second_analyze["newMessageCount"], 0);
    assert_eq!(second_analyze["duplicateCount"], 0);
    assert_eq!(second_analyze["totalInFile"], 0);

    let (status, second_import) =
        post_json(&app, &import_path, json!({ "file_path": valid_export })).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second_import["success"], true);
    assert_eq!(second_import["checkpointSkipped"], true);
    assert_eq!(second_import["newMessageCount"], 0);
    assert_eq!(second_import["duplicateCount"], 0);

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

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{to_bytes, Body};
use axum::http::{Method, Request, StatusCode};
use tower::util::ServiceExt;
use xenobot_api::database::repository::{ChatMeta, Member, Message};
use xenobot_api::database::Repository;
use xenobot_api::router::build_router;
use xenobot_api::ApiConfig;
use xenobot_core::config::DatabaseConfig;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn unique_test_root() -> PathBuf {
    let epoch_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("xenobot_api_media_{}_{}", epoch_nanos, seq))
}

async fn get_response(
    app: &axum::Router,
    uri: &str,
) -> Result<axum::response::Response, Box<dyn std::error::Error>> {
    let request = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())?;
    let response = app.clone().oneshot(request).await?;
    Ok(response)
}

async fn post_json_response(
    app: &axum::Router,
    uri: &str,
    payload: serde_json::Value,
) -> Result<axum::response::Response, Box<dyn std::error::Error>> {
    let request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&payload)?))?;
    let response = app.clone().oneshot(request).await?;
    Ok(response)
}

#[tokio::test]
async fn test_media_resolve_and_file_stream_happy_path() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let media_root = test_root.join("allowed_media");
    fs::create_dir_all(&media_root)?;
    let media_file = media_root.join("demo.png");
    let media_bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A];
    fs::write(&media_file, &media_bytes)?;

    let previous_roots = std::env::var("XENOBOT_MEDIA_ROOTS").ok();
    std::env::set_var(
        "XENOBOT_MEDIA_ROOTS",
        media_root.to_string_lossy().to_string(),
    );

    let db_path = test_root.join("xenobot_media_api.db");
    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let app = build_router(&ApiConfig::default());

    let resolve_uri = format!("/media/resolve?path={}", media_file.to_string_lossy());
    let resolve_resp = get_response(&app, &resolve_uri).await?;
    assert_eq!(resolve_resp.status(), StatusCode::OK);
    let resolve_body = to_bytes(resolve_resp.into_body(), usize::MAX).await?;
    let resolve_json: serde_json::Value = serde_json::from_slice(&resolve_body)?;
    assert_eq!(resolve_json["ok"], true);
    assert_eq!(resolve_json["contentType"], "image/png");
    assert_eq!(resolve_json["size"], media_bytes.len() as u64);

    let stream_uri = format!(
        "/media/file?path={}&download=true",
        media_file.to_string_lossy()
    );
    let stream_resp = get_response(&app, &stream_uri).await?;
    assert_eq!(stream_resp.status(), StatusCode::OK);
    let stream_headers = stream_resp.headers().clone();
    assert_eq!(
        stream_headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("image/png")
    );
    assert!(stream_headers
        .get(axum::http::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("attachment")));
    let streamed = to_bytes(stream_resp.into_body(), usize::MAX).await?;
    assert_eq!(streamed.as_ref(), media_bytes.as_slice());

    if let Some(value) = previous_roots {
        std::env::set_var("XENOBOT_MEDIA_ROOTS", value);
    } else {
        std::env::remove_var("XENOBOT_MEDIA_ROOTS");
    }
    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_media_resolve_rejects_path_outside_allowed_roots(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let allowed_root = test_root.join("allowed");
    let blocked_root = test_root.join("blocked");
    fs::create_dir_all(&allowed_root)?;
    fs::create_dir_all(&blocked_root)?;
    let blocked_file = blocked_root.join("secret.txt");
    fs::write(&blocked_file, b"blocked")?;

    let previous_roots = std::env::var("XENOBOT_MEDIA_ROOTS").ok();
    std::env::set_var(
        "XENOBOT_MEDIA_ROOTS",
        allowed_root.to_string_lossy().to_string(),
    );

    let db_path = test_root.join("xenobot_media_api_reject.db");
    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let app = build_router(&ApiConfig::default());
    let resolve_uri = format!("/media/resolve?path={}", blocked_file.to_string_lossy());
    let resp = get_response(&app, &resolve_uri).await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = to_bytes(resp.into_body(), usize::MAX).await?;
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(json["code"], 401);
    assert!(json["error"]
        .as_str()
        .is_some_and(|text| text.contains("outside allowed roots")));

    if let Some(value) = previous_roots {
        std::env::set_var("XENOBOT_MEDIA_ROOTS", value);
    } else {
        std::env::remove_var("XENOBOT_MEDIA_ROOTS");
    }
    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_media_message_route_streams_file_from_message_content(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let media_root = test_root.join("allowed_media");
    fs::create_dir_all(&media_root)?;
    let audio_file = media_root.join("voice.m4a");
    let audio_bytes = vec![0x00, 0x01, 0x02, 0x03, 0x04];
    fs::write(&audio_file, &audio_bytes)?;

    let previous_roots = std::env::var("XENOBOT_MEDIA_ROOTS").ok();
    std::env::set_var(
        "XENOBOT_MEDIA_ROOTS",
        media_root.to_string_lossy().to_string(),
    );

    let db_path = test_root.join("xenobot_media_message.db");
    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());

    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "Media Session".to_string(),
            platform: "telegram".to_string(),
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
        .create_member(&Member {
            id: 0,
            platform_id: "media-user".to_string(),
            account_name: Some("Media User".to_string()),
            group_nickname: Some("Media User".to_string()),
            aliases: None,
            avatar: None,
            roles: None,
        })
        .await?;

    let message_id = repo
        .create_message(&Message {
            id: 0,
            sender_id,
            sender_account_name: Some("Media User".to_string()),
            sender_group_nickname: Some("Media User".to_string()),
            ts: 1_700_000_100,
            msg_type: 34,
            content: Some(
                serde_json::json!({
                    "file_path": audio_file.to_string_lossy().to_string()
                })
                .to_string(),
            ),
            reply_to_message_id: None,
            platform_message_id: Some("media-msg-1".to_string()),
            meta_id,
        })
        .await?;

    let app = build_router(&ApiConfig::default());
    let media_uri = format!("/media/messages/{}?download=true", message_id);
    let resp = get_response(&app, &media_uri).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let headers = resp.headers().clone();
    assert_eq!(
        headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("audio/mp4")
    );
    assert!(headers
        .get(axum::http::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| value.contains("attachment")));
    let body = to_bytes(resp.into_body(), usize::MAX).await?;
    assert_eq!(body.as_ref(), audio_bytes.as_slice());

    if let Some(value) = previous_roots {
        std::env::set_var("XENOBOT_MEDIA_ROOTS", value);
    } else {
        std::env::remove_var("XENOBOT_MEDIA_ROOTS");
    }
    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[cfg(not(feature = "wechat"))]
#[tokio::test]
async fn test_media_dat_decrypt_requires_wechat_feature() -> Result<(), Box<dyn std::error::Error>>
{
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;

    let db_path = test_root.join("xenobot_media_dat_not_impl.db");
    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let app = build_router(&ApiConfig::default());
    let resp = post_json_response(&app, "/media/decrypt/dat", serde_json::json!({})).await?;
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
    let body = to_bytes(resp.into_body(), usize::MAX).await?;
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(json["code"], 501);
    assert!(json["error"]
        .as_str()
        .is_some_and(|text| text.contains("requires api feature")));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[cfg(not(feature = "wechat"))]
#[tokio::test]
async fn test_media_audio_transcode_requires_wechat_feature(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;

    let db_path = test_root.join("xenobot_media_audio_not_impl.db");
    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let app = build_router(&ApiConfig::default());
    let resp =
        post_json_response(&app, "/media/transcode/audio/mp3", serde_json::json!({})).await?;
    assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
    let body = to_bytes(resp.into_body(), usize::MAX).await?;
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(json["code"], 501);
    assert!(json["error"]
        .as_str()
        .is_some_and(|text| text.contains("requires api feature")));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[cfg(feature = "wechat")]
#[tokio::test]
async fn test_media_dat_decrypt_validates_payload_when_wechat_enabled(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;

    let db_path = test_root.join("xenobot_media_dat_validate.db");
    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let app = build_router(&ApiConfig::default());
    let resp = post_json_response(&app, "/media/decrypt/dat", serde_json::json!({})).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(resp.into_body(), usize::MAX).await?;
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(json["code"], 400);
    assert!(json["error"]
        .as_str()
        .is_some_and(|text| text.contains("path or payloadBase64")));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

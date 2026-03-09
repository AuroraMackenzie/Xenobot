use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use tower::util::ServiceExt;
use xenobot_api::chat;
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
    std::env::temp_dir().join(format!(
        "xenobot_api_basic_analysis_{}_{}",
        epoch_nanos, seq
    ))
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

fn sum_counts(items: &[serde_json::Value], key: &str) -> i64 {
    items
        .iter()
        .map(|item| item[key].as_i64().unwrap_or(0))
        .sum::<i64>()
}

#[tokio::test]
async fn test_basic_analysis_endpoints_with_time_filter_contract(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_basic_analysis.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool);
    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "Basic Analysis Session".to_string(),
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

    let alice = repo.get_or_create_member("u_alice", Some("Alice")).await?;
    let bob = repo.get_or_create_member("u_bob", Some("Bob")).await?;
    let cara = repo.get_or_create_member("u_cara", Some("Cara")).await?;

    let rows = vec![
        (alice, "Alice", 1704100000_i64, 0_i64, "short text"),
        (bob, "Bob", 1704103600_i64, 1_i64, "photo message"),
        (
            alice,
            "Alice",
            1735720000_i64,
            0_i64,
            "longer message payload for distribution",
        ),
        (bob, "Bob", 1735723600_i64, 2_i64, "emoji payload"),
        (cara, "Cara", 1735727200_i64, 0_i64, "follow up"),
    ];

    for (sender_id, sender_name, ts, msg_type, content) in &rows {
        repo.create_message(&Message {
            id: 0,
            sender_id: *sender_id,
            sender_account_name: Some((*sender_name).to_string()),
            sender_group_nickname: Some((*sender_name).to_string()),
            ts: *ts,
            msg_type: *msg_type,
            content: Some((*content).to_string()),
            reply_to_message_id: None,
            platform_message_id: None,
            meta_id,
        })
        .await?;
    }

    let app = chat::router();

    let (status, years_resp) =
        get_json(&app, &format!("/sessions/{meta_id}/available-years")).await?;
    assert_eq!(status, StatusCode::OK);
    let years = years_resp
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_i64())
        .collect::<Vec<_>>();
    assert!(years.contains(&2024));
    assert!(years.contains(&2025));

    let (status, time_range_resp) =
        get_json(&app, &format!("/sessions/{meta_id}/time-range")).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        time_range_resp["earliest"].as_i64().unwrap_or_default(),
        1704100000
    );
    assert_eq!(
        time_range_resp["latest"].as_i64().unwrap_or_default(),
        1735727200
    );

    let (status, member_resp) =
        get_json(&app, &format!("/sessions/{meta_id}/member-activity")).await?;
    assert_eq!(status, StatusCode::OK);
    let member_items = member_resp.as_array().cloned().unwrap_or_default();
    assert!(!member_items.is_empty());
    assert_eq!(sum_counts(&member_items, "message_count"), 5);

    let filtered_start = 1735720000_i64;
    let filtered_end = 1735727200_i64;
    let (status, member_filtered) = get_json(
        &app,
        &format!(
            "/sessions/{meta_id}/member-activity?start_ts={filtered_start}&end_ts={filtered_end}"
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let member_filtered_items = member_filtered.as_array().cloned().unwrap_or_default();
    assert_eq!(sum_counts(&member_filtered_items, "message_count"), 3);

    for endpoint in [
        "hourly-activity",
        "daily-activity",
        "weekday-activity",
        "monthly-activity",
        "yearly-activity",
    ] {
        let (status, body) = get_json(&app, &format!("/sessions/{meta_id}/{endpoint}")).await?;
        assert_eq!(status, StatusCode::OK, "endpoint {endpoint} should succeed");
        let items = body.as_array().cloned().unwrap_or_default();
        assert!(!items.is_empty(), "endpoint {endpoint} should not be empty");
        assert_eq!(
            sum_counts(&items, "message_count"),
            5,
            "endpoint {endpoint} should aggregate all rows"
        );
    }

    let (status, yearly_filtered) = get_json(
        &app,
        &format!(
            "/sessions/{meta_id}/yearly-activity?start_ts={filtered_start}&end_ts={filtered_end}"
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let yearly_filtered_items = yearly_filtered.as_array().cloned().unwrap_or_default();
    assert_eq!(sum_counts(&yearly_filtered_items, "message_count"), 3);
    assert_eq!(yearly_filtered_items.len(), 1);

    let (status, type_resp) = get_json(
        &app,
        &format!("/sessions/{meta_id}/message-type-distribution"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let type_items = type_resp.as_array().cloned().unwrap_or_default();
    assert!(!type_items.is_empty());
    assert_eq!(sum_counts(&type_items, "count"), 5);
    assert!(type_items
        .iter()
        .any(|row| row["msg_type"].as_i64() == Some(0) && row["count"].as_i64() == Some(3)));

    let (status, length_resp) = get_json(
        &app,
        &format!("/sessions/{meta_id}/message-length-distribution"),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let detail = length_resp["detail"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let grouped = length_resp["grouped"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(!detail.is_empty());
    assert!(!grouped.is_empty());
    let text_message_count = 3_i64;
    assert!(sum_counts(&detail, "count") > 0);
    assert!(sum_counts(&detail, "count") <= text_message_count);
    assert_eq!(sum_counts(&grouped, "count"), text_message_count);

    let (status, length_filtered) = get_json(
        &app,
        &format!(
            "/sessions/{meta_id}/message-length-distribution?start_ts={filtered_start}&end_ts={filtered_end}"
        ),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let filtered_grouped = length_filtered["grouped"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(sum_counts(&filtered_grouped, "count"), 2);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

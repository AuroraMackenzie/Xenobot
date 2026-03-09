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
        "xenobot_api_advanced_analysis_{}_{}",
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

#[tokio::test]
async fn test_advanced_analysis_endpoints_return_expected_shapes(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_advanced_analysis.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "Advanced Analysis Session".to_string(),
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
        (alice, "Alice", 1_800_000_001, "Echo phrase"),
        (bob, "Bob", 1_800_000_002, "Echo phrase"),
        (alice, "Alice", 1_800_000_120, "Alpha message"),
        (alice, "Alice", 1_800_000_240, "@Bob please check this"),
        (bob, "Bob", 1_800_000_250, "@Alice received lol"),
        (cara, "Cara", 1_800_000_260, "@Alice got it 哈哈"),
        (alice, "Alice", 1_800_086_400, "Daily check-in"),
        (alice, "Alice", 1_800_172_800, "Daily check-in"),
        (bob, "Bob", 1_800_172_810, "Bob follow up"),
        (cara, "Cara", 1_800_172_820, "Silent one"),
        (alice, "Alice", 1_800_172_900, "Echo phrase"),
        (bob, "Bob", 1_800_172_901, "Echo phrase"),
    ];

    for (sender_id, sender_name, ts, content) in rows {
        repo.create_message(&Message {
            id: 0,
            sender_id,
            sender_account_name: Some(sender_name.to_string()),
            sender_group_nickname: Some(sender_name.to_string()),
            ts,
            msg_type: 0,
            content: Some(content.to_string()),
            reply_to_message_id: None,
            platform_message_id: None,
            meta_id,
        })
        .await?;
    }

    let app = chat::router();

    let (status, night_resp) =
        get_json(&app, &format!("/sessions/{meta_id}/night-owl-analysis")).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(night_resp["members"].is_array());
    assert_eq!(
        night_resp["stats"]["totalMessages"].as_i64().unwrap_or(0),
        12
    );

    let (status, dragon_resp) =
        get_json(&app, &format!("/sessions/{meta_id}/dragon-king-analysis")).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dragon_resp["dragonKing"]["name"], "Alice");
    let dragon_board = dragon_resp["leaderboard"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(dragon_board.len() >= 3);

    let (status, lurker_resp) =
        get_json(&app, &format!("/sessions/{meta_id}/lurker-analysis")).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(lurker_resp["lurkers"].is_array());
    assert_eq!(lurker_resp["stats"]["memberCount"].as_i64().unwrap_or(0), 3);

    let (status, checkin_resp) =
        get_json(&app, &format!("/sessions/{meta_id}/checkin-analysis")).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(checkin_resp["leaderboard"].is_array());
    assert!(checkin_resp["weekdayDistribution"].is_array());

    let (status, repeat_resp) =
        get_json(&app, &format!("/sessions/{meta_id}/repeat-analysis")).await?;
    assert_eq!(status, StatusCode::OK);
    let phrases = repeat_resp["phrases"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(phrases.iter().any(|p| p["phrase"] == "echo phrase"));
    let runs = repeat_resp["runs"].as_array().cloned().unwrap_or_default();
    assert!(!runs.is_empty());

    let (status, catchphrase_resp) =
        get_json(&app, &format!("/sessions/{meta_id}/catchphrase-analysis")).await?;
    assert_eq!(status, StatusCode::OK);
    let members = catchphrase_resp["members"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(!members.is_empty());
    assert!(members.iter().any(|member| {
        member["catchphrases"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["content"] == "Echo phrase"))
    }));

    let (status, mention_resp) =
        get_json(&app, &format!("/sessions/{meta_id}/mention-analysis")).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(mention_resp["topMentioners"].is_array() || mention_resp["top_mentioners"].is_array());
    assert!(mention_resp["topMentioned"].is_array() || mention_resp["top_mentioned"].is_array());
    let total_mentions = mention_resp["totalMentions"]
        .as_i64()
        .or_else(|| mention_resp["total_mentions"].as_i64())
        .unwrap_or(0);
    assert!(total_mentions >= 3);

    let (status, cluster_resp) =
        get_json(&app, &format!("/sessions/{meta_id}/cluster-graph")).await?;
    assert_eq!(status, StatusCode::OK);
    let cluster_nodes = cluster_resp["nodes"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(!cluster_nodes.is_empty());
    assert!(cluster_nodes
        .iter()
        .all(|node| node.get("communityId").is_some()));
    assert_eq!(
        cluster_resp["stats"]["algorithm"].as_str().unwrap_or(""),
        "weighted_label_propagation"
    );

    let (status, laugh_resp) =
        get_json(&app, &format!("/sessions/{meta_id}/laugh-analysis")).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(laugh_resp["rankByCount"].is_array());
    assert!(laugh_resp["typeDistribution"].is_array());
    assert!(laugh_resp["totalLaughs"].as_i64().unwrap_or(0) >= 2);

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_advanced_analysis_endpoints_return_not_found_for_missing_session(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_advanced_analysis_missing_session.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let app = chat::router();
    let missing_session_id = 9_999_999_i64;
    let endpoints = [
        "night-owl-analysis",
        "dragon-king-analysis",
        "lurker-analysis",
        "checkin-analysis",
        "repeat-analysis",
        "catchphrase-analysis",
        "mention-analysis",
        "mention-graph",
        "cluster-graph",
        "laugh-analysis",
    ];

    for endpoint in endpoints {
        let (status, body) =
            get_json(&app, &format!("/sessions/{missing_session_id}/{endpoint}")).await?;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "expected 404 for endpoint {endpoint}"
        );
        assert_eq!(body["code"], 404);
        assert!(
            body["error"]
                .as_str()
                .is_some_and(|text| text.contains("session") && text.contains("not found")),
            "unexpected error body for endpoint {endpoint}: {body}"
        );
    }

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

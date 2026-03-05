use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use tower::util::ServiceExt;
use xenobot_api::agent;
use xenobot_api::database::repository::{
    ChatMeta, ChatSession, MemberNameHistory, Message, MessageContext,
};
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
    std::env::temp_dir().join(format!("xenobot_api_agent_tools_{}_{}", epoch_nanos, seq))
}

fn parse_sse_chunks(body: &str) -> Vec<serde_json::Value> {
    body.lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect()
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
async fn test_agent_tools_catalog_contract_and_session_error_semantics(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let app = agent::router();

    let (status, tools_json) = get_json(&app, "/tools").await?;
    assert_eq!(status, StatusCode::OK);
    let tools = tools_json
        .as_array()
        .expect("agent tools endpoint should return array");
    assert_eq!(tools.len(), 12);

    let names = tools
        .iter()
        .filter_map(|item| item.get("name"))
        .filter_map(|v| v.as_str())
        .collect::<std::collections::HashSet<_>>();
    let required = [
        "search_messages",
        "get_recent_messages",
        "member_stats",
        "time_stats",
        "member_list",
        "nickname_history",
        "conversation_between",
        "message_context",
        "search_sessions",
        "get_session_messages",
        "get_session_summary",
        "semantic_search",
    ];
    for key in required {
        assert!(names.contains(key), "missing tool definition: {key}");
    }

    for item in tools {
        let tool_name = item
            .get("name")
            .and_then(|v| v.as_str())
            .expect("tool name should be a string");
        let aliases = item
            .get("aliases")
            .and_then(|v| v.as_array())
            .expect("tool aliases should be array");
        assert!(
            !aliases.is_empty(),
            "tool aliases must not be empty for {tool_name}"
        );
        assert!(
            aliases.iter().any(|v| v.as_str() == Some(tool_name)),
            "tool aliases must include canonical name for {tool_name}"
        );

        let params = item
            .get("parameters")
            .expect("tool definition should contain parameters");
        assert!(
            params.get("required").is_some_and(|v| v.is_array()),
            "tool parameters.required must be array"
        );
        assert!(
            params.get("optional").is_some_and(|v| v.is_array()),
            "tool parameters.optional must be array"
        );
        assert!(
            params
                .get("inferredFromPrompt")
                .is_some_and(|v| v.is_array()),
            "tool parameters.inferredFromPrompt must be array"
        );
    }

    let member_stats_aliases = tools
        .iter()
        .find(|item| item.get("name").and_then(|v| v.as_str()) == Some("member_stats"))
        .and_then(|item| item.get("aliases"))
        .and_then(|v| v.as_array())
        .expect("member_stats aliases should exist");
    assert!(member_stats_aliases
        .iter()
        .any(|v| v.as_str() == Some("get_member_stats")));
    assert!(member_stats_aliases
        .iter()
        .any(|v| v.as_str() == Some("getMemberStats")));

    let session_summary_aliases = tools
        .iter()
        .find(|item| item.get("name").and_then(|v| v.as_str()) == Some("get_session_summary"))
        .and_then(|item| item.get("aliases"))
        .and_then(|v| v.as_array())
        .expect("get_session_summary aliases should exist");
    assert!(session_summary_aliases
        .iter()
        .any(|v| v.as_str() == Some("get_session_summaries")));
    assert!(session_summary_aliases
        .iter()
        .any(|v| v.as_str() == Some("getSessionSummaries")));

    let semantic_aliases = tools
        .iter()
        .find(|item| item.get("name").and_then(|v| v.as_str()) == Some("semantic_search"))
        .and_then(|item| item.get("aliases"))
        .and_then(|v| v.as_array())
        .expect("semantic_search aliases should exist");
    assert!(semantic_aliases
        .iter()
        .any(|v| v.as_str() == Some("semantic_search_messages")));
    assert!(semantic_aliases
        .iter()
        .any(|v| v.as_str() == Some("semanticSearchMessages")));

    let request = Request::builder()
        .method("POST")
        .uri("/run-stream")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&serde_json::json!({
            "requestId": "invalid_session_test",
            "userMessage": "test invalid session",
            "context": {
                "sessionId": "not_a_number"
            },
            "maxRounds": 1
        }))?))?;
    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let body_text = String::from_utf8(body_bytes.to_vec())?;
    let chunks = parse_sse_chunks(&body_text);
    assert!(chunks.iter().any(|chunk| {
        chunk.get("type").and_then(|v| v.as_str()) == Some("error")
            && chunk
                .get("error")
                .and_then(|v| v.as_str())
                .is_some_and(|msg| msg.contains("invalid session_id"))
    }));
    assert!(chunks.iter().any(|chunk| {
        chunk.get("type").and_then(|v| v.as_str()) == Some("error")
            && chunk.get("error_code").and_then(|v| v.as_str()) == Some("error.invalid_session_id")
    }));
    assert!(chunks.iter().any(|chunk| {
        chunk.get("type").and_then(|v| v.as_str()) == Some("error")
            && chunk.get("is_finished").and_then(|v| v.as_bool()) == Some(true)
    }));

    Ok(())
}

#[tokio::test]
async fn test_agent_run_stream_executes_real_tools() -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_agent_tools.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "Agent Tool Session".to_string(),
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
        .get_or_create_member("u_alice_agent", Some("Alice"))
        .await?;
    let bob = repo
        .get_or_create_member("u_bob_agent", Some("Bob"))
        .await?;

    let msg1 = repo
        .create_message(&Message {
            id: 0,
            sender_id: alice,
            sender_account_name: Some("Alice".to_string()),
            sender_group_nickname: Some("Alice".to_string()),
            ts: 1_800_000_000,
            msg_type: 0,
            content: Some("Data migration checkpoint completed".to_string()),
            reply_to_message_id: None,
            platform_message_id: None,
            meta_id,
        })
        .await?;
    let msg2 = repo
        .create_message(&Message {
            id: 0,
            sender_id: bob,
            sender_account_name: Some("Bob".to_string()),
            sender_group_nickname: Some("Bob".to_string()),
            ts: 1_800_000_120,
            msg_type: 0,
            content: Some("Incremental import semantic query passed".to_string()),
            reply_to_message_id: None,
            platform_message_id: None,
            meta_id,
        })
        .await?;

    let chat_session_id = repo
        .create_chat_session(&ChatSession {
            id: 0,
            meta_id,
            start_ts: 1_800_000_000,
            end_ts: 1_800_000_200,
            message_count: Some(2),
            is_manual: Some(false),
            summary: Some("short summary".to_string()),
        })
        .await?;
    repo.create_message_context(&MessageContext {
        message_id: msg1,
        session_id: chat_session_id,
        topic_id: None,
    })
    .await?;
    repo.create_message_context(&MessageContext {
        message_id: msg2,
        session_id: chat_session_id,
        topic_id: None,
    })
    .await?;

    let _ = repo
        .create_member_name_history(&MemberNameHistory {
            id: 0,
            member_id: alice,
            name_type: "group_nickname".to_string(),
            name: "Alice".to_string(),
            start_ts: 1_799_999_000,
            end_ts: None,
        })
        .await?;

    let app = agent::router();
    let request = Request::builder()
        .method("POST")
        .uri("/run-stream")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&serde_json::json!({
            "requestId": "agent_test_req",
            "userMessage": "请做成员统计、会话摘要和语义搜索",
            "context": {
                "sessionId": meta_id.to_string(),
                "maxMessagesLimit": 20
            },
            "maxRounds": 5
        }))?))?;
    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let body_text = String::from_utf8(body_bytes.to_vec())?;
    let chunks = parse_sse_chunks(&body_text);
    assert!(!chunks.is_empty());

    let tool_starts = chunks
        .iter()
        .filter(|chunk| chunk.get("type").and_then(|v| v.as_str()) == Some("tool_start"))
        .collect::<Vec<_>>();
    assert!(!tool_starts.is_empty());
    let tool_names = tool_starts
        .iter()
        .filter_map(|chunk| chunk.get("tool_name").and_then(|v| v.as_str()))
        .collect::<Vec<_>>();
    assert!(tool_names.iter().any(|name| *name == "member_stats"));
    assert!(tool_names.iter().any(|name| *name == "semantic_search"));
    assert!(tool_names.iter().any(|name| *name == "get_session_summary"));

    assert!(chunks
        .iter()
        .any(|chunk| { chunk.get("type").and_then(|v| v.as_str()) == Some("tool_result") }));
    assert!(chunks
        .iter()
        .any(|chunk| { chunk.get("type").and_then(|v| v.as_str()) == Some("done") }));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_agent_run_stream_accepts_snake_case_and_forced_tools(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_agent_tools_snake_case.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "Agent Snake Session".to_string(),
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
    let member = repo
        .get_or_create_member("u_snake_agent", Some("Snake"))
        .await?;
    repo.create_message(&Message {
        id: 0,
        sender_id: member,
        sender_account_name: Some("Snake".to_string()),
        sender_group_nickname: Some("Snake".to_string()),
        ts: 1_800_100_000,
        msg_type: 0,
        content: Some("semantic checkpoint signal".to_string()),
        reply_to_message_id: None,
        platform_message_id: None,
        meta_id,
    })
    .await?;

    let app = agent::router();
    let request = Request::builder()
        .method("POST")
        .uri("/run-stream")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&serde_json::json!({
            "request_id": "snake_case_req",
            "user_message": "请做语义检索",
            "context": {
                "session_id": meta_id.to_string(),
                "max_messages_limit": 10
            },
            "forced_tools": ["semantic_search", "totally_unknown_tool"],
            "max_rounds": 1,
            "include_tool_results": true
        }))?))?;
    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let body_text = String::from_utf8(body_bytes.to_vec())?;
    let chunks = parse_sse_chunks(&body_text);
    assert!(!chunks.is_empty());

    let tool_starts = chunks
        .iter()
        .filter(|chunk| chunk.get("type").and_then(|v| v.as_str()) == Some("tool_start"))
        .collect::<Vec<_>>();
    assert_eq!(
        tool_starts.len(),
        1,
        "only one known forced tool should run"
    );
    let tool_start = tool_starts[0];
    assert_eq!(tool_start["tool_name"], "semantic_search");
    assert_eq!(tool_start["tool_params"]["query"], "请做语义检索");
    assert_eq!(tool_start["tool_params"]["limit"], 10);

    assert!(chunks.iter().any(|chunk| {
        chunk.get("type").and_then(|v| v.as_str()) == Some("tool_result")
            && chunk.get("tool_name").and_then(|v| v.as_str()) == Some("semantic_search")
    }));
    assert!(chunks
        .iter()
        .any(|chunk| { chunk.get("type").and_then(|v| v.as_str()) == Some("done") }));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_agent_run_stream_resolves_owner_platform_id_for_member_tools(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_agent_tools_owner_platform.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "Agent Owner Platform Session".to_string(),
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

    let owner_member = repo
        .get_or_create_member("u_owner_platform", Some("Owner"))
        .await?;
    repo.create_message(&Message {
        id: 0,
        sender_id: owner_member,
        sender_account_name: Some("Owner".to_string()),
        sender_group_nickname: Some("Owner".to_string()),
        ts: 1_800_500_000,
        msg_type: 0,
        content: Some("owner context seed".to_string()),
        reply_to_message_id: None,
        platform_message_id: None,
        meta_id,
    })
    .await?;
    let _ = repo
        .create_member_name_history(&MemberNameHistory {
            id: 0,
            member_id: owner_member,
            name_type: "group_nickname".to_string(),
            name: "Owner".to_string(),
            start_ts: 1_800_400_000,
            end_ts: None,
        })
        .await?;

    let app = agent::router();
    let request = Request::builder()
        .method("POST")
        .uri("/run-stream")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&serde_json::json!({
            "requestId": "owner_platform_req",
            "userMessage": "查看昵称变化历史",
            "context": {
                "sessionId": meta_id.to_string(),
                "ownerInfo": {
                    "platformId": "u_owner_platform",
                    "displayName": "Owner"
                }
            },
            "forcedTools": ["nickname_history"],
            "maxRounds": 1,
            "includeToolResults": true
        }))?))?;
    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let body_text = String::from_utf8(body_bytes.to_vec())?;
    let chunks = parse_sse_chunks(&body_text);
    assert!(!chunks.is_empty());

    let tool_start = chunks
        .iter()
        .find(|chunk| chunk.get("type").and_then(|v| v.as_str()) == Some("tool_start"))
        .expect("tool_start chunk should exist");
    assert_eq!(tool_start["tool_name"], "nickname_history");
    assert_eq!(
        tool_start["tool_params"]["memberId"].as_i64(),
        Some(owner_member),
        "nickname_history should infer memberId from owner platformId"
    );

    let tool_result = chunks
        .iter()
        .find(|chunk| chunk.get("type").and_then(|v| v.as_str()) == Some("tool_result"))
        .expect("tool_result chunk should exist");
    assert_eq!(tool_result["tool_name"], "nickname_history");
    assert_eq!(
        tool_result["tool_result"]["memberId"].as_i64(),
        Some(owner_member)
    );
    assert!(
        tool_result["tool_result"]["historyCount"]
            .as_i64()
            .unwrap_or(0)
            >= 1,
        "owner member should have nickname history rows"
    );
    assert!(
        !chunks
            .iter()
            .any(|chunk| chunk.get("type").and_then(|v| v.as_str()) == Some("error")),
        "owner platform id resolution should avoid error chunk"
    );

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_agent_run_stream_normalizes_forced_tool_aliases_to_canonical_names(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_agent_tools_forced_alias.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "Agent Forced Alias Session".to_string(),
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

    let member = repo
        .get_or_create_member("u_alias_agent", Some("Alias"))
        .await?;
    let first_msg_id = repo
        .create_message(&Message {
            id: 0,
            sender_id: member,
            sender_account_name: Some("Alias".to_string()),
            sender_group_nickname: Some("Alias".to_string()),
            ts: 1_800_200_000,
            msg_type: 0,
            content: Some("semantic alias test message".to_string()),
            reply_to_message_id: None,
            platform_message_id: None,
            meta_id,
        })
        .await?;

    let chat_session_id = repo
        .create_chat_session(&ChatSession {
            id: 0,
            meta_id,
            start_ts: 1_800_200_000,
            end_ts: 1_800_200_060,
            message_count: Some(1),
            is_manual: Some(false),
            summary: Some("forced alias summary".to_string()),
        })
        .await?;
    repo.create_message_context(&MessageContext {
        message_id: first_msg_id,
        session_id: chat_session_id,
        topic_id: None,
    })
    .await?;

    let app = agent::router();
    let request = Request::builder()
        .method("POST")
        .uri("/run-stream")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&serde_json::json!({
            "requestId": "forced_alias_req",
            "userMessage": "请执行强制工具别名映射测试",
            "context": {
                "sessionId": meta_id.to_string(),
                "maxMessagesLimit": 20
            },
            "forcedTools": [
                "semanticSearchMessages",
                "getSessionSummaries",
                "memberStats",
                "semantic_search",
                "unknown_alias_tool"
            ],
            "maxRounds": 5
        }))?))?;
    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
    let body_text = String::from_utf8(body_bytes.to_vec())?;
    let chunks = parse_sse_chunks(&body_text);
    assert!(!chunks.is_empty());

    let tool_starts = chunks
        .iter()
        .filter(|chunk| chunk.get("type").and_then(|v| v.as_str()) == Some("tool_start"))
        .collect::<Vec<_>>();
    let tool_names = tool_starts
        .iter()
        .filter_map(|chunk| chunk.get("tool_name").and_then(|v| v.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        tool_names,
        vec!["semantic_search", "get_session_summary", "member_stats"]
    );
    assert!(!tool_names
        .iter()
        .any(|name| name.contains("semanticSearch")));
    assert!(!tool_names.iter().any(|name| name.contains("getSession")));

    assert!(chunks
        .iter()
        .any(|chunk| { chunk.get("type").and_then(|v| v.as_str()) == Some("done") }));

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_agent_forced_single_tool_parameter_contract_matrix(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_agent_tools_param_matrix.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "Agent Parameter Matrix Session".to_string(),
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
        .get_or_create_member("u_matrix_alice", Some("Alice"))
        .await?;
    let bob = repo
        .get_or_create_member("u_matrix_bob", Some("Bob"))
        .await?;
    let msg_id = repo
        .create_message(&Message {
            id: 0,
            sender_id: alice,
            sender_account_name: Some("Alice".to_string()),
            sender_group_nickname: Some("Alice".to_string()),
            ts: 1_800_300_000,
            msg_type: 0,
            content: Some("matrix semantic content".to_string()),
            reply_to_message_id: None,
            platform_message_id: None,
            meta_id,
        })
        .await?;
    repo.create_message(&Message {
        id: 0,
        sender_id: bob,
        sender_account_name: Some("Bob".to_string()),
        sender_group_nickname: Some("Bob".to_string()),
        ts: 1_800_300_020,
        msg_type: 0,
        content: Some("matrix second line".to_string()),
        reply_to_message_id: None,
        platform_message_id: None,
        meta_id,
    })
    .await?;

    let chat_session_id = repo
        .create_chat_session(&ChatSession {
            id: 0,
            meta_id,
            start_ts: 1_800_300_000,
            end_ts: 1_800_300_050,
            message_count: Some(2),
            is_manual: Some(false),
            summary: Some("matrix summary".to_string()),
        })
        .await?;
    repo.create_message_context(&MessageContext {
        message_id: msg_id,
        session_id: chat_session_id,
        topic_id: None,
    })
    .await?;
    let _ = repo
        .create_member_name_history(&MemberNameHistory {
            id: 0,
            member_id: alice,
            name_type: "group_nickname".to_string(),
            name: "Alice".to_string(),
            start_ts: 1_800_299_000,
            end_ts: None,
        })
        .await?;

    let cases: [(&str, &[&str]); 12] = [
        ("search_messages", &["query", "limit"]),
        ("get_recent_messages", &["limit"]),
        ("member_stats", &["topN"]),
        ("time_stats", &["dimension"]),
        ("member_list", &["limit"]),
        ("nickname_history", &["memberId"]),
        ("conversation_between", &["memberId1", "memberId2"]),
        ("message_context", &["messageId", "contextSize"]),
        ("search_sessions", &["limit"]),
        ("get_session_messages", &["chatSessionId"]),
        ("get_session_summary", &["chatSessionId"]),
        ("semantic_search", &["query", "limit", "threshold"]),
    ];

    let app = agent::router();
    for (tool_name, required_keys) in cases {
        let request = Request::builder()
            .method("POST")
            .uri("/run-stream")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&serde_json::json!({
                "requestId": format!("matrix_{}", tool_name),
                "userMessage": format!(
                    "执行 {} 并参考 {} {} {} {}",
                    tool_name, alice, bob, msg_id, chat_session_id
                ),
                "context": {
                    "sessionId": meta_id.to_string(),
                    "maxMessagesLimit": 20
                },
                "forcedTools": [tool_name],
                "maxRounds": 1,
                "includeToolResults": true
            }))?))?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::OK, "tool={tool_name}");
        let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body_text = String::from_utf8(body_bytes.to_vec())?;
        let chunks = parse_sse_chunks(&body_text);
        assert!(!chunks.is_empty(), "tool={tool_name}");

        let tool_start = chunks
            .iter()
            .find(|chunk| chunk.get("type").and_then(|v| v.as_str()) == Some("tool_start"))
            .unwrap_or_else(|| panic!("tool_start chunk should exist for {}", tool_name));
        assert_eq!(
            tool_start.get("tool_name").and_then(|v| v.as_str()),
            Some(tool_name),
            "tool={tool_name}"
        );
        let params = tool_start
            .get("tool_params")
            .and_then(|v| v.as_object())
            .unwrap_or_else(|| panic!("tool_params should be object for {}", tool_name));
        for key in required_keys {
            assert!(
                params.contains_key(*key),
                "tool={} missing expected tool_params key '{}'",
                tool_name,
                key
            );
        }
        assert!(
            chunks.iter().any(|chunk| {
                chunk.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                    && chunk.get("tool_name").and_then(|v| v.as_str()) == Some(tool_name)
            }),
            "tool_result chunk should exist for {}",
            tool_name
        );
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.get("type").and_then(|v| v.as_str()) == Some("done")),
            "done chunk should exist for {}",
            tool_name
        );
        assert!(
            !chunks
                .iter()
                .any(|chunk| chunk.get("type").and_then(|v| v.as_str()) == Some("error")),
            "error chunk should not appear for {}",
            tool_name
        );
    }

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

#[tokio::test]
async fn test_agent_run_stream_normalizes_calling_layer_alias_matrix(
) -> Result<(), Box<dyn std::error::Error>> {
    let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()?;
    let _cwd_guard = WorkingDirGuard::change_to(&workspace_root)?;

    let test_root = unique_test_root();
    fs::create_dir_all(&test_root)?;
    let db_path = test_root.join("xenobot_api_agent_tools_alias_matrix.db");

    let mut db_config = DatabaseConfig::default();
    db_config.sqlite_path = db_path;
    xenobot_api::database::init_database_with_config(&db_config).await?;

    let pool = xenobot_api::database::get_pool().await?;
    let repo = Repository::new(pool.clone());
    let meta_id = repo
        .create_chat(&ChatMeta {
            id: 0,
            name: "Agent Alias Matrix Session".to_string(),
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
        .get_or_create_member("u_alias_matrix_alice", Some("Alice"))
        .await?;
    let bob = repo
        .get_or_create_member("u_alias_matrix_bob", Some("Bob"))
        .await?;
    let msg_id = repo
        .create_message(&Message {
            id: 0,
            sender_id: alice,
            sender_account_name: Some("Alice".to_string()),
            sender_group_nickname: Some("Alice".to_string()),
            ts: 1_800_400_000,
            msg_type: 0,
            content: Some("alias matrix seed content".to_string()),
            reply_to_message_id: None,
            platform_message_id: None,
            meta_id,
        })
        .await?;
    repo.create_message(&Message {
        id: 0,
        sender_id: bob,
        sender_account_name: Some("Bob".to_string()),
        sender_group_nickname: Some("Bob".to_string()),
        ts: 1_800_400_020,
        msg_type: 0,
        content: Some("alias matrix secondary content".to_string()),
        reply_to_message_id: None,
        platform_message_id: None,
        meta_id,
    })
    .await?;

    let chat_session_id = repo
        .create_chat_session(&ChatSession {
            id: 0,
            meta_id,
            start_ts: 1_800_400_000,
            end_ts: 1_800_400_060,
            message_count: Some(2),
            is_manual: Some(false),
            summary: Some("alias matrix summary".to_string()),
        })
        .await?;
    repo.create_message_context(&MessageContext {
        message_id: msg_id,
        session_id: chat_session_id,
        topic_id: None,
    })
    .await?;
    let _ = repo
        .create_member_name_history(&MemberNameHistory {
            id: 0,
            member_id: alice,
            name_type: "group_nickname".to_string(),
            name: "Alice".to_string(),
            start_ts: 1_800_399_000,
            end_ts: None,
        })
        .await?;

    let aliases: [(&str, &str); 13] = [
        ("get_member_stats", "member_stats"),
        ("get_time_stats", "time_stats"),
        ("get_group_members", "member_list"),
        ("get_member_name_history", "nickname_history"),
        ("get_conversation_between", "conversation_between"),
        ("get_message_context", "message_context"),
        ("get_session_summaries", "get_session_summary"),
        ("semantic_search_messages", "semantic_search"),
        ("getMemberStats", "member_stats"),
        ("getTimeStats", "time_stats"),
        ("getGroupMembers", "member_list"),
        ("getSessionSummaries", "get_session_summary"),
        ("semanticSearchMessages", "semantic_search"),
    ];

    let app = agent::router();
    for (alias_name, canonical_name) in aliases {
        let request = Request::builder()
            .method("POST")
            .uri("/run-stream")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&serde_json::json!({
                "requestId": format!("alias_matrix_{}", alias_name),
                "userMessage": format!(
                    "执行 {} 并参考 {} {} {} {}",
                    alias_name, alice, bob, msg_id, chat_session_id
                ),
                "context": {
                    "sessionId": meta_id.to_string(),
                    "maxMessagesLimit": 20
                },
                "forcedTools": [alias_name],
                "maxRounds": 1,
                "includeToolResults": true
            }))?))?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::OK, "alias={alias_name}");
        let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body_text = String::from_utf8(body_bytes.to_vec())?;
        let chunks = parse_sse_chunks(&body_text);
        assert!(!chunks.is_empty(), "alias={alias_name}");

        let tool_start = chunks
            .iter()
            .find(|chunk| chunk.get("type").and_then(|v| v.as_str()) == Some("tool_start"))
            .unwrap_or_else(|| panic!("tool_start chunk should exist for {}", alias_name));
        assert_eq!(
            tool_start.get("tool_name").and_then(|v| v.as_str()),
            Some(canonical_name),
            "alias '{}' should normalize to '{}'",
            alias_name,
            canonical_name
        );
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.get("type").and_then(|v| v.as_str()) == Some("tool_result")),
            "tool_result chunk should exist for alias {}",
            alias_name
        );
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.get("type").and_then(|v| v.as_str()) == Some("done")),
            "done chunk should exist for alias {}",
            alias_name
        );
        assert!(
            !chunks
                .iter()
                .any(|chunk| chunk.get("type").and_then(|v| v.as_str()) == Some("error")),
            "error chunk should not appear for alias {}",
            alias_name
        );
    }

    let _ = fs::remove_dir_all(&test_root);
    Ok(())
}

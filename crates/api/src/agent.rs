//! Agent API module for Xenobot HTTP API.
//!
//! Provides HTTP endpoints equivalent to Xenobot's `agentApi` IPC methods.

use axum::{
    extract::Path,
    response::{sse::Event, Sse},
    routing::post,
    Json, Router,
};
use futures::stream;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::convert::Infallible;
use tokio::sync::RwLock;
use tracing::instrument;

use crate::ApiError;

static ABORTED_REQUESTS: Lazy<RwLock<HashSet<String>>> = Lazy::new(|| RwLock::new(HashSet::new()));

/// Agent API router.
pub fn router() -> Router {
    Router::new()
        .route("/run-stream", post(run_stream))
        .route("/abort/:request_id", post(abort))
}

// ==================== Request/Response Types ====================

#[derive(Debug, Serialize, Deserialize)]
struct TimeFilter {
    start_ts: Option<i64>,
    end_ts: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OwnerInfo {
    id: i32,
    name: String,
    avatar_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolContext {
    session_id: String,
    time_filter: Option<TimeFilter>,
    max_messages_limit: Option<i32>,
    owner_info: Option<OwnerInfo>,
    locale: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AgentStreamChunk {
    #[serde(rename = "type")]
    chunk_type: String, // "content", "think", "tool_start", "tool_result", "done", "error"
    content: Option<String>,
    think_tag: Option<String>,
    think_duration_ms: Option<u64>,
    tool_name: Option<String>,
    tool_params: Option<serde_json::Value>,
    tool_result: Option<serde_json::Value>,
    error: Option<String>,
    is_finished: Option<bool>,
    usage: Option<TokenUsage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct AgentResult {
    content: String,
    tools_used: Vec<String>,
    tool_rounds: u32,
    total_usage: Option<TokenUsage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PromptConfig {
    role_definition: String,
    response_rules: String,
}

// ==================== Handler Implementations ====================

#[derive(Debug, Deserialize)]
struct RunStreamRequest {
    user_message: String,
    context: ToolContext,
    history_messages: Option<Vec<serde_json::Value>>,
    chat_type: Option<String>, // "group" | "private"
    prompt_config: Option<PromptConfig>,
    locale: Option<String>,
}

#[instrument]
async fn run_stream(
    Json(req): Json<RunStreamRequest>,
) -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let locale = req.locale.unwrap_or_else(|| "zh-CN".to_string());
    let prompt = req.user_message.trim().to_string();
    let content = if prompt.is_empty() {
        if locale.starts_with("zh") {
            "请提供一个问题，我才能开始分析。".to_string()
        } else {
            "Please provide a question so I can analyze it.".to_string()
        }
    } else if locale.starts_with("zh") {
        format!("已收到你的问题：{}。当前为 HTTP 简化 Agent 流程。", prompt)
    } else {
        format!(
            "Received your question: {}. Running simplified HTTP agent flow.",
            prompt
        )
    };

    let think_chunk = AgentStreamChunk {
        chunk_type: "think".to_string(),
        content: Some(if locale.starts_with("zh") {
            "正在分析上下文并准备工具调用…".to_string()
        } else {
            "Analyzing context and preparing tool calls...".to_string()
        }),
        think_tag: Some("analysis".to_string()),
        think_duration_ms: Some(80),
        tool_name: None,
        tool_params: None,
        tool_result: None,
        error: None,
        is_finished: Some(false),
        usage: None,
    };
    let content_chunk = AgentStreamChunk {
        chunk_type: "content".to_string(),
        content: Some(content),
        think_tag: None,
        think_duration_ms: None,
        tool_name: None,
        tool_params: None,
        tool_result: None,
        error: None,
        is_finished: Some(false),
        usage: None,
    };
    let done_chunk = AgentStreamChunk {
        chunk_type: "done".to_string(),
        content: None,
        think_tag: None,
        think_duration_ms: None,
        tool_name: None,
        tool_params: None,
        tool_result: None,
        error: None,
        is_finished: Some(true),
        usage: Some(TokenUsage {
            prompt_tokens: prompt.chars().count() as u64 / 2 + 1,
            completion_tokens: 24,
            total_tokens: (prompt.chars().count() as u64 / 2 + 1) + 24,
        }),
    };

    let events = vec![think_chunk, content_chunk, done_chunk]
        .into_iter()
        .filter_map(|chunk| serde_json::to_string(&chunk).ok())
        .map(|text| Ok(Event::default().data(text)))
        .collect::<Vec<_>>();
    let stream = stream::iter(events);
    Sse::new(stream)
}

#[instrument]
async fn abort(Path(request_id): Path<String>) -> Result<Json<serde_json::Value>, ApiError> {
    let mut guard = ABORTED_REQUESTS.write().await;
    guard.insert(request_id.clone());
    Ok(Json(serde_json::json!({
        "success": true,
        "requestId": request_id,
    })))
}

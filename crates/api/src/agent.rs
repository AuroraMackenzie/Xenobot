//! Agent API module for Xenobot HTTP API.
//!
//! Provides a legal-safe, local-readonly tool-calling loop for chat data analysis.

use axum::{
    extract::Path,
    response::{sse::Event, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Row, SqlitePool};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::instrument;

use crate::ApiError;

static ABORTED_REQUESTS: Lazy<RwLock<HashSet<String>>> = Lazy::new(|| RwLock::new(HashSet::new()));

const TOOL_SEARCH_MESSAGES: &str = "search_messages";
const TOOL_GET_RECENT_MESSAGES: &str = "get_recent_messages";
const TOOL_MEMBER_STATS: &str = "member_stats";
const TOOL_TIME_STATS: &str = "time_stats";
const TOOL_MEMBER_LIST: &str = "member_list";
const TOOL_NICKNAME_HISTORY: &str = "nickname_history";
const TOOL_CONVERSATION_BETWEEN: &str = "conversation_between";
const TOOL_MESSAGE_CONTEXT: &str = "message_context";
const TOOL_SEARCH_SESSIONS: &str = "search_sessions";
const TOOL_GET_SESSION_MESSAGES: &str = "get_session_messages";
const TOOL_GET_SESSION_SUMMARY: &str = "get_session_summary";
const TOOL_SEMANTIC_SEARCH: &str = "semantic_search";

const ALL_TOOLS: [&str; 12] = [
    TOOL_SEARCH_MESSAGES,
    TOOL_GET_RECENT_MESSAGES,
    TOOL_MEMBER_STATS,
    TOOL_TIME_STATS,
    TOOL_MEMBER_LIST,
    TOOL_NICKNAME_HISTORY,
    TOOL_CONVERSATION_BETWEEN,
    TOOL_MESSAGE_CONTEXT,
    TOOL_SEARCH_SESSIONS,
    TOOL_GET_SESSION_MESSAGES,
    TOOL_GET_SESSION_SUMMARY,
    TOOL_SEMANTIC_SEARCH,
];

const TOOL_ALIAS_ENTRIES: &[(&str, &[&str])] = &[
    (TOOL_SEARCH_MESSAGES, &["searchmessages", "searchMessages"]),
    (
        TOOL_GET_RECENT_MESSAGES,
        &[
            "recent_messages",
            "getrecentmessages",
            "recentmessages",
            "getRecentMessages",
            "recentMessages",
        ],
    ),
    (
        TOOL_MEMBER_STATS,
        &[
            "get_member_stats",
            "memberstats",
            "getmemberstats",
            "getMemberStats",
        ],
    ),
    (
        TOOL_TIME_STATS,
        &[
            "get_time_stats",
            "timestats",
            "gettimestats",
            "getTimeStats",
        ],
    ),
    (
        TOOL_MEMBER_LIST,
        &[
            "get_member_list",
            "get_group_members",
            "memberlist",
            "getmemberlist",
            "getgroupmembers",
            "getMemberList",
            "getGroupMembers",
        ],
    ),
    (
        TOOL_NICKNAME_HISTORY,
        &[
            "member_name_history",
            "get_member_name_history",
            "nicknamehistory",
            "membernamehistory",
            "getmembernamehistory",
            "memberNameHistory",
            "getMemberNameHistory",
        ],
    ),
    (
        TOOL_CONVERSATION_BETWEEN,
        &[
            "get_conversation_between",
            "conversationbetween",
            "getconversationbetween",
            "conversationBetween",
            "getConversationBetween",
        ],
    ),
    (
        TOOL_MESSAGE_CONTEXT,
        &[
            "get_message_context",
            "messagecontext",
            "getmessagecontext",
            "messageContext",
            "getMessageContext",
        ],
    ),
    (TOOL_SEARCH_SESSIONS, &["searchsessions", "searchSessions"]),
    (
        TOOL_GET_SESSION_MESSAGES,
        &[
            "session_messages",
            "getsessionmessages",
            "sessionmessages",
            "sessionMessages",
            "getSessionMessages",
        ],
    ),
    (
        TOOL_GET_SESSION_SUMMARY,
        &[
            "session_summary",
            "get_session_summaries",
            "sessionsummary",
            "getsessionsummary",
            "getsessionsummaries",
            "sessionSummary",
            "getSessionSummary",
            "getSessionSummaries",
        ],
    ),
    (
        TOOL_SEMANTIC_SEARCH,
        &[
            "semantic_search_messages",
            "semanticsearch",
            "semanticsearchmessages",
            "semanticSearch",
            "semanticSearchMessages",
        ],
    ),
];

static TOOL_ALIAS_LOOKUP: Lazy<HashMap<String, &'static str>> = Lazy::new(|| {
    let mut lookup: HashMap<String, &'static str> = HashMap::new();
    for canonical in ALL_TOOLS {
        lookup.insert(normalize_tool_alias_input(canonical), canonical);
    }
    for (canonical, aliases) in TOOL_ALIAS_ENTRIES {
        lookup.insert(normalize_tool_alias_input(canonical), canonical);
        for alias in *aliases {
            lookup.insert(normalize_tool_alias_input(alias), canonical);
        }
    }
    lookup
});

/// Agent API router.
pub fn router() -> Router {
    Router::new()
        .route("/tools", get(list_tools))
        .route("/run-stream", post(run_stream))
        .route("/abort/:request_id", post(abort))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentToolDefinition {
    name: String,
    description: String,
    parameters: serde_json::Value,
    aliases: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TimeFilter {
    #[serde(alias = "start_ts")]
    start_ts: Option<i64>,
    #[serde(alias = "end_ts")]
    end_ts: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct OwnerInfo {
    #[serde(default, alias = "member_id", alias = "memberId")]
    id: Option<i64>,
    #[serde(default, alias = "platform_id", alias = "platformId")]
    platform_id: Option<String>,
    #[serde(default, alias = "display_name", alias = "displayName")]
    name: Option<String>,
    #[serde(alias = "avatar_url")]
    avatar_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ToolContext {
    #[serde(alias = "session_id")]
    session_id: String,
    #[serde(alias = "time_filter")]
    time_filter: Option<TimeFilter>,
    #[serde(alias = "max_messages_limit")]
    max_messages_limit: Option<i32>,
    #[serde(alias = "owner_info")]
    owner_info: Option<OwnerInfo>,
    locale: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TokenUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct AgentStreamChunk {
    #[serde(rename = "type")]
    chunk_type: String, // content | think | tool_start | tool_result | done | error
    content: Option<String>,
    think_tag: Option<String>,
    think_duration_ms: Option<u64>,
    tool_name: Option<String>,
    tool_params: Option<serde_json::Value>,
    tool_result: Option<serde_json::Value>,
    error: Option<String>,
    error_code: Option<String>,
    is_finished: Option<bool>,
    usage: Option<TokenUsage>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptConfig {
    #[serde(alias = "role_definition")]
    role_definition: String,
    #[serde(alias = "response_rules")]
    response_rules: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunStreamRequest {
    #[serde(alias = "request_id")]
    request_id: Option<String>,
    #[serde(alias = "user_message")]
    user_message: String,
    context: ToolContext,
    #[serde(alias = "history_messages")]
    history_messages: Option<Vec<serde_json::Value>>,
    #[serde(alias = "chat_type")]
    chat_type: Option<String>, // group | private
    #[serde(alias = "prompt_config")]
    prompt_config: Option<PromptConfig>,
    locale: Option<String>,
    #[serde(alias = "forced_tools")]
    forced_tools: Option<Vec<String>>,
    #[serde(alias = "max_rounds")]
    max_rounds: Option<u32>,
    #[serde(alias = "include_tool_results")]
    include_tool_results: Option<bool>,
}

#[derive(Debug, Clone)]
struct ToolExecution {
    name: String,
    result: serde_json::Value,
}

#[derive(Debug, Clone)]
struct AgentRuntimeContext {
    meta_id: i64,
    filter: Option<TimeFilter>,
    max_messages_limit: i64,
    owner_info: Option<OwnerInfo>,
    owner_member_id: Option<i64>,
}

#[instrument]
async fn list_tools() -> Json<Vec<AgentToolDefinition>> {
    let tools = vec![
        AgentToolDefinition {
            name: TOOL_SEARCH_MESSAGES.to_string(),
            description: "Keyword search over messages in the current imported chat.".to_string(),
            parameters: serde_json::json!({
                "required": [],
                "optional": ["query", "limit", "startTs", "endTs"],
                "inferredFromPrompt": ["query", "keywords"],
                "notes": "When query is missing, keywords are inferred from the user message."
            }),
            aliases: tool_aliases_for_client(TOOL_SEARCH_MESSAGES),
        },
        AgentToolDefinition {
            name: TOOL_GET_RECENT_MESSAGES.to_string(),
            description: "Fetch latest messages under optional time filter.".to_string(),
            parameters: serde_json::json!({
                "required": [],
                "optional": ["limit", "startTs", "endTs"],
                "inferredFromPrompt": [],
                "notes": "Limit is bounded by runtime max_messages_limit."
            }),
            aliases: tool_aliases_for_client(TOOL_GET_RECENT_MESSAGES),
        },
        AgentToolDefinition {
            name: TOOL_MEMBER_STATS.to_string(),
            description: "Compute member activity ranking and percentage.".to_string(),
            parameters: serde_json::json!({
                "required": [],
                "optional": ["topN"],
                "inferredFromPrompt": [],
                "notes": "Current implementation returns top members with fixed safety bound."
            }),
            aliases: tool_aliases_for_client(TOOL_MEMBER_STATS),
        },
        AgentToolDefinition {
            name: TOOL_TIME_STATS.to_string(),
            description: "Compute hourly/weekday activity distributions.".to_string(),
            parameters: serde_json::json!({
                "required": [],
                "optional": ["dimension"],
                "inferredFromPrompt": [],
                "notes": "Returns hourly and weekday distributions for current context."
            }),
            aliases: tool_aliases_for_client(TOOL_TIME_STATS),
        },
        AgentToolDefinition {
            name: TOOL_MEMBER_LIST.to_string(),
            description: "List members with aliases and message counts.".to_string(),
            parameters: serde_json::json!({
                "required": [],
                "optional": ["limit", "search"],
                "inferredFromPrompt": [],
                "notes": "Member list is constrained by an internal upper bound."
            }),
            aliases: tool_aliases_for_client(TOOL_MEMBER_LIST),
        },
        AgentToolDefinition {
            name: TOOL_NICKNAME_HISTORY.to_string(),
            description: "Query nickname/account name history for a member.".to_string(),
            parameters: serde_json::json!({
                "required": [],
                "optional": ["memberId"],
                "inferredFromPrompt": ["memberId", "ownerInfo.memberId", "ownerInfo.platformId"],
                "notes": "memberId is inferred from user prompt numeric ids or owner context."
            }),
            aliases: tool_aliases_for_client(TOOL_NICKNAME_HISTORY),
        },
        AgentToolDefinition {
            name: TOOL_CONVERSATION_BETWEEN.to_string(),
            description: "Query conversation stream between two members.".to_string(),
            parameters: serde_json::json!({
                "required": [],
                "optional": ["memberId1", "memberId2", "startTs", "endTs", "limit"],
                "inferredFromPrompt": ["memberId1", "memberId2"],
                "notes": "When member ids are missing, tool returns guidance payload instead of failure."
            }),
            aliases: tool_aliases_for_client(TOOL_CONVERSATION_BETWEEN),
        },
        AgentToolDefinition {
            name: TOOL_MESSAGE_CONTEXT.to_string(),
            description: "Fetch context window around a message.".to_string(),
            parameters: serde_json::json!({
                "required": [],
                "optional": ["messageId", "messageIds", "contextSize"],
                "inferredFromPrompt": ["messageId"],
                "notes": "Supports single message id or multiple message ids."
            }),
            aliases: tool_aliases_for_client(TOOL_MESSAGE_CONTEXT),
        },
        AgentToolDefinition {
            name: TOOL_SEARCH_SESSIONS.to_string(),
            description: "List segmented chat sessions in current meta session.".to_string(),
            parameters: serde_json::json!({
                "required": [],
                "optional": ["limit"],
                "inferredFromPrompt": [],
                "notes": "Results are sorted by time span and bounded by safe limit."
            }),
            aliases: tool_aliases_for_client(TOOL_SEARCH_SESSIONS),
        },
        AgentToolDefinition {
            name: TOOL_GET_SESSION_MESSAGES.to_string(),
            description: "Fetch messages inside a segmented chat session.".to_string(),
            parameters: serde_json::json!({
                "required": [],
                "optional": ["chatSessionId", "limit"],
                "inferredFromPrompt": ["chatSessionId"],
                "notes": "chatSessionId is inferred from numeric ids in prompt when absent."
            }),
            aliases: tool_aliases_for_client(TOOL_GET_SESSION_MESSAGES),
        },
        AgentToolDefinition {
            name: TOOL_GET_SESSION_SUMMARY.to_string(),
            description: "Get session summary and aggregate stats.".to_string(),
            parameters: serde_json::json!({
                "required": [],
                "optional": ["chatSessionId"],
                "inferredFromPrompt": ["chatSessionId"],
                "notes": "chatSessionId is inferred from numeric ids in prompt when absent."
            }),
            aliases: tool_aliases_for_client(TOOL_GET_SESSION_SUMMARY),
        },
        AgentToolDefinition {
            name: TOOL_SEMANTIC_SEARCH.to_string(),
            description: "Semantic similarity search over message content.".to_string(),
            parameters: serde_json::json!({
                "required": [],
                "optional": ["query", "limit", "threshold"],
                "inferredFromPrompt": ["query", "keywords"],
                "notes": "Query defaults to user prompt and runs local vector similarity search."
            }),
            aliases: tool_aliases_for_client(TOOL_SEMANTIC_SEARCH),
        },
    ];
    Json(tools)
}

#[instrument]
async fn run_stream(
    Json(req): Json<RunStreamRequest>,
) -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let locale = req
        .locale
        .clone()
        .or(req.context.locale.clone())
        .unwrap_or_else(|| "zh-CN".to_string());
    let prompt = req.user_message.trim().to_string();
    let request_id = req
        .request_id
        .clone()
        .unwrap_or_else(|| format!("agent_req_{}", now_nanos()));
    let max_rounds = req.max_rounds.unwrap_or(5).clamp(1, 5) as usize;
    let include_tool_results = req.include_tool_results.unwrap_or(true);

    let mut chunks: Vec<AgentStreamChunk> = Vec::new();
    let prompt_tokens = ((prompt.chars().count() as u64) / 2).saturating_add(1);

    if prompt.is_empty() {
        chunks.push(error_chunk(
            if locale.starts_with("zh") {
                "请提供一个问题，我才能开始分析。".to_string()
            } else {
                "Please provide a question so the agent can run tools.".to_string()
            },
            "error.invalid_prompt",
            true,
        ));
        return sse_from_chunks(chunks);
    }

    let meta_id = match parse_meta_id(&req.context.session_id) {
        Ok(v) => v,
        Err(err) => {
            chunks.push(error_chunk(
                err.to_string(),
                "error.invalid_session_id",
                true,
            ));
            return sse_from_chunks(chunks);
        }
    };

    let pool = match get_pool().await {
        Ok(p) => p,
        Err(err) => {
            chunks.push(error_chunk(err.to_string(), "error.database", true));
            return sse_from_chunks(chunks);
        }
    };

    let owner_member_id =
        resolve_owner_member_id(req.context.owner_info.as_ref(), meta_id, pool.as_ref()).await;

    chunks.push(AgentStreamChunk {
        chunk_type: "think".to_string(),
        content: Some(if locale.starts_with("zh") {
            "正在分析问题并规划可执行工具…".to_string()
        } else {
            "Analyzing request and selecting executable tools...".to_string()
        }),
        think_tag: Some("planning".to_string()),
        think_duration_ms: Some(80),
        tool_name: None,
        tool_params: None,
        tool_result: None,
        error: None,
        error_code: None,
        is_finished: Some(false),
        usage: None,
    });

    let runtime = AgentRuntimeContext {
        meta_id,
        filter: req.context.time_filter.clone(),
        max_messages_limit: i64::from(req.context.max_messages_limit.unwrap_or(50).clamp(10, 500)),
        owner_info: req.context.owner_info.clone(),
        owner_member_id,
    };

    let tool_plan = build_tool_plan(&prompt, req.forced_tools.clone());
    let mut executions: Vec<ToolExecution> = Vec::new();
    let mut tools_used: Vec<String> = Vec::new();

    for (round, tool_name) in tool_plan.into_iter().take(max_rounds).enumerate() {
        if is_request_aborted(&request_id).await {
            chunks.push(error_chunk(
                if locale.starts_with("zh") {
                    format!("请求 {} 已终止。", request_id)
                } else {
                    format!("Request {} has been aborted.", request_id)
                },
                "error.request_aborted",
                true,
            ));
            clear_abort_marker(&request_id).await;
            return sse_from_chunks(chunks);
        }

        let params = tool_params_preview(&tool_name, &prompt, &runtime);
        chunks.push(AgentStreamChunk {
            chunk_type: "tool_start".to_string(),
            content: None,
            think_tag: Some("tool".to_string()),
            think_duration_ms: Some(20),
            tool_name: Some(tool_name.clone()),
            tool_params: Some(params),
            tool_result: None,
            error: None,
            error_code: None,
            is_finished: Some(false),
            usage: None,
        });

        match execute_tool(&tool_name, &prompt, &runtime, pool.as_ref()).await {
            Ok(result) => {
                let returned = if include_tool_results {
                    result.clone()
                } else {
                    summarize_tool_result(&result)
                };
                chunks.push(AgentStreamChunk {
                    chunk_type: "tool_result".to_string(),
                    content: None,
                    think_tag: Some("tool".to_string()),
                    think_duration_ms: Some(30),
                    tool_name: Some(tool_name.clone()),
                    tool_params: None,
                    tool_result: Some(returned),
                    error: None,
                    error_code: None,
                    is_finished: Some(false),
                    usage: None,
                });
                tools_used.push(tool_name.clone());
                executions.push(ToolExecution {
                    name: tool_name.clone(),
                    result,
                });
            }
            Err(err) => {
                chunks.push(AgentStreamChunk {
                    chunk_type: "error".to_string(),
                    content: None,
                    think_tag: Some("tool".to_string()),
                    think_duration_ms: Some(0),
                    tool_name: Some(tool_name.clone()),
                    tool_params: None,
                    tool_result: None,
                    error: Some(err.to_string()),
                    error_code: Some(api_error_code(&err).to_string()),
                    is_finished: Some(false),
                    usage: None,
                });
            }
        }

        if round + 1 >= max_rounds {
            break;
        }
    }

    let final_text = build_final_answer(&prompt, &tools_used, &executions, &locale);
    chunks.push(AgentStreamChunk {
        chunk_type: "content".to_string(),
        content: Some(final_text),
        think_tag: None,
        think_duration_ms: None,
        tool_name: None,
        tool_params: None,
        tool_result: None,
        error: None,
        error_code: None,
        is_finished: Some(false),
        usage: None,
    });

    let completion_tokens = (tools_used.len() as u64)
        .saturating_mul(40)
        .saturating_add(32);
    chunks.push(AgentStreamChunk {
        chunk_type: "done".to_string(),
        content: None,
        think_tag: None,
        think_duration_ms: None,
        tool_name: None,
        tool_params: None,
        tool_result: Some(serde_json::json!({
            "requestId": request_id,
            "toolsUsed": tools_used,
            "toolRounds": executions.len() as u32,
        })),
        error: None,
        error_code: None,
        is_finished: Some(true),
        usage: Some(TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens.saturating_add(completion_tokens),
        }),
    });

    clear_abort_marker(&request_id).await;
    sse_from_chunks(chunks)
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

fn sse_from_chunks(
    chunks: Vec<AgentStreamChunk>,
) -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let events = chunks
        .into_iter()
        .filter_map(|chunk| serde_json::to_string(&chunk).ok())
        .map(|text| Ok(Event::default().data(text)))
        .collect::<Vec<_>>();
    Sse::new(stream::iter(events))
}

fn error_chunk(message: String, code: &str, finished: bool) -> AgentStreamChunk {
    AgentStreamChunk {
        chunk_type: "error".to_string(),
        content: None,
        think_tag: None,
        think_duration_ms: None,
        tool_name: None,
        tool_params: None,
        tool_result: None,
        error: Some(message),
        error_code: Some(code.to_string()),
        is_finished: Some(finished),
        usage: None,
    }
}

fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

fn parse_meta_id(value: &str) -> Result<i64, ApiError> {
    value
        .parse::<i64>()
        .map_err(|_| ApiError::InvalidRequest("invalid session_id".to_string()))
}

async fn get_pool() -> Result<Arc<SqlitePool>, ApiError> {
    crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))
}

async fn is_request_aborted(request_id: &str) -> bool {
    let guard = ABORTED_REQUESTS.read().await;
    guard.contains(request_id)
}

async fn clear_abort_marker(request_id: &str) {
    let mut guard = ABORTED_REQUESTS.write().await;
    guard.remove(request_id);
}

fn build_tool_plan(prompt: &str, forced_tools: Option<Vec<String>>) -> Vec<String> {
    if let Some(forced) = forced_tools {
        let mut dedup = Vec::new();
        for tool in forced {
            if let Some(canonical) = normalize_tool_name(&tool) {
                if !dedup.iter().any(|x| x == canonical) {
                    dedup.push(canonical.to_string());
                }
            }
        }
        if !dedup.is_empty() {
            return dedup;
        }
    }

    let text = prompt.to_lowercase();
    let mut tools = Vec::new();

    if contains_any(&text, &["语义", "semantic", "相似", "embedding"]) {
        push_unique(&mut tools, TOOL_SEMANTIC_SEARCH);
    }
    if contains_any(&text, &["关键词", "搜索", "search", "query"]) {
        push_unique(&mut tools, TOOL_SEARCH_MESSAGES);
    }
    if contains_any(&text, &["最近", "recent", "latest"]) {
        push_unique(&mut tools, TOOL_GET_RECENT_MESSAGES);
    }
    if contains_any(&text, &["成员统计", "活跃", "member stat", "top member"]) {
        push_unique(&mut tools, TOOL_MEMBER_STATS);
    }
    if contains_any(&text, &["时间分布", "hourly", "weekday", "time stat"]) {
        push_unique(&mut tools, TOOL_TIME_STATS);
    }
    if contains_any(&text, &["成员列表", "member list", "成员清单"]) {
        push_unique(&mut tools, TOOL_MEMBER_LIST);
    }
    if contains_any(&text, &["昵称", "改名", "name history", "nickname"]) {
        push_unique(&mut tools, TOOL_NICKNAME_HISTORY);
    }
    if contains_any(&text, &["两人", "between", "双人", "对话"]) {
        push_unique(&mut tools, TOOL_CONVERSATION_BETWEEN);
    }
    if contains_any(&text, &["上下文", "context", "前后"]) {
        push_unique(&mut tools, TOOL_MESSAGE_CONTEXT);
    }
    if contains_any(&text, &["会话列表", "session list", "chat session"]) {
        push_unique(&mut tools, TOOL_SEARCH_SESSIONS);
    }
    if contains_any(&text, &["会话消息", "session messages", "分段消息"]) {
        push_unique(&mut tools, TOOL_GET_SESSION_MESSAGES);
    }
    if contains_any(&text, &["会话摘要", "summary", "总结"]) {
        push_unique(&mut tools, TOOL_GET_SESSION_SUMMARY);
    }

    if tools.is_empty() {
        push_unique(&mut tools, TOOL_SEARCH_MESSAGES);
        push_unique(&mut tools, TOOL_GET_RECENT_MESSAGES);
    }
    tools
}

fn api_error_code(err: &ApiError) -> &'static str {
    match err {
        ApiError::InvalidRequest(_) => "error.invalid_request",
        ApiError::NotFound(_) => "error.not_found",
        ApiError::Database(_) => "error.database",
        ApiError::Timeout(_) => "error.timeout",
        ApiError::Auth(_) => "error.auth",
        ApiError::Http(_) => "error.http",
        ApiError::Io(_) | ApiError::Json(_) | ApiError::Internal(_) | ApiError::Core(_) => {
            "error.internal"
        }
        ApiError::NotImplemented(_) => "error.not_implemented",
        #[cfg(feature = "wechat")]
        ApiError::WeChat(_) => "error.wechat",
    }
}

async fn resolve_owner_member_id(
    owner_info: Option<&OwnerInfo>,
    meta_id: i64,
    pool: &SqlitePool,
) -> Option<i64> {
    let owner = owner_info?;

    if let Some(owner_id) = owner.id.filter(|v| *v > 0) {
        if let Ok(Some(found_id)) = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT m.id
            FROM member m
            JOIN message msg ON msg.sender_id = m.id
            WHERE m.id = ?1 AND msg.meta_id = ?2
            LIMIT 1
            "#,
        )
        .bind(owner_id)
        .bind(meta_id)
        .fetch_optional(pool)
        .await
        {
            return Some(found_id);
        }
    }

    let platform_id = owner
        .platform_id
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())?;

    if let Ok(Some(found_id)) = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT m.id
        FROM member m
        JOIN message msg ON msg.sender_id = m.id
        WHERE m.platform_id = ?1 AND msg.meta_id = ?2
        ORDER BY msg.ts DESC, msg.id DESC
        LIMIT 1
        "#,
    )
    .bind(platform_id)
    .bind(meta_id)
    .fetch_optional(pool)
    .await
    {
        return Some(found_id);
    }

    if let Ok(Some(found_id)) = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id
        FROM member
        WHERE platform_id = ?1
        ORDER BY id DESC
        LIMIT 1
        "#,
    )
    .bind(platform_id)
    .fetch_optional(pool)
    .await
    {
        return Some(found_id);
    }

    None
}

fn normalize_tool_name(raw: &str) -> Option<&'static str> {
    let normalized = normalize_tool_alias_input(raw);
    TOOL_ALIAS_LOOKUP.get(normalized.as_str()).copied()
}

fn normalize_tool_alias_input(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(trimmed.len() + 8);
    let mut prev_was_sep = false;
    for ch in trimmed.chars() {
        if ch.is_ascii_uppercase() {
            if !out.is_empty() && !prev_was_sep {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_was_sep = false;
            continue;
        }
        if ch == '-' || ch == ' ' || ch == '/' {
            if !out.is_empty() && !prev_was_sep {
                out.push('_');
            }
            prev_was_sep = true;
            continue;
        }
        if ch == '_' {
            if !out.is_empty() && !prev_was_sep {
                out.push('_');
            }
            prev_was_sep = true;
            continue;
        }
        out.push(ch.to_ascii_lowercase());
        prev_was_sep = false;
    }
    out
}

fn tool_aliases_for_client(canonical: &str) -> Vec<String> {
    let mut aliases = vec![canonical.to_string()];
    if let Some((_, extra_aliases)) = TOOL_ALIAS_ENTRIES
        .iter()
        .find(|(name, _)| *name == canonical)
    {
        for alias in *extra_aliases {
            if !aliases.iter().any(|value| value == alias) {
                aliases.push((*alias).to_string());
            }
        }
    }
    aliases
}

fn push_unique(tools: &mut Vec<String>, name: &str) {
    if !tools.iter().any(|x| x == name) {
        tools.push(name.to_string());
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|s| text.contains(s))
}

fn tokenize_query(text: &str) -> Vec<String> {
    let stop_words = [
        "请",
        "帮",
        "我",
        "一下",
        "一下子",
        "查看",
        "分析",
        "消息",
        "聊天",
        "记录",
        "the",
        "and",
        "for",
        "with",
        "from",
        "that",
        "this",
        "what",
        "how",
        "about",
    ];
    text.split(|c: char| !c.is_alphanumeric() && !('\u{4e00}' <= c && c <= '\u{9fff}'))
        .map(|s| s.trim().to_lowercase())
        .filter(|s| s.chars().count() >= 2)
        .filter(|s| !stop_words.iter().any(|w| *w == s))
        .collect()
}

fn parse_numeric_ids(text: &str) -> Vec<i64> {
    let mut ids = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            if let Ok(v) = current.parse::<i64>() {
                ids.push(v);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(v) = current.parse::<i64>() {
            ids.push(v);
        }
    }
    ids
}

fn tool_params_preview(
    tool_name: &str,
    prompt: &str,
    runtime: &AgentRuntimeContext,
) -> serde_json::Value {
    let ids = parse_numeric_ids(prompt);
    let keywords = tokenize_query(prompt);
    let start_ts = runtime.filter.as_ref().and_then(|f| f.start_ts);
    let end_ts = runtime.filter.as_ref().and_then(|f| f.end_ts);
    let keyword_hints = keywords.into_iter().take(5).collect::<Vec<_>>();
    let numeric_hints = ids.clone().into_iter().take(5).collect::<Vec<_>>();
    let bounded_limit = runtime.max_messages_limit.clamp(1, 500);

    match tool_name {
        TOOL_SEARCH_MESSAGES => serde_json::json!({
            "query": prompt.trim(),
            "keywords": keyword_hints,
            "limit": bounded_limit.min(100),
            "startTs": start_ts,
            "endTs": end_ts
        }),
        TOOL_GET_RECENT_MESSAGES => serde_json::json!({
            "limit": bounded_limit.min(100),
            "startTs": start_ts,
            "endTs": end_ts
        }),
        TOOL_MEMBER_STATS => serde_json::json!({
            "topN": 10
        }),
        TOOL_TIME_STATS => serde_json::json!({
            "dimension": "hourly|weekday"
        }),
        TOOL_MEMBER_LIST => serde_json::json!({
            "limit": 300
        }),
        TOOL_NICKNAME_HISTORY => serde_json::json!({
            "memberId": ids
                .first()
                .copied()
                .or(runtime.owner_member_id)
        }),
        TOOL_CONVERSATION_BETWEEN => serde_json::json!({
            "memberId1": ids.first().copied(),
            "memberId2": ids.get(1).copied(),
            "startTs": start_ts,
            "endTs": end_ts
        }),
        TOOL_MESSAGE_CONTEXT => serde_json::json!({
            "messageId": ids.first().copied(),
            "contextSize": 20
        }),
        TOOL_SEARCH_SESSIONS => serde_json::json!({
            "limit": 20
        }),
        TOOL_GET_SESSION_MESSAGES | TOOL_GET_SESSION_SUMMARY => serde_json::json!({
            "chatSessionId": ids.first().copied()
        }),
        TOOL_SEMANTIC_SEARCH => serde_json::json!({
            "query": prompt.trim(),
            "keywords": keyword_hints,
            "limit": bounded_limit.min(50),
            "threshold": 0.30
        }),
        _ => serde_json::json!({
            "metaId": runtime.meta_id,
            "startTs": start_ts,
            "endTs": end_ts,
            "maxMessagesLimit": runtime.max_messages_limit,
            "keywordHints": keyword_hints,
            "numericHints": numeric_hints
        }),
    }
}

async fn execute_tool(
    tool_name: &str,
    prompt: &str,
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<serde_json::Value, ApiError> {
    match tool_name {
        TOOL_SEARCH_MESSAGES => tool_search_messages(prompt, runtime, pool).await,
        TOOL_GET_RECENT_MESSAGES => tool_get_recent_messages(runtime, pool).await,
        TOOL_MEMBER_STATS => tool_member_stats(runtime, pool).await,
        TOOL_TIME_STATS => tool_time_stats(runtime, pool).await,
        TOOL_MEMBER_LIST => tool_member_list(runtime, pool).await,
        TOOL_NICKNAME_HISTORY => tool_nickname_history(prompt, runtime, pool).await,
        TOOL_CONVERSATION_BETWEEN => tool_conversation_between(prompt, runtime, pool).await,
        TOOL_MESSAGE_CONTEXT => tool_message_context(prompt, runtime, pool).await,
        TOOL_SEARCH_SESSIONS => tool_search_sessions(runtime, pool).await,
        TOOL_GET_SESSION_MESSAGES => tool_get_session_messages(prompt, runtime, pool).await,
        TOOL_GET_SESSION_SUMMARY => tool_get_session_summary(prompt, runtime, pool).await,
        TOOL_SEMANTIC_SEARCH => tool_semantic_search(prompt, runtime, pool).await,
        _ => Err(ApiError::InvalidRequest(format!(
            "unknown tool: {}",
            tool_name
        ))),
    }
}

fn apply_time_filter(
    qb: &mut QueryBuilder<'_, sqlx::Sqlite>,
    filter: &Option<TimeFilter>,
    alias: &str,
) {
    if let Some(start_ts) = filter.as_ref().and_then(|f| f.start_ts) {
        qb.push(" AND ")
            .push(alias)
            .push(".ts >= ")
            .push_bind(start_ts);
    }
    if let Some(end_ts) = filter.as_ref().and_then(|f| f.end_ts) {
        qb.push(" AND ")
            .push(alias)
            .push(".ts <= ")
            .push_bind(end_ts);
    }
}

fn extract_message_json(row: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    serde_json::json!({
        "id": row.get::<i64, _>("id"),
        "timestamp": row.get::<i64, _>("ts"),
        "senderName": row.get::<String, _>("sender_name"),
        "senderPlatformId": row.get::<String, _>("sender_platform_id"),
        "content": row.get::<String, _>("content"),
        "type": row.get::<i64, _>("msg_type"),
    })
}

async fn tool_search_messages(
    prompt: &str,
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<serde_json::Value, ApiError> {
    let mut keywords = tokenize_query(prompt);
    keywords.truncate(4);
    let limit = runtime.max_messages_limit.min(100);

    let mut qb = QueryBuilder::<sqlx::Sqlite>::new(
        r#"
        SELECT
            msg.id as id,
            msg.ts as ts,
            COALESCE(msg.sender_group_nickname, msg.sender_account_name, m.group_nickname, m.account_name, m.platform_id, '') as sender_name,
            m.platform_id as sender_platform_id,
            COALESCE(msg.content, '') as content,
            msg.msg_type as msg_type
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id = 
        "#,
    );
    qb.push_bind(runtime.meta_id);
    qb.push(" AND COALESCE(msg.content, '') != ''");
    apply_time_filter(&mut qb, &runtime.filter, "msg");
    if !keywords.is_empty() {
        qb.push(" AND (");
        for (idx, keyword) in keywords.iter().enumerate() {
            if idx > 0 {
                qb.push(" OR ");
            }
            qb.push("LOWER(COALESCE(msg.content, '')) LIKE ")
                .push_bind(format!("%{}%", keyword));
        }
        qb.push(")");
    }
    qb.push(" ORDER BY msg.ts DESC LIMIT ").push_bind(limit);

    let rows = qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let items: Vec<serde_json::Value> = rows.iter().map(extract_message_json).collect();

    Ok(serde_json::json!({
        "count": items.len() as i64,
        "keywords": keywords,
        "items": items
    }))
}

async fn tool_get_recent_messages(
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<serde_json::Value, ApiError> {
    let limit = runtime.max_messages_limit.min(80);
    let mut qb = QueryBuilder::<sqlx::Sqlite>::new(
        r#"
        SELECT
            msg.id as id,
            msg.ts as ts,
            COALESCE(msg.sender_group_nickname, msg.sender_account_name, m.group_nickname, m.account_name, m.platform_id, '') as sender_name,
            m.platform_id as sender_platform_id,
            COALESCE(msg.content, '') as content,
            msg.msg_type as msg_type
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id =
        "#,
    );
    qb.push_bind(runtime.meta_id);
    apply_time_filter(&mut qb, &runtime.filter, "msg");
    qb.push(" ORDER BY msg.ts DESC LIMIT ").push_bind(limit);
    let rows = qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let items: Vec<serde_json::Value> = rows.iter().map(extract_message_json).collect();
    Ok(serde_json::json!({
        "count": items.len() as i64,
        "items": items
    }))
}

async fn tool_member_stats(
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<serde_json::Value, ApiError> {
    let mut qb = QueryBuilder::<sqlx::Sqlite>::new(
        r#"
        SELECT
            m.id as member_id,
            m.platform_id as platform_id,
            COALESCE(m.group_nickname, m.account_name, m.platform_id) as name,
            COUNT(*) as message_count
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id =
        "#,
    );
    qb.push_bind(runtime.meta_id);
    apply_time_filter(&mut qb, &runtime.filter, "msg");
    qb.push(
        " GROUP BY m.id, m.platform_id, COALESCE(m.group_nickname, m.account_name, m.platform_id)",
    );
    qb.push(" ORDER BY message_count DESC, member_id ASC LIMIT 100");

    let rows = qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let total = rows
        .iter()
        .map(|r| r.get::<i64, _>("message_count"))
        .sum::<i64>();

    let members = rows
        .into_iter()
        .map(|row| {
            let message_count = row.get::<i64, _>("message_count");
            let percentage = if total > 0 {
                message_count as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            serde_json::json!({
                "memberId": row.get::<i64, _>("member_id"),
                "platformId": row.get::<String, _>("platform_id"),
                "name": row.get::<String, _>("name"),
                "messageCount": message_count,
                "percentage": percentage
            })
        })
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "totalMessages": total,
        "members": members
    }))
}

async fn tool_time_stats(
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<serde_json::Value, ApiError> {
    let mut hour_qb = QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT CAST(strftime('%H', datetime(msg.ts, 'unixepoch', 'localtime')) AS INTEGER) as period, COUNT(*) as count FROM message msg WHERE msg.meta_id = ",
    );
    hour_qb.push_bind(runtime.meta_id);
    apply_time_filter(&mut hour_qb, &runtime.filter, "msg");
    hour_qb.push(" GROUP BY period ORDER BY period ASC");
    let hourly_rows = hour_qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let hourly = hourly_rows
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "period": row.get::<i64, _>("period"),
                "count": row.get::<i64, _>("count")
            })
        })
        .collect::<Vec<_>>();

    let mut weekday_qb = QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT CAST(strftime('%w', datetime(msg.ts, 'unixepoch', 'localtime')) AS INTEGER) as period, COUNT(*) as count FROM message msg WHERE msg.meta_id = ",
    );
    weekday_qb.push_bind(runtime.meta_id);
    apply_time_filter(&mut weekday_qb, &runtime.filter, "msg");
    weekday_qb.push(" GROUP BY period ORDER BY period ASC");
    let weekday_rows = weekday_qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let weekday = weekday_rows
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "period": row.get::<i64, _>("period"),
                "count": row.get::<i64, _>("count")
            })
        })
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "hourly": hourly,
        "weekday": weekday
    }))
}

async fn tool_member_list(
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<serde_json::Value, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT
            m.id as member_id,
            m.platform_id as platform_id,
            m.account_name as account_name,
            m.group_nickname as group_nickname,
            m.aliases as aliases,
            COUNT(msg.id) as message_count
        FROM member m
        LEFT JOIN message msg ON msg.sender_id = m.id AND msg.meta_id = ?1
        GROUP BY m.id, m.platform_id, m.account_name, m.group_nickname, m.aliases
        ORDER BY message_count DESC, m.id ASC
        LIMIT 300
        "#,
    )
    .bind(runtime.meta_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    let members = rows
        .into_iter()
        .map(|row| {
            let aliases_raw = row.get::<Option<String>, _>("aliases").unwrap_or_default();
            let aliases = if aliases_raw.trim().is_empty() {
                Vec::<String>::new()
            } else {
                serde_json::from_str::<Vec<String>>(&aliases_raw).unwrap_or_default()
            };
            serde_json::json!({
                "memberId": row.get::<i64, _>("member_id"),
                "platformId": row.get::<String, _>("platform_id"),
                "accountName": row.get::<Option<String>, _>("account_name"),
                "groupNickname": row.get::<Option<String>, _>("group_nickname"),
                "aliases": aliases,
                "messageCount": row.get::<i64, _>("message_count")
            })
        })
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "count": members.len() as i64,
        "members": members
    }))
}

fn resolve_member_id_from_prompt(prompt: &str, runtime: &AgentRuntimeContext) -> Option<i64> {
    parse_numeric_ids(prompt)
        .into_iter()
        .find(|id| *id > 0)
        .or(runtime.owner_member_id)
}

async fn tool_nickname_history(
    prompt: &str,
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<serde_json::Value, ApiError> {
    let Some(member_id) = resolve_member_id_from_prompt(prompt, runtime) else {
        return Ok(serde_json::json!({
            "memberId": null,
            "history": [],
            "note": "member id not found in prompt or owner context"
        }));
    };

    let rows = sqlx::query(
        r#"
        SELECT id, member_id, name_type, name, start_ts, end_ts
        FROM member_name_history
        WHERE member_id = ?1
        ORDER BY start_ts DESC
        LIMIT 100
        "#,
    )
    .bind(member_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;
    let history = rows
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "id": row.get::<i64, _>("id"),
                "memberId": row.get::<i64, _>("member_id"),
                "nameType": row.get::<String, _>("name_type"),
                "name": row.get::<String, _>("name"),
                "startTs": row.get::<i64, _>("start_ts"),
                "endTs": row.get::<Option<i64>, _>("end_ts")
            })
        })
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "memberId": member_id,
        "historyCount": history.len() as i64,
        "history": history
    }))
}

async fn pick_two_members(
    prompt: &str,
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<Option<(i64, i64)>, ApiError> {
    let mut ids: Vec<i64> = parse_numeric_ids(prompt)
        .into_iter()
        .filter(|v| *v > 0)
        .take(2)
        .collect();
    if ids.len() >= 2 {
        return Ok(Some((ids[0], ids[1])));
    }

    let rows = sqlx::query(
        r#"
        SELECT sender_id as member_id, COUNT(*) as message_count
        FROM message
        WHERE meta_id = ?1
        GROUP BY sender_id
        ORDER BY message_count DESC, member_id ASC
        LIMIT 2
        "#,
    )
    .bind(runtime.meta_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;
    ids = rows
        .into_iter()
        .map(|r| r.get::<i64, _>("member_id"))
        .collect::<Vec<_>>();
    if ids.len() < 2 {
        return Ok(None);
    }
    Ok(Some((ids[0], ids[1])))
}

async fn tool_conversation_between(
    prompt: &str,
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<serde_json::Value, ApiError> {
    let Some((member_a, member_b)) = pick_two_members(prompt, runtime, pool).await? else {
        return Ok(serde_json::json!({
            "pair": null,
            "messages": [],
            "note": "not enough members to build pair conversation"
        }));
    };

    let mut qb = QueryBuilder::<sqlx::Sqlite>::new(
        r#"
        SELECT
            msg.id as id,
            msg.ts as ts,
            COALESCE(msg.sender_group_nickname, msg.sender_account_name, m.group_nickname, m.account_name, m.platform_id, '') as sender_name,
            m.platform_id as sender_platform_id,
            COALESCE(msg.content, '') as content,
            msg.msg_type as msg_type
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id =
        "#,
    );
    qb.push_bind(runtime.meta_id);
    qb.push(" AND msg.sender_id IN (")
        .push_bind(member_a)
        .push(", ")
        .push_bind(member_b)
        .push(")");
    apply_time_filter(&mut qb, &runtime.filter, "msg");
    qb.push(" ORDER BY msg.ts DESC LIMIT ")
        .push_bind(runtime.max_messages_limit.min(120));
    let rows = qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    let mut messages = rows.iter().map(extract_message_json).collect::<Vec<_>>();
    messages.reverse();
    Ok(serde_json::json!({
        "pair": {"memberId1": member_a, "memberId2": member_b},
        "count": messages.len() as i64,
        "messages": messages
    }))
}

async fn pick_message_id_for_context(
    prompt: &str,
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<Option<i64>, ApiError> {
    if let Some(id) = parse_numeric_ids(prompt).into_iter().find(|v| *v > 0) {
        return Ok(Some(id));
    }

    let keywords = tokenize_query(prompt);
    let mut qb = QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT msg.id as id FROM message msg WHERE msg.meta_id = ",
    );
    qb.push_bind(runtime.meta_id);
    qb.push(" AND COALESCE(msg.content, '') != ''");
    if !keywords.is_empty() {
        qb.push(" AND (");
        for (idx, k) in keywords.iter().enumerate() {
            if idx > 0 {
                qb.push(" OR ");
            }
            qb.push("LOWER(COALESCE(msg.content, '')) LIKE ")
                .push_bind(format!("%{}%", k));
        }
        qb.push(")");
    }
    qb.push(" ORDER BY msg.ts DESC LIMIT 1");
    let row = qb
        .build()
        .fetch_optional(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;
    Ok(row.map(|r| r.get::<i64, _>("id")))
}

async fn tool_message_context(
    prompt: &str,
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<serde_json::Value, ApiError> {
    let Some(message_id) = pick_message_id_for_context(prompt, runtime, pool).await? else {
        return Ok(serde_json::json!({
            "messageId": null,
            "context": [],
            "note": "no matching message found"
        }));
    };
    let context_size = runtime.max_messages_limit.min(10).max(2);

    let center = sqlx::query(
        r#"
        SELECT msg.id as id, msg.ts as ts
        FROM message msg
        WHERE msg.id = ?1 AND msg.meta_id = ?2
        "#,
    )
    .bind(message_id)
    .bind(runtime.meta_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;
    let Some(center_row) = center else {
        return Ok(serde_json::json!({
            "messageId": message_id,
            "context": [],
            "note": "message not found in current session"
        }));
    };
    let center_ts = center_row.get::<i64, _>("ts");

    let before_rows = sqlx::query(
        r#"
        SELECT
            msg.id as id,
            msg.ts as ts,
            COALESCE(msg.sender_group_nickname, msg.sender_account_name, m.group_nickname, m.account_name, m.platform_id, '') as sender_name,
            m.platform_id as sender_platform_id,
            COALESCE(msg.content, '') as content,
            msg.msg_type as msg_type
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id = ?1 AND msg.ts <= ?2
        ORDER BY msg.ts DESC
        LIMIT ?3
        "#,
    )
    .bind(runtime.meta_id)
    .bind(center_ts)
    .bind(context_size)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    let after_rows = sqlx::query(
        r#"
        SELECT
            msg.id as id,
            msg.ts as ts,
            COALESCE(msg.sender_group_nickname, msg.sender_account_name, m.group_nickname, m.account_name, m.platform_id, '') as sender_name,
            m.platform_id as sender_platform_id,
            COALESCE(msg.content, '') as content,
            msg.msg_type as msg_type
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id = ?1 AND msg.ts > ?2
        ORDER BY msg.ts ASC
        LIMIT ?3
        "#,
    )
    .bind(runtime.meta_id)
    .bind(center_ts)
    .bind(context_size)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    let mut context = before_rows
        .iter()
        .map(extract_message_json)
        .collect::<Vec<_>>();
    context.reverse();
    context.extend(after_rows.iter().map(extract_message_json));

    Ok(serde_json::json!({
        "messageId": message_id,
        "contextSize": context_size,
        "context": context
    }))
}

async fn tool_search_sessions(
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<serde_json::Value, ApiError> {
    let rows = sqlx::query(
        r#"
        SELECT id, meta_id, start_ts, end_ts, COALESCE(message_count, 0) as message_count, COALESCE(summary, '') as summary
        FROM chat_session
        WHERE meta_id = ?1
        ORDER BY start_ts DESC
        LIMIT 200
        "#,
    )
    .bind(runtime.meta_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;
    let sessions = rows
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "id": row.get::<i64, _>("id"),
                "metaId": row.get::<i64, _>("meta_id"),
                "startTs": row.get::<i64, _>("start_ts"),
                "endTs": row.get::<i64, _>("end_ts"),
                "messageCount": row.get::<i64, _>("message_count"),
                "summary": row.get::<String, _>("summary")
            })
        })
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "count": sessions.len() as i64,
        "sessions": sessions
    }))
}

async fn resolve_chat_session_id(
    prompt: &str,
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<Option<i64>, ApiError> {
    if let Some(id) = parse_numeric_ids(prompt).into_iter().find(|v| *v > 0) {
        return Ok(Some(id));
    }
    let row = sqlx::query(
        "SELECT id FROM chat_session WHERE meta_id = ?1 ORDER BY start_ts DESC LIMIT 1",
    )
    .bind(runtime.meta_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;
    Ok(row.map(|r| r.get::<i64, _>("id")))
}

async fn tool_get_session_messages(
    prompt: &str,
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<serde_json::Value, ApiError> {
    let Some(chat_session_id) = resolve_chat_session_id(prompt, runtime, pool).await? else {
        return Ok(serde_json::json!({
            "chatSessionId": null,
            "messages": [],
            "note": "no chat session available"
        }));
    };

    let rows = sqlx::query(
        r#"
        SELECT
            msg.id as id,
            msg.ts as ts,
            COALESCE(msg.sender_group_nickname, msg.sender_account_name, m.group_nickname, m.account_name, m.platform_id, '') as sender_name,
            m.platform_id as sender_platform_id,
            COALESCE(msg.content, '') as content,
            msg.msg_type as msg_type
        FROM message_context ctx
        JOIN message msg ON ctx.message_id = msg.id
        JOIN member m ON msg.sender_id = m.id
        WHERE ctx.session_id = ?1 AND msg.meta_id = ?2
        ORDER BY msg.ts ASC
        LIMIT ?3
        "#,
    )
    .bind(chat_session_id)
    .bind(runtime.meta_id)
    .bind(runtime.max_messages_limit.min(300))
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    let messages = rows.iter().map(extract_message_json).collect::<Vec<_>>();
    Ok(serde_json::json!({
        "chatSessionId": chat_session_id,
        "count": messages.len() as i64,
        "messages": messages
    }))
}

async fn tool_get_session_summary(
    prompt: &str,
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<serde_json::Value, ApiError> {
    let Some(chat_session_id) = resolve_chat_session_id(prompt, runtime, pool).await? else {
        return Ok(serde_json::json!({
            "chatSessionId": null,
            "summary": "",
            "stats": {}
        }));
    };

    let row = sqlx::query(
        r#"
        SELECT id, meta_id, start_ts, end_ts, COALESCE(message_count, 0) as message_count, summary
        FROM chat_session
        WHERE id = ?1 AND meta_id = ?2
        "#,
    )
    .bind(chat_session_id)
    .bind(runtime.meta_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;
    let Some(chat_row) = row else {
        return Ok(serde_json::json!({
            "chatSessionId": chat_session_id,
            "summary": "",
            "note": "chat session not found in current meta session"
        }));
    };

    let start_ts = chat_row.get::<i64, _>("start_ts");
    let end_ts = chat_row.get::<i64, _>("end_ts");
    let stored_summary = chat_row
        .get::<Option<String>, _>("summary")
        .unwrap_or_default();

    let member_rows = sqlx::query(
        r#"
        SELECT
            COALESCE(m.group_nickname, m.account_name, m.platform_id) as name,
            COUNT(*) as message_count
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id = ?1 AND msg.ts >= ?2 AND msg.ts <= ?3
        GROUP BY m.id, COALESCE(m.group_nickname, m.account_name, m.platform_id)
        ORDER BY message_count DESC
        LIMIT 3
        "#,
    )
    .bind(runtime.meta_id)
    .bind(start_ts)
    .bind(end_ts)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::Database(e.to_string()))?;

    let top_members = member_rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "name": r.get::<String, _>("name"),
                "messageCount": r.get::<i64, _>("message_count")
            })
        })
        .collect::<Vec<_>>();

    let final_summary = if stored_summary.trim().is_empty() {
        format!(
            "Session {} spans {} - {}, with {} top participants in sampled range.",
            chat_session_id,
            start_ts,
            end_ts,
            top_members.len()
        )
    } else {
        stored_summary
    };

    Ok(serde_json::json!({
        "chatSessionId": chat_session_id,
        "summary": final_summary,
        "stats": {
            "startTs": start_ts,
            "endTs": end_ts,
            "messageCount": chat_row.get::<i64, _>("message_count"),
            "topMembers": top_members
        }
    }))
}

fn rewrite_semantic_query(query: &str) -> String {
    let mut normalized = query.trim().to_lowercase();
    if normalized.is_empty() {
        return String::new();
    }

    let replacements = [
        ("聊天记录", "聊天 消息 记录"),
        ("群聊", "群组 聊天"),
        ("私聊", "私人 聊天"),
        ("语音", "音频"),
        ("图片", "图像 照片"),
        ("msg", "message"),
        ("msgs", "messages"),
        ("chat_history", "chat log"),
        ("im", "instant message"),
    ];
    for (from, to) in replacements {
        normalized = normalized.replace(from, to);
    }
    normalized
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn semantic_chunk_text(text: &str, max_chars: usize, overlap_chars: usize) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    if text.chars().count() <= max_chars {
        return vec![text];
    }

    let mut chunks = Vec::new();
    let chars = text.char_indices().collect::<Vec<_>>();
    let mut start_idx = 0usize;
    while start_idx < chars.len() {
        let end_idx = (start_idx + max_chars).min(chars.len());
        let byte_start = chars[start_idx].0;
        let byte_end = if end_idx >= chars.len() {
            text.len()
        } else {
            chars[end_idx].0
        };
        chunks.push(&text[byte_start..byte_end]);
        if end_idx >= chars.len() {
            break;
        }
        let step = max_chars.saturating_sub(overlap_chars).max(1);
        start_idx = start_idx.saturating_add(step);
    }
    chunks
}

fn semantic_tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && !('\u{4e00}' <= c && c <= '\u{9fff}'))
        .map(|token| token.trim().to_lowercase())
        .filter(|token| !token.is_empty())
        .collect()
}

fn embed_text_for_semantic(text: &str) -> Vec<f32> {
    let chunks = semantic_chunk_text(text, 256, 64);
    if chunks.is_empty() {
        return vec![0.0; 128];
    }

    let mut embedding = vec![0f32; 128];
    for chunk in chunks {
        let tokens = semantic_tokenize(chunk);
        for token in tokens {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            token.hash(&mut hasher);
            let idx = (hasher.finish() as usize) % embedding.len();
            embedding[idx] += 1.0;
        }
    }

    let norm = embedding.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut embedding {
            *value /= norm;
        }
    }
    embedding
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut an = 0.0f32;
    let mut bn = 0.0f32;
    for i in 0..len {
        dot += a[i] * b[i];
        an += a[i] * a[i];
        bn += b[i] * b[i];
    }
    if an <= 0.0 || bn <= 0.0 {
        return 0.0;
    }
    dot / (an.sqrt() * bn.sqrt())
}

async fn tool_semantic_search(
    prompt: &str,
    runtime: &AgentRuntimeContext,
    pool: &SqlitePool,
) -> Result<serde_json::Value, ApiError> {
    let rewritten_query = rewrite_semantic_query(prompt);
    let keywords = tokenize_query(&rewritten_query);
    let limit = runtime.max_messages_limit.min(80);
    let threshold = 0.35f32;

    let mut qb = QueryBuilder::<sqlx::Sqlite>::new(
        r#"
        SELECT
            msg.id as id,
            msg.ts as ts,
            COALESCE(msg.sender_group_nickname, msg.sender_account_name, m.group_nickname, m.account_name, m.platform_id, '') as sender_name,
            m.platform_id as sender_platform_id,
            COALESCE(msg.content, '') as content,
            msg.msg_type as msg_type
        FROM message msg
        JOIN member m ON msg.sender_id = m.id
        WHERE msg.meta_id =
        "#,
    );
    qb.push_bind(runtime.meta_id);
    qb.push(" AND COALESCE(msg.content, '') != ''");
    apply_time_filter(&mut qb, &runtime.filter, "msg");
    if !keywords.is_empty() {
        qb.push(" AND (");
        for (idx, k) in keywords.iter().enumerate() {
            if idx > 0 {
                qb.push(" OR ");
            }
            qb.push("LOWER(COALESCE(msg.content, '')) LIKE ")
                .push_bind(format!("%{}%", k));
        }
        qb.push(")");
    }
    qb.push(" ORDER BY msg.ts DESC LIMIT ").push_bind(limit * 6);

    let rows = qb
        .build()
        .fetch_all(pool)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let query_embedding = embed_text_for_semantic(&rewritten_query);
    let mut ranked: Vec<(f32, serde_json::Value)> = rows
        .iter()
        .map(|row| {
            let content = row.get::<String, _>("content");
            let score = cosine_similarity(&query_embedding, &embed_text_for_semantic(&content));
            let mut value = extract_message_json(row);
            if let Some(obj) = value.as_object_mut() {
                obj.insert("similarity".to_string(), serde_json::json!(score));
            }
            (score, value)
        })
        .filter(|(score, _)| *score >= threshold)
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    ranked.truncate(limit as usize);

    let items = ranked.into_iter().map(|(_, item)| item).collect::<Vec<_>>();
    Ok(serde_json::json!({
        "queryRewritten": rewritten_query,
        "threshold": threshold,
        "count": items.len() as i64,
        "items": items
    }))
}

fn summarize_tool_result(result: &serde_json::Value) -> serde_json::Value {
    if let Some(count) = result.get("count").and_then(|v| v.as_i64()) {
        return serde_json::json!({
            "count": count,
            "keys": result.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()
        });
    }
    if let Some(total_messages) = result.get("totalMessages").and_then(|v| v.as_i64()) {
        return serde_json::json!({
            "totalMessages": total_messages,
            "keys": result.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()
        });
    }
    serde_json::json!({
        "keys": result.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()
    })
}

fn build_final_answer(
    prompt: &str,
    tools_used: &[String],
    executions: &[ToolExecution],
    locale: &str,
) -> String {
    let mut lines = Vec::new();
    if locale.starts_with("zh") {
        lines.push(format!("问题：{}", prompt));
        lines.push(format!(
            "已执行工具（{}）：{}",
            tools_used.len(),
            tools_used.join(", ")
        ));
        for exe in executions {
            let count = exe
                .result
                .get("count")
                .and_then(|v| v.as_i64())
                .or_else(|| exe.result.get("historyCount").and_then(|v| v.as_i64()))
                .unwrap_or(0);
            lines.push(format!("- {}: 返回 {} 条结果", exe.name, count));
        }
    } else {
        lines.push(format!("Question: {}", prompt));
        lines.push(format!(
            "Executed tools ({}): {}",
            tools_used.len(),
            tools_used.join(", ")
        ));
        for exe in executions {
            let count = exe
                .result
                .get("count")
                .and_then(|v| v.as_i64())
                .or_else(|| exe.result.get("historyCount").and_then(|v| v.as_i64()))
                .unwrap_or(0);
            lines.push(format!("- {}: {} rows returned", exe.name, count));
        }
    }
    lines.join("\n")
}

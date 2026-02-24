//! Events API module for Xenobot HTTP API.
//!
//! Provides Server-Sent Events (SSE) endpoints equivalent to Xenobot's IPC event listeners.
//! This allows the frontend to subscribe to real-time updates (import progress, LLM streaming, etc.).

use axum::{
    extract::Path,
    response::{sse::Event, Sse},
    routing::get,
    Router,
};
use futures::stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;
use tracing::instrument;

/// Events API router.
pub fn router() -> Router {
    Router::new()
        .route("/import-progress", get(import_progress_sse))
        .route("/export-progress", get(export_progress_sse))
        .route("/llm-stream/:request_id", get(llm_stream_sse))
        .route("/agent-stream/:request_id", get(agent_stream_sse))
        .route("/agent-complete/:request_id", get(agent_complete_sse))
        .route("/merge-parse-progress", get(merge_parse_progress_sse))
}

// Request/Response types

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgressEvent {
    pub total: u64,
    pub processed: u64,
    pub current_file: Option<String>,
    pub status: String, // "parsing", "importing", "complete", "error"
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgressEvent {
    pub total: u64,
    pub processed: u64,
    pub current_chunk: Option<String>,
    pub status: String, // "processing", "writing", "complete", "error"
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmStreamChunkEvent {
    pub request_id: String,
    pub chunk: String,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStreamChunkEvent {
    pub request_id: String,
    pub chunk: String,
    pub chunk_type: String, // "thought", "action", "result", "final"
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCompleteEvent {
    pub request_id: String,
    pub result: serde_json::Value,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeParseProgressEvent {
    pub total_files: u64,
    pub parsed_files: u64,
    pub current_file: Option<String>,
    pub status: String,
}

// Handler functions

#[axum::debug_handler]
#[instrument]
pub async fn import_progress_sse() -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    // Placeholder: return empty stream that never emits
    let stream = stream::empty::<Result<Event, Infallible>>();
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

#[axum::debug_handler]
#[instrument]
pub async fn export_progress_sse() -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::empty::<Result<Event, Infallible>>();
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

#[axum::debug_handler]
#[instrument]
pub async fn llm_stream_sse(
    Path(request_id): Path<String>,
) -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::empty::<Result<Event, Infallible>>();
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

#[axum::debug_handler]
#[instrument]
pub async fn agent_stream_sse(
    Path(request_id): Path<String>,
) -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::empty::<Result<Event, Infallible>>();
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

#[axum::debug_handler]
#[instrument]
pub async fn agent_complete_sse(
    Path(request_id): Path<String>,
) -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = stream::empty::<Result<Event, Infallible>>();
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

#[axum::debug_handler]
#[instrument]
pub async fn merge_parse_progress_sse() -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>>
{
    let stream = stream::empty::<Result<Event, Infallible>>();
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

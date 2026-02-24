//! Frontend integration utilities.
//!
//! This module provides adapters between the preserved Vue 3 frontend
//! and the Rust backend API/WebSocket services.

use std::sync::Arc;

use crate::error::WebResult;
use crate::websocket::{WebSocketMessage, WebSocketState};

/// Frontend integration service.
#[derive(Debug, Clone)]
pub struct FrontendIntegration {
    /// WebSocket state for real-time updates.
    pub ws_state: Arc<WebSocketState>,
    /// API base URL (for HTTP requests).
    pub api_base_url: String,
}

impl FrontendIntegration {
    /// Create a new frontend integration service.
    pub fn new(ws_state: Arc<WebSocketState>, api_base_url: impl Into<String>) -> Self {
        Self {
            ws_state,
            api_base_url: api_base_url.into(),
        }
    }

    /// Notify frontend of a WeChat message arrival.
    pub async fn notify_wechat_message(
        &self,
        chat: String,
        sender: String,
        content: String,
        timestamp: i64,
    ) -> WebResult<()> {
        let message = WebSocketMessage::WeChatMessage {
            chat,
            sender,
            content,
            timestamp,
        };
        self.ws_state.broadcast(message).await
    }

    /// Notify frontend of analysis progress.
    pub async fn notify_analysis_progress(
        &self,
        task_id: String,
        progress: f32,
        message: String,
    ) -> WebResult<()> {
        let msg = WebSocketMessage::AnalysisProgress {
            task_id,
            progress,
            message,
        };
        self.ws_state.broadcast(msg).await
    }

    /// Send a system notification to frontend.
    pub async fn send_system_notification(
        &self,
        title: String,
        message: String,
        level: String,
    ) -> WebResult<()> {
        let msg = WebSocketMessage::SystemNotification {
            title,
            message,
            level,
        };
        self.ws_state.broadcast(msg).await
    }

    /// Get API endpoint URL for a given path.
    pub fn api_url(&self, path: &str) -> String {
        format!("{}{}", self.api_base_url, path)
    }
}

/// IPC to HTTP adapter (placeholder).
/// In the original Electron app, the frontend uses `window.electronAPI` or `ipcRenderer`.
/// We will replace those calls with HTTP requests to the Rust backend.
/// This module provides a TypeScript/JavaScript shim that can be injected into the frontend.
/// For now, we just document the mapping.
pub mod ipc_adapter {
    //! Mapping of IPC channels to HTTP endpoints.
    //!
    //! Example mapping:
    //! - `ipcRenderer.invoke('get-chats')` → GET /api/chats
    //! - `ipcRenderer.invoke('merge-chats', args)` → POST /api/merge
    //! - `ipcRenderer.send('start-analysis')` → POST /api/analysis/start
    //! - Real-time events via WebSocket instead of `ipcRenderer.on`
}

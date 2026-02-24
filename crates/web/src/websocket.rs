//! WebSocket server for real-time updates.

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Extension,
    },
    response::IntoResponse,
};
use futures::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::info;

use crate::error::WebResult;

/// Shared state for WebSocket connections.
#[derive(Debug, Clone)]
pub struct WebSocketState {
    /// Broadcast sender for sending messages to all connected clients.
    pub tx: broadcast::Sender<WebSocketMessage>,
    /// Connected client count.
    pub client_count: Arc<RwLock<u32>>,
}

/// Message types for WebSocket communication.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WebSocketMessage {
    /// WeChat message received.
    WeChatMessage {
        chat: String,
        sender: String,
        content: String,
        timestamp: i64,
    },
    /// Analysis progress update.
    AnalysisProgress {
        task_id: String,
        progress: f32,
        message: String,
    },
    /// System notification.
    SystemNotification {
        title: String,
        message: String,
        level: String,
    },
    /// Ping/Pong heartbeat.
    Ping,
    Pong,
}

impl WebSocketState {
    /// Create new WebSocket state.
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            tx,
            client_count: Arc::new(RwLock::new(0)),
        }
    }

    /// Send a message to all connected clients.
    pub async fn broadcast(&self, message: WebSocketMessage) -> WebResult<()> {
        let _ = self.tx.send(message);
        Ok(())
    }

    /// Increment client count.
    pub async fn increment_client_count(&self) {
        let mut count = self.client_count.write().await;
        *count += 1;
        info!("WebSocket client connected. Total clients: {}", count);
    }

    /// Decrement client count.
    pub async fn decrement_client_count(&self) {
        let mut count = self.client_count.write().await;
        *count = count.saturating_sub(1);
        info!("WebSocket client disconnected. Total clients: {}", count);
    }
}

/// WebSocket upgrade handler.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(state): Extension<Arc<WebSocketState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle individual WebSocket connection.
async fn handle_socket(socket: WebSocket, state: Arc<WebSocketState>) {
    state.increment_client_count().await;

    let mut rx = state.tx.subscribe();
    let (sender, mut receiver): (SplitSink<WebSocket, Message>, SplitStream<WebSocket>) =
        socket.split();

    // Clone sender for the send task
    let sender = Arc::new(tokio::sync::Mutex::new(sender));
    let sender_clone = Arc::clone(&sender);

    // Spawn task to receive messages from client.
    let recv_task = tokio::spawn(async move {
        let mut sender = sender_clone.lock().await;
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    // Handle incoming messages from client
                    if let Ok(message) = serde_json::from_str::<WebSocketMessage>(&text) {
                        match message {
                            WebSocketMessage::Ping => {
                                // Respond with pong
                                let _ = sender
                                    .send(Message::Text(
                                        serde_json::to_string(&WebSocketMessage::Pong).unwrap(),
                                    ))
                                    .await;
                            }
                            _ => {
                                // Forward to broadcast? For now, just log
                                info!("Received WebSocket message: {:?}", message);
                            }
                        }
                    }
                }
                Message::Close(_) => {
                    break;
                }
                _ => {}
            }
        }
    });

    // Spawn task to send messages to client.
    let send_task = tokio::spawn(async move {
        let mut sender = sender.lock().await;
        while let Ok(message) = rx.recv().await {
            let text = serde_json::to_string(&message).unwrap();
            if sender.send(Message::Text(text)).await.is_err() {
                break;
            }
        }
    });

    // Wait for either task to complete.
    tokio::select! {
        _ = recv_task => {},
        _ = send_task => {},
    }

    state.decrement_client_count().await;
}

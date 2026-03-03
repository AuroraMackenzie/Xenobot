//! MCP server implementation.

use crate::config::McpServerConfig;
use crate::error::{McpError, Result};
use crate::protocol::{
    ClientCapabilities, ClientInfo, Content, InitializeRequest, InitializeResponse, McpMessage,
    Notification, NotificationLevel, ResourceSubscribeRequest, ResourceUpdate, ServerCapabilities,
    ServerInfo, ToolCallRequest, ToolCallResult, ToolResult,
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        ConnectInfo, Path, State,
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Sse},
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_stream;
use tracing::{debug, error, info};

/// MCP server state.
#[derive(Clone)]
pub struct McpServer {
    /// Server configuration.
    config: McpServerConfig,
    /// Connected clients.
    clients: Arc<RwLock<HashMap<String, ClientState>>>,
    /// Available tools.
    tools: Arc<RwLock<HashMap<String, ToolHandler>>>,
    /// Available resources.
    resources: Arc<RwLock<HashMap<String, Resource>>>,
}

/// Client connection state.
#[allow(dead_code)]
#[derive(Debug)]
struct ClientState {
    /// Client ID.
    id: String,
    /// Client information.
    info: Option<ClientInfo>,
    /// Client capabilities.
    capabilities: ClientCapabilities,
    /// WebSocket sender (if using WebSocket).
    ws_sender: Option<mpsc::UnboundedSender<Message>>,
    /// Subscribed resources.
    subscribed_resources: Vec<String>,
    /// Pending tool calls.
    pending_tool_calls: HashMap<String, ToolCallRequest>,
}

/// Tool handler function.
type ToolHandler = Box<dyn Fn(Value) -> Result<Value> + Send + Sync>;

/// Resource representation.
#[derive(Debug, Clone)]
struct Resource {
    /// Resource URI.
    uri: String,
    /// Resource content.
    content: Vec<Content>,
    /// Resource MIME type.
    mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct IntegrationCatalogItem {
    id: String,
    name: String,
    description: String,
}

#[derive(Debug, Clone, Serialize)]
struct IntegrationPreset {
    id: String,
    name: String,
    description: String,
    transport: serde_json::Value,
    configuration: serde_json::Value,
    notes: Vec<String>,
}

impl McpServer {
    /// Create a new MCP server with the given configuration.
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            clients: Arc::new(RwLock::new(HashMap::new())),
            tools: Arc::new(RwLock::new(HashMap::new())),
            resources: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a tool handler.
    pub async fn register_tool(&self, name: String, handler: ToolHandler) -> Result<()> {
        let mut tools = self.tools.write().await;
        tools.insert(name, handler);
        Ok(())
    }

    /// Register built-in MCP tools for local chat querying.
    async fn register_builtin_tools(&self) -> Result<()> {
        self.register_tool(
            "current_time".to_string(),
            Box::new(|_args| {
                let now = chrono::Utc::now();
                Ok(serde_json::json!({
                    "unix": now.timestamp(),
                    "iso8601": now.to_rfc3339(),
                    "timezone": "UTC",
                }))
            }),
        )
        .await?;
        self.register_tool(
            "get_current_time".to_string(),
            Box::new(|args| {
                // Alias for clients that use verb-style tool naming.
                let _ = args;
                let now = chrono::Utc::now();
                Ok(serde_json::json!({
                    "unix": now.timestamp(),
                    "iso8601": now.to_rfc3339(),
                    "timezone": "UTC",
                }))
            }),
        )
        .await?;

        self.register_tool("list_sessions".to_string(), Box::new(list_sessions_tool))
            .await?;

        self.register_tool("list_contacts".to_string(), Box::new(list_contacts_tool))
            .await?;

        self.register_tool(
            "recent_messages".to_string(),
            Box::new(recent_messages_tool),
        )
        .await?;

        self.register_tool(
            "search_messages".to_string(),
            Box::new(search_messages_tool),
        )
        .await?;

        // MCP first-batch tool aliases aligned with 10.2 contract names.
        self.register_tool("query_contacts".to_string(), Box::new(list_contacts_tool))
            .await?;
        self.register_tool("recent_sessions".to_string(), Box::new(list_sessions_tool))
            .await?;
        self.register_tool("query_chats".to_string(), Box::new(list_sessions_tool))
            .await?;
        self.register_tool("query_groups".to_string(), Box::new(query_groups_tool))
            .await?;
        self.register_tool("chat_records".to_string(), Box::new(chat_records_tool))
            .await?;
        self.register_tool("chat_history".to_string(), Box::new(chat_records_tool))
            .await?;

        Ok(())
    }

    /// Start the MCP server.
    pub async fn start(self) -> Result<()> {
        self.register_builtin_tools().await?;
        let app = self.create_router();

        let addr = format!("{}:{}", self.config.bind_address, self.config.port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| McpError::Network(format!("Failed to bind to {}: {}", addr, e)))?;

        info!("MCP server listening on {}", addr);

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .map_err(|e| McpError::Network(format!("Server error: {}", e)))?;

        Ok(())
    }

    /// Create the Axum router for the server.
    fn create_router(&self) -> Router {
        let state = Arc::new(self.clone());

        Router::new()
            // WebSocket endpoint
            .route("/ws", get(handle_websocket))
            // SSE endpoint
            .route("/sse", get(handle_sse))
            // Streamable HTTP JSON-RPC endpoint
            .route("/mcp", post(handle_streamable_http_rpc))
            // HTTP endpoint for tool calls
            .route("/tools/:tool_name", post(handle_http_tool_call))
            // HTTP endpoint for listing available tools
            .route("/tools", get(handle_http_tools_list))
            // HTTP endpoint for resources
            .route("/resources/*uri", get(handle_http_resource))
            // Integration catalog for MCP desktop clients
            .route("/integrations", get(handle_integrations_list))
            .route("/integrations/:target", get(handle_integration_preset))
            // Health check
            .route("/health", get(|| async { "OK" }))
            .with_state(state)
    }

    /// Handle WebSocket connection.
    async fn handle_websocket_connection(
        self: Arc<Self>,
        ws: WebSocket,
        client_id: String,
        client_info: Option<ClientInfo>,
        capabilities: ClientCapabilities,
    ) {
        let (mut ws_sender, mut ws_receiver) = ws.split();

        // Create channel for sending messages to this client
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Store client state
        let client_state = ClientState {
            id: client_id.clone(),
            info: client_info,
            capabilities,
            ws_sender: Some(tx.clone()),
            subscribed_resources: Vec::new(),
            pending_tool_calls: HashMap::new(),
        };

        {
            let mut clients = self.clients.write().await;
            clients.insert(client_id.clone(), client_state);
        }

        info!("Client {} connected via WebSocket", client_id);

        // Spawn task to send messages to WebSocket
        let send_task = tokio::spawn({
            let client_id = client_id.clone();
            let server = self.clone();
            async move {
                while let Some(message) = rx.recv().await {
                    if let Err(e) = ws_sender.send(message).await {
                        error!("Failed to send message to client {}: {}", client_id, e);
                        break;
                    }
                }
                let mut clients = server.clients.write().await;
                clients.remove(&client_id);
                info!("Client {} disconnected", client_id);
            }
        });

        // Spawn task to receive messages from WebSocket
        let recv_task = tokio::spawn({
            let server = self.clone();
            let client_id = client_id.clone();
            async move {
                while let Some(Ok(message)) = ws_receiver.next().await {
                    match message {
                        Message::Text(text) => {
                            if let Err(e) = server.handle_client_message(&client_id, &text).await {
                                error!("Error handling message from client {}: {}", client_id, e);
                            }
                        }
                        Message::Close(_) => {
                            break;
                        }
                        _ => {
                            // Ignore other message types
                        }
                    }
                }
                // Signal send task to stop
                drop(tx);
            }
        });

        // Wait for either task to complete
        tokio::select! {
            _ = send_task => {},
            _ = recv_task => {},
        }
    }

    /// Handle client message.
    async fn handle_client_message(&self, client_id: &str, text: &str) -> Result<()> {
        let message: McpMessage = serde_json::from_str(text)
            .map_err(|e| McpError::Serialization(format!("Invalid message: {}", e)))?;

        match message {
            McpMessage::InitializeRequest(req) => {
                self.handle_initialize(client_id, req).await?;
            }
            McpMessage::ToolCallRequest(req) => {
                self.handle_tool_call(client_id, req).await?;
            }
            McpMessage::ResourceSubscribeRequest(req) => {
                self.handle_resource_subscribe(client_id, req).await?;
            }
            _ => {
                return Err(McpError::Protocol(format!(
                    "Unsupported message type from client {}",
                    client_id
                )));
            }
        }

        Ok(())
    }

    /// Handle initialization request.
    async fn handle_initialize(&self, client_id: &str, request: InitializeRequest) -> Result<()> {
        debug!(
            "Client {} initialized with version {}",
            client_id, request.protocol_version
        );

        let response = InitializeResponse {
            protocol_version: crate::protocol::MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ServerCapabilities::default(),
            server_info: Some(ServerInfo {
                name: self.config.name.clone(),
                version: self.config.version.clone(),
            }),
            instructions: Some("Welcome to Xenobot MCP Server".to_string()),
        };

        self.send_message(client_id, McpMessage::InitializeResponse(response))
            .await
    }

    /// Handle tool call request.
    async fn handle_tool_call(&self, client_id: &str, request: ToolCallRequest) -> Result<()> {
        debug!("Client {} called tool {}", client_id, request.name);

        let tools = self.tools.read().await;
        let handler = tools.get(&request.name);

        let result = match handler {
            Some(handler) => {
                let arguments = request.arguments.unwrap_or_default();
                match handler(arguments) {
                    Ok(result) => ToolResult::Success {
                        content: vec![Content::Text {
                            text: serde_json::to_string(&result).unwrap_or_default(),
                        }],
                        metadata: None,
                    },
                    Err(e) => ToolResult::Error {
                        code: "TOOL_ERROR".to_string(),
                        message: e.to_string(),
                        details: None,
                    },
                }
            }
            None => ToolResult::Error {
                code: "TOOL_NOT_FOUND".to_string(),
                message: format!("Tool '{}' not found", request.name),
                details: None,
            },
        };

        let response = ToolCallResult {
            call_id: request.id,
            result,
        };

        self.send_message(client_id, McpMessage::ToolCallResult(response))
            .await
    }

    /// Handle resource subscription request.
    async fn handle_resource_subscribe(
        &self,
        client_id: &str,
        request: ResourceSubscribeRequest,
    ) -> Result<()> {
        debug!(
            "Client {} subscribed to resource {}",
            client_id, request.uri
        );

        let resources = self.resources.read().await;
        let resource = resources.get(&request.uri);

        if let Some(resource) = resource {
            let update = ResourceUpdate {
                uri: resource.uri.clone(),
                content: resource.content.clone(),
            };

            self.send_message(client_id, McpMessage::ResourceUpdate(update))
                .await?;
        }

        // Store subscription
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(client_id) {
            client.subscribed_resources.push(request.uri);
        }

        Ok(())
    }

    /// Send a message to a client.
    async fn send_message(&self, client_id: &str, message: McpMessage) -> Result<()> {
        let clients = self.clients.read().await;
        let client = clients.get(client_id);

        if let Some(client) = client {
            if let Some(sender) = &client.ws_sender {
                let json = serde_json::to_string(&message).map_err(|e| {
                    McpError::Serialization(format!("Failed to serialize message: {}", e))
                })?;

                sender
                    .send(Message::Text(json))
                    .map_err(|e| McpError::Network(format!("Failed to send message: {}", e)))?;
            }
        }

        Ok(())
    }

    /// Send a notification to all clients.
    pub async fn broadcast_notification(
        &self,
        level: NotificationLevel,
        message: String,
    ) -> Result<()> {
        let notification = Notification {
            level,
            message,
            data: None,
        };
        let msg = McpMessage::Notification(notification);

        let clients = self.clients.read().await;
        for client in clients.values() {
            if let Some(sender) = &client.ws_sender {
                let json = serde_json::to_string(&msg).map_err(|e| {
                    McpError::Serialization(format!("Failed to serialize message: {}", e))
                })?;

                let _ = sender.send(Message::Text(json));
            }
        }

        Ok(())
    }
}

fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot")
        .join("xenobot.db")
}

fn resolve_db_path() -> PathBuf {
    std::env::var("XENOBOT_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_db_path())
}

fn open_xenobot_db() -> Result<Connection> {
    let db_path = resolve_db_path();
    Connection::open(&db_path)
        .map_err(|e| McpError::Tool(format!("open database {:?} failed: {e}", db_path)))
}

fn list_sessions_tool(args: Value) -> Result<Value> {
    let limit = parse_limit(&args, 20, 200)?;
    let conn = open_xenobot_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, platform, chat_type, imported_at
             FROM meta
             ORDER BY imported_at DESC, id DESC
             LIMIT ?1",
        )
        .map_err(|e| McpError::Tool(format!("prepare list_sessions failed: {e}")))?;
    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "platform": row.get::<_, String>(2)?,
                "chatType": row.get::<_, String>(3)?,
                "importedAt": row.get::<_, i64>(4)?,
            }))
        })
        .map_err(|e| McpError::Tool(format!("query list_sessions failed: {e}")))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| McpError::Tool(format!("read list_sessions row failed: {e}")))?);
    }
    Ok(serde_json::json!({
        "count": items.len(),
        "sessions": items,
    }))
}

fn list_contacts_tool(args: Value) -> Result<Value> {
    let limit = parse_limit(&args, 50, 500)?;
    let session_id = parse_optional_i64(&args, "session_id");
    let conn = open_xenobot_db()?;

    let mut items = Vec::new();
    if let Some(meta_id) = session_id {
        let mut stmt = conn
            .prepare(
                "SELECT
                    m.id,
                    m.platform_id,
                    m.account_name,
                    m.group_nickname,
                    m.avatar,
                    COUNT(*) as message_count
                 FROM member m
                 JOIN message msg ON msg.sender_id = m.id
                 WHERE msg.meta_id = ?1
                 GROUP BY m.id, m.platform_id, m.account_name, m.group_nickname, m.avatar
                 ORDER BY message_count DESC, m.id ASC
                 LIMIT ?2",
            )
            .map_err(|e| McpError::Tool(format!("prepare list_contacts failed: {e}")))?;
        let rows = stmt
            .query_map(params![meta_id, limit], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "platformId": row.get::<_, String>(1)?,
                    "accountName": row.get::<_, Option<String>>(2)?,
                    "groupNickname": row.get::<_, Option<String>>(3)?,
                    "avatar": row.get::<_, Option<String>>(4)?,
                    "messageCount": row.get::<_, i64>(5)?,
                }))
            })
            .map_err(|e| McpError::Tool(format!("query list_contacts failed: {e}")))?;
        for row in rows {
            items.push(
                row.map_err(|e| McpError::Tool(format!("read list_contacts row failed: {e}")))?,
            );
        }
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, platform_id, account_name, group_nickname, avatar
                 FROM member
                 ORDER BY id DESC
                 LIMIT ?1",
            )
            .map_err(|e| McpError::Tool(format!("prepare list_contacts failed: {e}")))?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "platformId": row.get::<_, String>(1)?,
                    "accountName": row.get::<_, Option<String>>(2)?,
                    "groupNickname": row.get::<_, Option<String>>(3)?,
                    "avatar": row.get::<_, Option<String>>(4)?,
                }))
            })
            .map_err(|e| McpError::Tool(format!("query list_contacts failed: {e}")))?;
        for row in rows {
            items.push(
                row.map_err(|e| McpError::Tool(format!("read list_contacts row failed: {e}")))?,
            );
        }
    }

    Ok(serde_json::json!({
        "count": items.len(),
        "contacts": items,
    }))
}

fn query_groups_tool(args: Value) -> Result<Value> {
    let limit = parse_limit(&args, 20, 200)?;
    let conn = open_xenobot_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, platform, chat_type, imported_at
             FROM meta
             WHERE chat_type = 'group'
             ORDER BY imported_at DESC, id DESC
             LIMIT ?1",
        )
        .map_err(|e| McpError::Tool(format!("prepare query_groups failed: {e}")))?;
    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "platform": row.get::<_, String>(2)?,
                "chatType": row.get::<_, String>(3)?,
                "importedAt": row.get::<_, i64>(4)?,
            }))
        })
        .map_err(|e| McpError::Tool(format!("query query_groups failed: {e}")))?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| McpError::Tool(format!("read query_groups row failed: {e}")))?);
    }
    Ok(serde_json::json!({
        "count": items.len(),
        "groups": items
    }))
}

fn recent_messages_tool(args: Value) -> Result<Value> {
    let meta_id = parse_required_i64(&args, "session_id")?;
    let limit = parse_limit(&args, 50, 500)?;
    let conn = open_xenobot_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT
                msg.id,
                msg.ts,
                msg.msg_type,
                COALESCE(msg.content, ''),
                COALESCE(msg.sender_group_nickname, msg.sender_account_name, m.group_nickname, m.account_name, m.platform_id, '') as sender_name,
                COALESCE(m.platform_id, '')
             FROM message msg
             LEFT JOIN member m ON m.id = msg.sender_id
             WHERE msg.meta_id = ?1
             ORDER BY msg.ts DESC, msg.id DESC
             LIMIT ?2",
        )
        .map_err(|e| McpError::Tool(format!("prepare recent_messages failed: {e}")))?;
    let rows = stmt
        .query_map(params![meta_id, limit], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "timestamp": row.get::<_, i64>(1)?,
                "msgType": row.get::<_, i64>(2)?,
                "content": row.get::<_, String>(3)?,
                "senderName": row.get::<_, String>(4)?,
                "senderPlatformId": row.get::<_, String>(5)?,
            }))
        })
        .map_err(|e| McpError::Tool(format!("query recent_messages failed: {e}")))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(
            row.map_err(|e| McpError::Tool(format!("read recent_messages row failed: {e}")))?,
        );
    }
    Ok(serde_json::json!({
        "count": items.len(),
        "messages": items,
    }))
}

fn search_messages_tool(args: Value) -> Result<Value> {
    let meta_id = parse_required_i64(&args, "session_id")?;
    let keyword = parse_keyword(&args)?;
    let limit = parse_limit(&args, 50, 500)?;
    let conn = open_xenobot_db()?;
    let pattern = format!("%{}%", keyword);
    let mut stmt = conn
        .prepare(
            "SELECT
                msg.id,
                msg.ts,
                msg.msg_type,
                COALESCE(msg.content, ''),
                COALESCE(msg.sender_group_nickname, msg.sender_account_name, m.group_nickname, m.account_name, m.platform_id, '') as sender_name,
                COALESCE(m.platform_id, '')
             FROM message msg
             LEFT JOIN member m ON m.id = msg.sender_id
             WHERE msg.meta_id = ?1
               AND COALESCE(msg.content, '') LIKE ?2
             ORDER BY msg.ts DESC, msg.id DESC
             LIMIT ?3",
        )
        .map_err(|e| McpError::Tool(format!("prepare search_messages failed: {e}")))?;
    let rows = stmt
        .query_map(params![meta_id, pattern, limit], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "timestamp": row.get::<_, i64>(1)?,
                "msgType": row.get::<_, i64>(2)?,
                "content": row.get::<_, String>(3)?,
                "senderName": row.get::<_, String>(4)?,
                "senderPlatformId": row.get::<_, String>(5)?,
            }))
        })
        .map_err(|e| McpError::Tool(format!("query search_messages failed: {e}")))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(
            row.map_err(|e| McpError::Tool(format!("read search_messages row failed: {e}")))?,
        );
    }
    Ok(serde_json::json!({
        "keyword": keyword,
        "count": items.len(),
        "messages": items,
    }))
}

fn chat_records_tool(args: Value) -> Result<Value> {
    let meta_id = parse_required_i64(&args, "session_id")?;
    let limit = parse_limit(&args, 50, 500)?;
    let offset = parse_offset(&args)?;
    let start_ts = parse_optional_i64(&args, "start_ts");
    let end_ts = parse_optional_i64(&args, "end_ts");
    let keyword = parse_optional_keyword(&args);
    let pattern = keyword.as_ref().map(|kw| format!("%{}%", kw));
    let conn = open_xenobot_db()?;

    let total_count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM message msg
             WHERE msg.meta_id = ?1
               AND (?2 IS NULL OR msg.ts >= ?2)
               AND (?3 IS NULL OR msg.ts <= ?3)
               AND (?4 IS NULL OR COALESCE(msg.content, '') LIKE ?4)",
            params![meta_id, start_ts, end_ts, pattern],
            |row| row.get(0),
        )
        .map_err(|e| McpError::Tool(format!("count chat_records failed: {e}")))?;

    let mut stmt = conn
        .prepare(
            "SELECT
                msg.id,
                msg.ts,
                msg.msg_type,
                COALESCE(msg.content, ''),
                COALESCE(msg.sender_group_nickname, msg.sender_account_name, m.group_nickname, m.account_name, m.platform_id, '') as sender_name,
                COALESCE(m.platform_id, '')
             FROM message msg
             LEFT JOIN member m ON m.id = msg.sender_id
             WHERE msg.meta_id = ?1
               AND (?2 IS NULL OR msg.ts >= ?2)
               AND (?3 IS NULL OR msg.ts <= ?3)
               AND (?4 IS NULL OR COALESCE(msg.content, '') LIKE ?4)
             ORDER BY msg.ts DESC, msg.id DESC
             LIMIT ?5 OFFSET ?6",
        )
        .map_err(|e| McpError::Tool(format!("prepare chat_records failed: {e}")))?;
    let rows = stmt
        .query_map(
            params![meta_id, start_ts, end_ts, pattern, limit, offset],
            |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "timestamp": row.get::<_, i64>(1)?,
                    "msgType": row.get::<_, i64>(2)?,
                    "content": row.get::<_, String>(3)?,
                    "senderName": row.get::<_, String>(4)?,
                    "senderPlatformId": row.get::<_, String>(5)?,
                }))
            },
        )
        .map_err(|e| McpError::Tool(format!("query chat_records failed: {e}")))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| McpError::Tool(format!("read chat_records row failed: {e}")))?);
    }

    Ok(serde_json::json!({
        "sessionId": meta_id,
        "keyword": keyword,
        "startTs": start_ts,
        "endTs": end_ts,
        "offset": offset,
        "limit": limit,
        "count": items.len(),
        "totalCount": total_count,
        "hasMore": offset + limit < total_count,
        "messages": items
    }))
}

fn parse_limit(args: &Value, default_limit: i64, max_limit: i64) -> Result<i64> {
    let limit = parse_optional_i64(args, "limit").unwrap_or(default_limit);
    Ok(limit.max(1).min(max_limit))
}

fn parse_offset(args: &Value) -> Result<i64> {
    let offset = parse_optional_i64(args, "offset").unwrap_or(0);
    Ok(offset.max(0))
}

fn parse_required_i64(args: &Value, key: &str) -> Result<i64> {
    parse_optional_i64(args, key)
        .ok_or_else(|| McpError::Argument(format!("missing required integer field: {key}")))
}

fn parse_optional_i64(args: &Value, key: &str) -> Option<i64> {
    arg_value(args, key).and_then(|v| v.as_i64())
}

fn parse_optional_keyword(args: &Value) -> Option<String> {
    arg_value(args, "keyword")
        .and_then(|v| v.as_str())
        .or_else(|| arg_value(args, "query").and_then(|v| v.as_str()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn parse_keyword(args: &Value) -> Result<String> {
    if let Some(keyword) = parse_optional_keyword(args) {
        return Ok(keyword);
    }

    if let Some(keyword) = arg_value(args, "keywords")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        return Ok(keyword);
    }

    Err(McpError::Argument(
        "missing search keyword (keyword or keywords[0])".to_string(),
    ))
}

fn arg_value<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key)
        .or_else(|| args.get(&snake_to_camel(key)))
        .or_else(|| args.get(&camel_to_snake(key)))
}

fn snake_to_camel(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut uppercase_next = false;
    for ch in key.chars() {
        if ch == '_' {
            uppercase_next = true;
            continue;
        }
        if uppercase_next {
            out.extend(ch.to_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn camel_to_snake(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    for (idx, ch) in key.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if idx > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn public_host(bind_address: &str) -> &str {
    match bind_address {
        "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
        other => other,
    }
}

fn build_http_base_url(config: &McpServerConfig) -> String {
    format!(
        "http://{}:{}",
        public_host(&config.bind_address),
        config.port
    )
}

fn integration_catalog() -> Vec<IntegrationCatalogItem> {
    vec![
        IntegrationCatalogItem {
            id: "claude-desktop".to_string(),
            name: "Claude Desktop".to_string(),
            description: "Configuration snippet for Claude Desktop MCP integration".to_string(),
        },
        IntegrationCatalogItem {
            id: "chatwise".to_string(),
            name: "ChatWise".to_string(),
            description: "Configuration snippet for ChatWise MCP integration".to_string(),
        },
        IntegrationCatalogItem {
            id: "opencode".to_string(),
            name: "Opencode".to_string(),
            description: "Configuration snippet for Opencode MCP integration".to_string(),
        },
    ]
}

fn build_integration_preset(config: &McpServerConfig, target: &str) -> Option<IntegrationPreset> {
    let normalized = target.trim().to_ascii_lowercase();
    let base_url = build_http_base_url(config);
    let sse_url = format!("{}/sse", base_url);
    let ws_url = format!("{}/ws", base_url);
    let tools_url = format!("{}/tools", base_url);

    match normalized.as_str() {
        "claude-desktop" | "claude_desktop" | "claude" => Some(IntegrationPreset {
            id: "claude-desktop".to_string(),
            name: "Claude Desktop".to_string(),
            description: "Use mcp-remote to bridge Claude Desktop to Xenobot over SSE".to_string(),
            transport: serde_json::json!({
                "sse": sse_url,
                "websocket": ws_url,
                "tools": tools_url,
            }),
            configuration: serde_json::json!({
                "mcpServers": {
                    "xenobot": {
                        "command": "pnpm",
                        "args": ["dlx", "mcp-remote", sse_url]
                    }
                }
            }),
            notes: vec![
                "Install Node.js before using the mcp-remote bridge.".to_string(),
                "If your MCP server runs on another host, replace localhost with that host.".to_string(),
                "Restart Claude Desktop after editing its MCP configuration file.".to_string(),
            ],
        }),
        "chatwise" => Some(IntegrationPreset {
            id: "chatwise".to_string(),
            name: "ChatWise".to_string(),
            description: "ChatWise server configuration for Xenobot MCP over SSE".to_string(),
            transport: serde_json::json!({
                "sse": sse_url,
                "websocket": ws_url,
                "tools": tools_url,
            }),
            configuration: serde_json::json!({
                "servers": [
                    {
                        "name": "xenobot",
                        "transport": "sse",
                        "url": sse_url
                    }
                ]
            }),
            notes: vec![
                "The exact UI field names can vary by ChatWise release; map these values to the MCP server form.".to_string(),
                "Use the same URL base for tool discovery and streaming transport.".to_string(),
            ],
        }),
        "opencode" => Some(IntegrationPreset {
            id: "opencode".to_string(),
            name: "Opencode".to_string(),
            description: "Opencode MCP configuration using Xenobot SSE endpoint".to_string(),
            transport: serde_json::json!({
                "sse": sse_url,
                "websocket": ws_url,
                "tools": tools_url,
            }),
            configuration: serde_json::json!({
                "mcpServers": [
                    {
                        "name": "xenobot",
                        "transport": "sse",
                        "url": sse_url
                    }
                ]
            }),
            notes: vec![
                "Map this preset to the MCP server section in your Opencode settings.".to_string(),
                "If Opencode supports WebSocket MCP in your version, you can also use the ws URL.".to_string(),
            ],
        }),
        _ => None,
    }
}

/// WebSocket handler.
async fn handle_websocket(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    State(server): State<Arc<McpServer>>,
) -> impl IntoResponse {
    let client_id = format!("client-{}", addr);
    let client_info = extract_client_info(&headers);
    let capabilities = ClientCapabilities::default();

    ws.on_upgrade(move |socket| {
        server.handle_websocket_connection(socket, client_id, client_info, capabilities)
    })
}

/// HTTP tool list handler.
async fn handle_http_tools_list(State(server): State<Arc<McpServer>>) -> impl IntoResponse {
    let tools = server.tools.read().await;
    let mut names: Vec<String> = tools.keys().cloned().collect();
    names.sort();
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "count": names.len(),
            "tools": names,
        })),
    )
        .into_response()
}

async fn handle_integrations_list(State(_server): State<Arc<McpServer>>) -> impl IntoResponse {
    let catalog = integration_catalog();
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "count": catalog.len(),
            "integrations": catalog,
        })),
    )
        .into_response()
}

async fn handle_integration_preset(
    Path(target): Path<String>,
    State(server): State<Arc<McpServer>>,
) -> impl IntoResponse {
    match build_integration_preset(&server.config, &target) {
        Some(preset) => (StatusCode::OK, Json(preset)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!("integration target '{}' not supported", target),
                "supported": integration_catalog().into_iter().map(|item| item.id).collect::<Vec<_>>(),
            })),
        )
            .into_response(),
    }
}

/// SSE handler.
async fn handle_sse(headers: HeaderMap, State(server): State<Arc<McpServer>>) -> impl IntoResponse {
    let client_name = headers
        .get("x-client-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("anonymous");

    let welcome_msg = McpMessage::Notification(Notification {
        level: NotificationLevel::Info,
        message: format!(
            "SSE connected to {} v{} (client: {})",
            server.config.name, server.config.version, client_name
        ),
        data: None,
    });

    let welcome_payload = serde_json::to_string(&welcome_msg).unwrap_or_else(|_| "{}".to_string());
    let init_events: Vec<std::result::Result<axum::response::sse::Event, Infallible>> = vec![
        Ok(axum::response::sse::Event::default()
            .event("message")
            .data(welcome_payload)),
        Ok(axum::response::sse::Event::default()
            .event("ready")
            .data("{\"status\":\"ok\"}")),
    ];
    let init_stream = tokio_stream::iter(init_events);

    let heartbeat = tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(
        std::time::Duration::from_secs(20),
    ))
    .map(
        |_| -> std::result::Result<axum::response::sse::Event, Infallible> {
            Ok(axum::response::sse::Event::default()
                .event("heartbeat")
                .data("{\"type\":\"ping\"}"))
        },
    );

    let stream = init_stream.chain(heartbeat);
    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keep-alive"),
    )
}

fn json_rpc_ok(id: serde_json::Value, result: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn json_rpc_err(
    id: serde_json::Value,
    code: i64,
    message: &str,
    data: Option<serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
            "data": data,
        },
    })
}

fn parse_streamable_tool_name(params: &serde_json::Value) -> Option<String> {
    params
        .get("name")
        .and_then(|v| v.as_str())
        .or_else(|| params.get("tool").and_then(|v| v.as_str()))
        .or_else(|| params.get("tool_name").and_then(|v| v.as_str()))
        .or_else(|| params.get("toolName").and_then(|v| v.as_str()))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn parse_streamable_tool_args(params: &serde_json::Value) -> serde_json::Value {
    params
        .get("arguments")
        .cloned()
        .or_else(|| params.get("args").cloned())
        .unwrap_or_else(|| serde_json::json!({}))
}

async fn handle_streamable_http_rpc(
    State(server): State<Arc<McpServer>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let id = payload
        .get("id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let method = payload
        .get("method")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .unwrap_or_default();

    if method.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json_rpc_err(
                id,
                -32600,
                "invalid_request",
                Some(serde_json::json!({
                    "reason": "missing non-empty method",
                })),
            )),
        )
            .into_response();
    }

    match method {
        "initialize" => {
            let server_name = server.config.name.clone();
            let server_version = server.config.version.clone();
            let result = serde_json::json!({
                "protocolVersion": crate::protocol::MCP_PROTOCOL_VERSION,
                "serverInfo": {
                    "name": server_name,
                    "version": server_version,
                },
                "capabilities": {
                    "tools": {
                        "supported": true,
                        "listChanged": true
                    },
                    "resources": {
                        "supported": true,
                        "listChanged": true,
                        "subscribe": true
                    },
                    "roots": {
                        "supported": false
                    }
                },
                "instructions": "Welcome to Xenobot MCP Server"
            });
            (StatusCode::OK, Json(json_rpc_ok(id, result))).into_response()
        }
        "tools/list" | "tool/list" => {
            let tools = server.tools.read().await;
            let mut names: Vec<String> = tools.keys().cloned().collect();
            names.sort();
            let tool_specs: Vec<serde_json::Value> = names
                .iter()
                .map(|name| {
                    serde_json::json!({
                        "name": name,
                        "description": format!("Xenobot MCP tool: {name}"),
                    })
                })
                .collect();
            let result = serde_json::json!({
                "count": names.len(),
                "tools": tool_specs
            });
            (StatusCode::OK, Json(json_rpc_ok(id, result))).into_response()
        }
        "tools/call" | "tool/call" => {
            let params = payload
                .get("params")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let Some(tool_name) = parse_streamable_tool_name(&params) else {
                return (
                    StatusCode::OK,
                    Json(json_rpc_err(
                        id,
                        -32602,
                        "invalid_params",
                        Some(serde_json::json!({
                            "reason": "missing tool name in params.name or params.tool",
                        })),
                    )),
                )
                    .into_response();
            };
            let args = parse_streamable_tool_args(&params);
            let tools = server.tools.read().await;
            let Some(handler) = tools.get(&tool_name) else {
                return (
                    StatusCode::OK,
                    Json(json_rpc_err(
                        id,
                        -32001,
                        "tool_not_found",
                        Some(serde_json::json!({
                            "tool": tool_name,
                        })),
                    )),
                )
                    .into_response();
            };

            match handler(args) {
                Ok(tool_result) => (
                    StatusCode::OK,
                    Json(json_rpc_ok(
                        id,
                        serde_json::json!({
                            "tool": tool_name,
                            "isError": false,
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string(&tool_result).unwrap_or_else(|_| "{}".to_string())
                            }],
                            "structuredContent": tool_result
                        }),
                    )),
                )
                    .into_response(),
                Err(e) => (
                    StatusCode::OK,
                    Json(json_rpc_err(
                        id,
                        -32002,
                        "tool_error",
                        Some(serde_json::json!({
                            "tool": tool_name,
                            "error": e.to_string(),
                        })),
                    )),
                )
                    .into_response(),
            }
        }
        _ => (
            StatusCode::OK,
            Json(json_rpc_err(
                id,
                -32601,
                "method_not_found",
                Some(serde_json::json!({
                    "method": method,
                })),
            )),
        )
            .into_response(),
    }
}

/// HTTP tool call handler.
async fn handle_http_tool_call(
    Path(tool_name): Path<String>,
    State(server): State<Arc<McpServer>>,
    Json(args): Json<Value>,
) -> impl IntoResponse {
    let tools = server.tools.read().await;
    let handler = tools.get(&tool_name);

    match handler {
        Some(handler) => match handler(args) {
            Ok(result) => (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "tool": tool_name,
                    "result": result
                })),
            )
                .into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "code": "tool_error",
                    "error": e.to_string()
                })),
            )
                .into_response(),
        },
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "success": false,
                "code": "tool_not_found",
                "error": format!("Tool '{}' not found", tool_name)
            })),
        )
            .into_response(),
    }
}

/// HTTP resource handler.
async fn handle_http_resource(
    Path(uri): Path<String>,
    State(server): State<Arc<McpServer>>,
) -> impl IntoResponse {
    let resources = server.resources.read().await;
    match resources.get(&uri) {
        Some(resource) => {
            // Convert resource to JSON
            let json = serde_json::json!({
                "uri": resource.uri,
                "content": resource.content,
                "mimeType": resource.mime_type,
            });
            (StatusCode::OK, Json(json)).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Resource '{}' not found", uri) })),
        )
            .into_response(),
    }
}

/// Extract client information from headers.
fn extract_client_info(headers: &HeaderMap) -> Option<ClientInfo> {
    let name = headers
        .get("x-client-name")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let version = headers
        .get("x-client-version")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if name.is_some() || version.is_some() {
        Some(ClientInfo {
            name: name.unwrap_or_default(),
            version: version.unwrap_or_default(),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Method, Request, StatusCode};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower::util::ServiceExt;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);
    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn unique_db_path(name: &str) -> std::path::PathBuf {
        let epoch_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("xenobot_mcp_{name}_{epoch_nanos}_{seq}.db"))
    }

    fn seed_chat_records_fixture(db_path: &std::path::Path) {
        let conn = Connection::open(db_path).expect("open db");
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS meta (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                platform TEXT NOT NULL,
                chat_type TEXT NOT NULL,
                imported_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS member (
                id INTEGER PRIMARY KEY,
                platform_id TEXT NOT NULL,
                account_name TEXT,
                group_nickname TEXT,
                avatar TEXT
            );
            CREATE TABLE IF NOT EXISTS message (
                id INTEGER PRIMARY KEY,
                sender_id INTEGER,
                meta_id INTEGER NOT NULL,
                ts INTEGER NOT NULL,
                msg_type INTEGER NOT NULL,
                content TEXT,
                sender_group_nickname TEXT,
                sender_account_name TEXT
            );
            "#,
        )
        .expect("create schema");
        conn.execute(
            "INSERT INTO meta (id, name, platform, chat_type, imported_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![1_i64, "MCP Session", "wechat", "group", 1_900_000_000_i64],
        )
        .expect("insert meta");
        conn.execute(
            "INSERT INTO member (id, platform_id, account_name, group_nickname, avatar) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![11_i64, "u1", "Alice", "Alice", Option::<String>::None],
        )
        .expect("insert member");
        conn.execute(
            "INSERT INTO message (id, sender_id, meta_id, ts, msg_type, content, sender_group_nickname, sender_account_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                101_i64,
                11_i64,
                1_i64,
                1_900_000_001_i64,
                0_i64,
                "hello world",
                "Alice",
                "Alice"
            ],
        )
        .expect("insert message 1");
        conn.execute(
            "INSERT INTO message (id, sender_id, meta_id, ts, msg_type, content, sender_group_nickname, sender_account_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                102_i64,
                11_i64,
                1_i64,
                1_900_000_010_i64,
                0_i64,
                "another line",
                "Alice",
                "Alice"
            ],
        )
        .expect("insert message 2");
    }

    async fn request_json(
        app: &Router,
        method: Method,
        path: &str,
        payload: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let has_payload = payload.is_some();
        let body = payload
            .map(|p| Body::from(p.to_string()))
            .unwrap_or_else(Body::empty);
        let mut builder = Request::builder().method(method).uri(path);
        if path != "/tools" || has_payload {
            builder = builder.header("content-type", "application/json");
        }
        let request = builder.body(body).expect("build request");
        let response = app
            .clone()
            .oneshot(request)
            .await
            .expect("oneshot response");
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let json = serde_json::from_slice::<serde_json::Value>(&bytes).expect("json body");
        (status, json)
    }

    #[tokio::test]
    async fn test_server_creation() {
        let config = McpServerConfig::default();
        let server = McpServer::new(config);
        assert!(server.tools.read().await.is_empty());
        assert!(server.resources.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_builtin_tools_include_mcp_first_batch_aliases() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let config = McpServerConfig::default();
        let server = McpServer::new(config);
        server
            .register_builtin_tools()
            .await
            .expect("builtin tool registration should succeed");
        let tools = server.tools.read().await;
        for name in [
            "current_time",
            "get_current_time",
            "query_contacts",
            "query_groups",
            "recent_sessions",
            "query_chats",
            "chat_records",
            "chat_history",
        ] {
            assert!(tools.contains_key(name), "missing tool alias: {name}");
        }
    }

    #[tokio::test]
    async fn test_chat_records_tool_supports_filters_and_paging() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let db_path = unique_db_path("chat_records");
        let previous_db = std::env::var("XENOBOT_DB_PATH").ok();
        std::env::set_var("XENOBOT_DB_PATH", &db_path);
        seed_chat_records_fixture(&db_path);

        let config = McpServerConfig::default();
        let server = McpServer::new(config);
        server
            .register_builtin_tools()
            .await
            .expect("register builtin tools");
        let tools = server.tools.read().await;
        let handler = tools
            .get("chat_records")
            .expect("chat_records tool must exist");

        let result = handler(serde_json::json!({
            "sessionId": 1,
            "keyword": "hello",
            "startTs": 1_900_000_000_i64,
            "endTs": 1_900_000_005_i64,
            "limit": 10,
            "offset": 0
        }))
        .expect("chat_records tool should succeed");
        assert_eq!(result["totalCount"], 1);
        assert_eq!(result["count"], 1);
        assert_eq!(result["messages"][0]["content"], "hello world");
        assert_eq!(result["hasMore"], false);

        drop(tools);
        if let Some(previous) = previous_db {
            std::env::set_var("XENOBOT_DB_PATH", previous);
        } else {
            std::env::remove_var("XENOBOT_DB_PATH");
        }
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_http_tools_contract_and_error_semantics() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let db_path = unique_db_path("http_contract");
        let previous_db = std::env::var("XENOBOT_DB_PATH").ok();
        std::env::set_var("XENOBOT_DB_PATH", &db_path);
        seed_chat_records_fixture(&db_path);

        let config = McpServerConfig::default();
        let server = McpServer::new(config);
        server
            .register_builtin_tools()
            .await
            .expect("register builtin tools");
        let app = server.create_router();

        let (status, tools_json) = request_json(&app, Method::GET, "/tools", None).await;
        assert_eq!(status, StatusCode::OK);
        let names = tools_json["tools"]
            .as_array()
            .expect("tools should be array")
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<std::collections::HashSet<_>>();
        for required in [
            "current_time",
            "get_current_time",
            "query_contacts",
            "query_groups",
            "recent_sessions",
            "query_chats",
            "chat_records",
            "chat_history",
        ] {
            assert!(
                names.contains(required),
                "missing tool in HTTP list: {required}"
            );
        }

        let (status, success_json) = request_json(
            &app,
            Method::POST,
            "/tools/chat_records",
            Some(serde_json::json!({
                "sessionId": 1,
                "keyword": "hello",
                "startTs": 1_900_000_000_i64,
                "endTs": 1_900_000_005_i64,
                "limit": 20,
                "offset": 0
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(success_json["success"], true);
        assert_eq!(success_json["tool"], "chat_records");
        assert_eq!(success_json["result"]["totalCount"], 1);
        assert_eq!(
            success_json["result"]["messages"][0]["content"],
            "hello world"
        );

        let (status, query_chats_json) = request_json(
            &app,
            Method::POST,
            "/tools/query_chats",
            Some(serde_json::json!({
                "limit": 10
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(query_chats_json["success"], true);
        assert_eq!(query_chats_json["tool"], "query_chats");
        assert!(query_chats_json["result"]["count"].as_u64().unwrap_or(0) >= 1);

        let (status, query_contacts_json) = request_json(
            &app,
            Method::POST,
            "/tools/query_contacts",
            Some(serde_json::json!({
                "sessionId": 1,
                "limit": 10
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(query_contacts_json["success"], true);
        assert_eq!(query_contacts_json["tool"], "query_contacts");
        assert!(query_contacts_json["result"]["count"].as_u64().unwrap_or(0) >= 1);

        let (status, history_alias_json) = request_json(
            &app,
            Method::POST,
            "/tools/chat_history",
            Some(serde_json::json!({
                "session_id": 1,
                "keyword": "another",
                "start_ts": 1_900_000_006_i64,
                "end_ts": 1_900_000_020_i64,
                "limit": 20,
                "offset": 0
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(history_alias_json["success"], true);
        assert_eq!(history_alias_json["tool"], "chat_history");
        assert_eq!(history_alias_json["result"]["totalCount"], 1);
        assert_eq!(
            history_alias_json["result"]["messages"][0]["content"],
            "another line"
        );

        let (status, alias_time_json) = request_json(
            &app,
            Method::POST,
            "/tools/get_current_time",
            Some(serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(alias_time_json["success"], true);
        assert_eq!(alias_time_json["tool"], "get_current_time");
        assert!(alias_time_json["result"]["unix"].is_number());

        let (status, missing_tool_json) = request_json(
            &app,
            Method::POST,
            "/tools/not_exists",
            Some(serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(missing_tool_json["success"], false);
        assert_eq!(missing_tool_json["code"], "tool_not_found");

        let (status, bad_args_json) = request_json(
            &app,
            Method::POST,
            "/tools/chat_records",
            Some(serde_json::json!({
                "keyword": "hello"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(bad_args_json["success"], false);
        assert_eq!(bad_args_json["code"], "tool_error");
        assert!(bad_args_json["error"]
            .as_str()
            .is_some_and(|text| text.contains("missing required integer field")));

        if let Some(previous) = previous_db {
            std::env::set_var("XENOBOT_DB_PATH", previous);
        } else {
            std::env::remove_var("XENOBOT_DB_PATH");
        }
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_streamable_http_rpc_contract_and_error_semantics() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let db_path = unique_db_path("streamable_rpc");
        let previous_db = std::env::var("XENOBOT_DB_PATH").ok();
        std::env::set_var("XENOBOT_DB_PATH", &db_path);
        seed_chat_records_fixture(&db_path);

        let config = McpServerConfig::default();
        let server = McpServer::new(config);
        server
            .register_builtin_tools()
            .await
            .expect("register builtin tools");
        let app = server.create_router();

        let (status, init_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "init-1",
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05"
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(init_json["jsonrpc"], "2.0");
        assert_eq!(init_json["id"], "init-1");
        assert_eq!(
            init_json["result"]["protocolVersion"],
            crate::protocol::MCP_PROTOCOL_VERSION
        );
        assert!(init_json["result"]["serverInfo"]["name"]
            .as_str()
            .is_some_and(|value| !value.trim().is_empty()));

        let (status, list_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "list-1",
                "method": "tools/list"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let tool_names = list_json["result"]["tools"]
            .as_array()
            .expect("tools/list result should contain tools array")
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<std::collections::HashSet<_>>();
        for required in [
            "chat_records",
            "query_contacts",
            "query_groups",
            "get_current_time",
        ] {
            assert!(
                tool_names.contains(required),
                "missing tool in streamable list: {required}"
            );
        }

        let (status, call_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "call-1",
                "method": "tools/call",
                "params": {
                    "name": "chat_records",
                    "arguments": {
                        "sessionId": 1,
                        "keyword": "hello",
                        "startTs": 1_900_000_000_i64,
                        "endTs": 1_900_000_005_i64,
                        "limit": 20,
                        "offset": 0
                    }
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(call_json["jsonrpc"], "2.0");
        assert_eq!(call_json["id"], "call-1");
        assert_eq!(call_json["result"]["tool"], "chat_records");
        assert_eq!(call_json["result"]["isError"], false);
        assert_eq!(call_json["result"]["structuredContent"]["totalCount"], 1);
        assert_eq!(
            call_json["result"]["structuredContent"]["messages"][0]["content"],
            "hello world"
        );

        let (status, missing_tool_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "call-2",
                "method": "tools/call",
                "params": {
                    "name": "not_exists",
                    "arguments": {}
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(missing_tool_json["error"]["code"], -32001);
        assert_eq!(missing_tool_json["error"]["message"], "tool_not_found");

        let (status, bad_params_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "call-3",
                "method": "tools/call",
                "params": {
                    "arguments": {}
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(bad_params_json["error"]["code"], -32602);
        assert_eq!(bad_params_json["error"]["message"], "invalid_params");

        let (status, unknown_method_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "unknown-1",
                "method": "ping"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(unknown_method_json["error"]["code"], -32601);
        assert_eq!(unknown_method_json["error"]["message"], "method_not_found");

        if let Some(previous) = previous_db {
            std::env::set_var("XENOBOT_DB_PATH", previous);
        } else {
            std::env::remove_var("XENOBOT_DB_PATH");
        }
        let _ = std::fs::remove_file(&db_path);
    }

    #[tokio::test]
    async fn test_sse_endpoint_returns_event_stream_content_type() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let config = McpServerConfig::default();
        let server = McpServer::new(config);
        let app = server.create_router();

        let request = Request::builder()
            .method(Method::GET)
            .uri("/sse")
            .body(Body::empty())
            .expect("build sse request");
        let response = app
            .oneshot(request)
            .await
            .expect("sse oneshot should succeed");
        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(
            content_type.starts_with("text/event-stream"),
            "unexpected content-type: {content_type}"
        );
    }

    #[tokio::test]
    async fn test_http_integrations_catalog_and_preset_endpoints() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let config = McpServerConfig::default();
        let server = McpServer::new(config);
        let app = server.create_router();

        let (status, catalog_json) = request_json(&app, Method::GET, "/integrations", None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(catalog_json["integrations"]
            .as_array()
            .is_some_and(|arr| !arr.is_empty()));
        assert!(catalog_json["integrations"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(|item| item["id"] == "claude-desktop")));

        let (status, preset_json) =
            request_json(&app, Method::GET, "/integrations/claude-desktop", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(preset_json["id"], "claude-desktop");
        assert!(preset_json["transport"]["sse"]
            .as_str()
            .is_some_and(|url| url.ends_with("/sse")));
        assert!(preset_json["configuration"]["mcpServers"]["xenobot"].is_object());

        let (status, unknown_json) = request_json(
            &app,
            Method::GET,
            "/integrations/not-supported-target",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(unknown_json["supported"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(|item| item == "claude-desktop")));
    }

    #[test]
    fn test_build_integration_preset_claude_desktop() {
        let config = McpServerConfig::default();
        let preset =
            build_integration_preset(&config, "claude-desktop").expect("preset should exist");
        assert_eq!(preset.id, "claude-desktop");
        assert!(preset.configuration["mcpServers"]["xenobot"].is_object());
    }

    #[test]
    fn test_build_integration_preset_chatwise() {
        let config = McpServerConfig::default();
        let preset = build_integration_preset(&config, "chatwise").expect("preset should exist");
        assert_eq!(preset.id, "chatwise");
        assert!(preset.configuration["servers"][0].is_object());
    }

    #[test]
    fn test_build_integration_preset_opencode() {
        let config = McpServerConfig::default();
        let preset = build_integration_preset(&config, "opencode").expect("preset should exist");
        assert_eq!(preset.id, "opencode");
        assert!(preset.configuration["mcpServers"][0].is_object());
    }
}

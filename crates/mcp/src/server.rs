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
        self.register_tool("member_stats".to_string(), Box::new(member_stats_tool))
            .await?;
        self.register_tool("get_member_stats".to_string(), Box::new(member_stats_tool))
            .await?;
        self.register_tool("time_stats".to_string(), Box::new(time_stats_tool))
            .await?;
        self.register_tool("get_time_stats".to_string(), Box::new(time_stats_tool))
            .await?;
        self.register_tool(
            "session_summary".to_string(),
            Box::new(session_summary_tool),
        )
        .await?;
        self.register_tool(
            "get_session_summary".to_string(),
            Box::new(session_summary_tool),
        )
        .await?;

        Ok(())
    }

    /// Register built-in MCP resources for baseline client integration.
    async fn register_builtin_resources(&self) -> Result<()> {
        let server_info_uri = "xenobot://server/info".to_string();
        let capabilities_uri = "xenobot://server/capabilities".to_string();
        let integrations_uri = "xenobot://server/integrations".to_string();

        let server_info_text = serde_json::json!({
            "name": self.config.name,
            "version": self.config.version,
            "bindAddress": self.config.bind_address,
            "port": self.config.port,
        })
        .to_string();
        let capabilities_text = serde_json::json!({
            "transport": {
                "sse": self.config.enable_sse,
                "streamableHttp": self.config.enable_streamable_http,
            },
            "tools": {
                "firstBatch": [
                    "query_contacts",
                    "query_groups",
                    "recent_sessions",
                    "chat_records",
                    "get_current_time"
                ],
                "secondBatch": [
                    "member_stats",
                    "time_stats",
                    "session_summary"
                ]
            },
            "resources": {
                "list": true,
                "read": true,
                "subscribe": true
            }
        })
        .to_string();
        let integrations_text = serde_json::json!({
            "targets": ["claude-desktop", "chatwise", "opencode", "pencil"],
            "hint": "use /integrations/{target} to fetch a transport preset"
        })
        .to_string();

        let mut resources = self.resources.write().await;
        resources.insert(
            server_info_uri.clone(),
            Resource {
                uri: server_info_uri,
                content: vec![Content::Text {
                    text: server_info_text,
                }],
                mime_type: Some("application/json".to_string()),
            },
        );
        resources.insert(
            capabilities_uri.clone(),
            Resource {
                uri: capabilities_uri,
                content: vec![Content::Text {
                    text: capabilities_text,
                }],
                mime_type: Some("application/json".to_string()),
            },
        );
        resources.insert(
            integrations_uri.clone(),
            Resource {
                uri: integrations_uri,
                content: vec![Content::Text {
                    text: integrations_text,
                }],
                mime_type: Some("application/json".to_string()),
            },
        );

        Ok(())
    }

    /// Start the MCP server.
    pub async fn start(self) -> Result<()> {
        self.register_builtin_tools().await?;
        self.register_builtin_resources().await?;
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
            // HTTP endpoint for listing resources
            .route("/resources", get(handle_http_resources_list))
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

fn member_stats_tool(args: Value) -> Result<Value> {
    let meta_id = parse_required_i64(&args, "session_id")?;
    let limit = parse_limit(&args, 20, 500)?;
    let conn = open_xenobot_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT
                m.id,
                COALESCE(msg.sender_group_nickname, msg.sender_account_name, m.group_nickname, m.account_name, m.platform_id, '') as sender_name,
                CAST(COUNT(*) AS INTEGER) as message_count
             FROM message msg
             LEFT JOIN member m ON m.id = msg.sender_id
             WHERE msg.meta_id = ?1
             GROUP BY m.id, sender_name
             ORDER BY message_count DESC, m.id ASC
             LIMIT ?2",
        )
        .map_err(|e| McpError::Tool(format!("prepare member_stats failed: {e}")))?;
    let rows = stmt
        .query_map(params![meta_id, limit], |row| {
            Ok(serde_json::json!({
                "memberId": row.get::<_, Option<i64>>(0)?,
                "senderName": row.get::<_, String>(1)?,
                "messageCount": row.get::<_, i64>(2)?,
            }))
        })
        .map_err(|e| McpError::Tool(format!("query member_stats failed: {e}")))?;

    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|e| McpError::Tool(format!("read member_stats row failed: {e}")))?);
    }
    Ok(serde_json::json!({
        "sessionId": meta_id,
        "count": items.len(),
        "members": items
    }))
}

fn time_stats_tool(args: Value) -> Result<Value> {
    let meta_id = parse_required_i64(&args, "session_id")?;
    let granularity = parse_granularity(&args);
    let conn = open_xenobot_db()?;

    let period_sql = match granularity.as_str() {
        "hour" => "strftime('%H', datetime(msg.ts, 'unixepoch'))",
        "weekday" => "strftime('%w', datetime(msg.ts, 'unixepoch'))",
        "month" => "strftime('%m', datetime(msg.ts, 'unixepoch'))",
        "year" => "strftime('%Y', datetime(msg.ts, 'unixepoch'))",
        _ => "strftime('%Y-%m-%d', datetime(msg.ts, 'unixepoch'))",
    };
    let sql = format!(
        "SELECT
            {period_sql} as period,
            CAST(COUNT(*) AS INTEGER) as count
         FROM message msg
         WHERE msg.meta_id = ?1
         GROUP BY period
         ORDER BY period ASC"
    );

    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| McpError::Tool(format!("prepare time_stats failed: {e}")))?;
    let rows = stmt
        .query_map(params![meta_id], |row| {
            Ok(serde_json::json!({
                "period": row.get::<_, String>(0)?,
                "count": row.get::<_, i64>(1)?,
            }))
        })
        .map_err(|e| McpError::Tool(format!("query time_stats failed: {e}")))?;

    let mut buckets = Vec::new();
    let mut total_messages = 0_i64;
    for row in rows {
        let item = row.map_err(|e| McpError::Tool(format!("read time_stats row failed: {e}")))?;
        total_messages += item["count"].as_i64().unwrap_or(0);
        buckets.push(item);
    }

    Ok(serde_json::json!({
        "sessionId": meta_id,
        "granularity": granularity,
        "bucketCount": buckets.len(),
        "totalMessages": total_messages,
        "buckets": buckets
    }))
}

fn session_summary_tool(args: Value) -> Result<Value> {
    let meta_id = parse_required_i64(&args, "session_id")?;
    let conn = open_xenobot_db()?;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, platform, chat_type, imported_at
             FROM meta
             WHERE id = ?1",
        )
        .map_err(|e| McpError::Tool(format!("prepare session_summary meta failed: {e}")))?;
    let meta_row = stmt
        .query_row(params![meta_id], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "name": row.get::<_, String>(1)?,
                "platform": row.get::<_, String>(2)?,
                "chatType": row.get::<_, String>(3)?,
                "importedAt": row.get::<_, i64>(4)?,
            }))
        })
        .map_err(|e| McpError::Tool(format!("query session_summary meta failed: {e}")))?;

    let (message_count, unique_senders, min_ts, max_ts): (i64, i64, Option<i64>, Option<i64>) =
        conn.query_row(
            "SELECT
                CAST(COUNT(*) AS INTEGER),
                CAST(COUNT(DISTINCT sender_id) AS INTEGER),
                MIN(ts),
                MAX(ts)
             FROM message
             WHERE meta_id = ?1",
            params![meta_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| McpError::Tool(format!("query session_summary stats failed: {e}")))?;

    Ok(serde_json::json!({
        "session": meta_row,
        "summary": {
            "messageCount": message_count,
            "uniqueSenders": unique_senders,
            "startTs": min_ts,
            "endTs": max_ts
        }
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

fn parse_granularity(args: &Value) -> String {
    arg_value(args, "granularity")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_lowercase())
        .filter(|s| matches!(s.as_str(), "day" | "hour" | "weekday" | "month" | "year"))
        .unwrap_or_else(|| "day".to_string())
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
        .or_else(|| args.get(snake_to_camel(key)))
        .or_else(|| args.get(camel_to_snake(key)))
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
        IntegrationCatalogItem {
            id: "pencil".to_string(),
            name: "Pencil".to_string(),
            description: "Configuration snippet for Pencil-compatible MCP integration".to_string(),
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
        "pencil" => Some(IntegrationPreset {
            id: "pencil".to_string(),
            name: "Pencil".to_string(),
            description: "Pencil-compatible MCP configuration using Xenobot SSE endpoint".to_string(),
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
                        "url": sse_url,
                        "toolsUrl": tools_url
                    }
                ]
            }),
            notes: vec![
                "Use this preset when your Pencil build supports custom MCP servers over SSE.".to_string(),
                "If Pencil expects different UI labels, keep the same SSE URL and map the values manually.".to_string(),
                "When a direct Pencil tool entry is not available in the current runtime, fetch this preset from Xenobot and apply it in the Pencil host.".to_string(),
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
    let tool_specs = build_tool_specs(&names);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "count": names.len(),
            "tools": names,
            "toolSpecs": tool_specs,
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

fn extract_non_empty_string_from_keys(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(|v| v.as_str()) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn parse_streamable_tool_name(params: &serde_json::Value) -> Option<String> {
    if let Some(name) =
        extract_non_empty_string_from_keys(params, &["name", "tool", "tool_name", "toolName"])
    {
        return Some(name);
    }

    params
        .get("tool")
        .filter(|value| value.is_object())
        .and_then(|tool| {
            extract_non_empty_string_from_keys(tool, &["name", "tool", "tool_name", "toolName"])
        })
}

fn parse_streamable_tool_args(params: &serde_json::Value) -> serde_json::Value {
    if let Some(direct_args) = params
        .get("arguments")
        .cloned()
        .or_else(|| params.get("args").cloned())
    {
        return direct_args;
    }

    if let Some(tool_obj) = params.get("tool").filter(|value| value.is_object()) {
        if let Some(nested_args) = tool_obj
            .get("arguments")
            .cloned()
            .or_else(|| tool_obj.get("args").cloned())
            .or_else(|| tool_obj.get("input").cloned())
        {
            return nested_args;
        }
    }

    serde_json::json!({})
}

fn parse_streamable_resource_uri(params: &serde_json::Value) -> Option<String> {
    if let Some(uri) = extract_non_empty_string_from_keys(
        params,
        &["uri", "resource", "path", "resource_uri", "resourceUri"],
    ) {
        return Some(uri);
    }

    params
        .get("resource")
        .filter(|value| value.is_object())
        .and_then(|resource| {
            extract_non_empty_string_from_keys(
                resource,
                &["uri", "path", "resource_uri", "resourceUri"],
            )
        })
}

fn tool_spec_for_name(name: &str) -> serde_json::Value {
    let (description, input_schema) = match name {
        "current_time" | "get_current_time" => (
            "Get current UTC time.",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        ),
        "query_contacts" | "list_contacts" => (
            "Query contacts with optional session scope and paging.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "sessionId": {"type": "integer"},
                    "session_id": {"type": "integer"},
                    "limit": {"type": "integer"},
                    "offset": {"type": "integer"}
                },
                "additionalProperties": true
            }),
        ),
        "query_groups" => (
            "List available group chats.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": {"type": "integer"},
                    "offset": {"type": "integer"}
                },
                "additionalProperties": true
            }),
        ),
        "member_stats" | "get_member_stats" => (
            "Aggregate message counts by member in a session.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "sessionId": {"type": "integer"},
                    "session_id": {"type": "integer"},
                    "limit": {"type": "integer"}
                },
                "anyOf": [
                    {"required": ["sessionId"]},
                    {"required": ["session_id"]}
                ],
                "additionalProperties": true
            }),
        ),
        "time_stats" | "get_time_stats" => (
            "Aggregate message distribution by time buckets.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "sessionId": {"type": "integer"},
                    "session_id": {"type": "integer"},
                    "granularity": {
                        "type": "string",
                        "enum": ["day", "hour", "weekday", "month", "year"]
                    }
                },
                "anyOf": [
                    {"required": ["sessionId"]},
                    {"required": ["session_id"]}
                ],
                "additionalProperties": true
            }),
        ),
        "session_summary" | "get_session_summary" => (
            "Return session profile and aggregate counters.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "sessionId": {"type": "integer"},
                    "session_id": {"type": "integer"}
                },
                "anyOf": [
                    {"required": ["sessionId"]},
                    {"required": ["session_id"]}
                ],
                "additionalProperties": true
            }),
        ),
        "recent_sessions" | "query_chats" | "list_sessions" => (
            "List recent sessions.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": {"type": "integer"},
                    "offset": {"type": "integer"}
                },
                "additionalProperties": true
            }),
        ),
        "chat_records" | "chat_history" | "recent_messages" | "search_messages" => (
            "Query chat messages with optional keyword and time range filters.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "sessionId": {"type": "integer"},
                    "session_id": {"type": "integer"},
                    "keyword": {"type": "string"},
                    "query": {"type": "string"},
                    "startTs": {"type": "integer"},
                    "start_ts": {"type": "integer"},
                    "endTs": {"type": "integer"},
                    "end_ts": {"type": "integer"},
                    "limit": {"type": "integer"},
                    "offset": {"type": "integer"}
                },
                "anyOf": [
                    {"required": ["sessionId"]},
                    {"required": ["session_id"]}
                ],
                "additionalProperties": true
            }),
        ),
        _ => (
            "Xenobot MCP tool.",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": true
            }),
        ),
    };
    serde_json::json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

fn build_tool_specs(names: &[String]) -> Vec<serde_json::Value> {
    names.iter().map(|name| tool_spec_for_name(name)).collect()
}

fn classify_tool_error_for_streamable(err: &McpError) -> (i64, &'static str) {
    match err {
        McpError::Argument(_) => (-32602, "invalid_params"),
        _ => (-32002, "tool_error"),
    }
}

fn classify_tool_error_for_http(err: &McpError) -> (StatusCode, &'static str) {
    match err {
        McpError::Argument(_) => (StatusCode::BAD_REQUEST, "invalid_params"),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "tool_error"),
    }
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
            let tool_specs = build_tool_specs(&names);
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
                Err(e) => {
                    let (code, message) = classify_tool_error_for_streamable(&e);
                    (
                        StatusCode::OK,
                        Json(json_rpc_err(
                            id,
                            code,
                            message,
                            Some(serde_json::json!({
                                "tool": tool_name,
                                "error": e.to_string(),
                            })),
                        )),
                    )
                        .into_response()
                }
            }
        }
        "resources/list" | "resource/list" => {
            let resources = server.resources.read().await;
            let mut uris: Vec<String> = resources.keys().cloned().collect();
            uris.sort();
            let resource_specs: Vec<serde_json::Value> = uris
                .iter()
                .map(|uri| {
                    let resource = resources.get(uri).expect("resource key must exist");
                    serde_json::json!({
                        "uri": resource.uri,
                        "name": resource.uri.rsplit('/').next().unwrap_or("resource"),
                        "description": format!("Xenobot MCP resource: {}", resource.uri),
                        "mimeType": resource.mime_type
                    })
                })
                .collect();
            let result = serde_json::json!({
                "count": resource_specs.len(),
                "resources": resource_specs
            });
            (StatusCode::OK, Json(json_rpc_ok(id, result))).into_response()
        }
        "resources/read" | "resource/read" => {
            let params = payload
                .get("params")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let Some(uri) = parse_streamable_resource_uri(&params) else {
                return (
                    StatusCode::OK,
                    Json(json_rpc_err(
                        id,
                        -32602,
                        "invalid_params",
                        Some(serde_json::json!({
                            "reason": "missing resource URI in params.uri",
                        })),
                    )),
                )
                    .into_response();
            };

            let resources = server.resources.read().await;
            let Some(resource) = resources.get(&uri) else {
                return (
                    StatusCode::OK,
                    Json(json_rpc_err(
                        id,
                        -32003,
                        "resource_not_found",
                        Some(serde_json::json!({
                            "uri": uri,
                        })),
                    )),
                )
                    .into_response();
            };

            let result = serde_json::json!({
                "uri": resource.uri,
                "mimeType": resource.mime_type,
                "content": resource.content,
            });
            (StatusCode::OK, Json(json_rpc_ok(id, result))).into_response()
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
            Err(e) => {
                let (status, code) = classify_tool_error_for_http(&e);
                (
                    status,
                    Json(serde_json::json!({
                        "success": false,
                        "code": code,
                        "error": e.to_string()
                    })),
                )
                    .into_response()
            }
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

/// HTTP resource list handler.
async fn handle_http_resources_list(State(server): State<Arc<McpServer>>) -> impl IntoResponse {
    let resources = server.resources.read().await;
    let mut uris: Vec<String> = resources.keys().cloned().collect();
    uris.sort();
    let items: Vec<serde_json::Value> = uris
        .iter()
        .filter_map(|uri| resources.get(uri))
        .map(|resource| {
            serde_json::json!({
                "uri": resource.uri,
                "mimeType": resource.mime_type,
            })
        })
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "count": items.len(),
            "resources": items,
        })),
    )
        .into_response()
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

    fn build_ws_request(with_upgrade_headers: bool) -> Request<Body> {
        let connect_info = ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 45_678)));
        let mut builder = Request::builder()
            .method(Method::GET)
            .uri("/ws")
            .extension(connect_info);
        if with_upgrade_headers {
            builder = builder
                .header(axum::http::header::CONNECTION, "Upgrade")
                .header(axum::http::header::UPGRADE, "websocket")
                .header(axum::http::header::SEC_WEBSOCKET_VERSION, "13")
                .header(
                    axum::http::header::SEC_WEBSOCKET_KEY,
                    "dGhlIHNhbXBsZSBub25jZQ==",
                );
        }
        builder
            .body(Body::empty())
            .expect("build websocket request")
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
            "member_stats",
            "get_member_stats",
            "time_stats",
            "get_time_stats",
            "session_summary",
            "get_session_summary",
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
        let specs = tools_json["toolSpecs"]
            .as_array()
            .expect("toolSpecs should be array");
        assert!(
            specs.iter().all(|item| {
                item["name"].as_str().is_some()
                    && item["description"].as_str().is_some()
                    && item["inputSchema"].is_object()
            }),
            "every tool spec should expose name/description/inputSchema"
        );
        assert!(specs.iter().any(|item| {
            item["name"] == "chat_records"
                && item["inputSchema"]["properties"]["sessionId"].is_object()
        }));
        let query_contacts_spec = specs
            .iter()
            .find(|item| item["name"] == "query_contacts")
            .expect("query_contacts spec should exist");
        assert!(
            query_contacts_spec["inputSchema"]["required"].is_null(),
            "query_contacts should not require sessionId/session_id"
        );

        let chat_records_spec = specs
            .iter()
            .find(|item| item["name"] == "chat_records")
            .expect("chat_records spec should exist");
        let any_of = chat_records_spec["inputSchema"]["anyOf"]
            .as_array()
            .expect("chat_records anyOf should exist");
        assert!(
            any_of.iter().any(|rule| rule["required"][0] == "sessionId"),
            "chat_records schema should accept sessionId"
        );
        assert!(
            any_of
                .iter()
                .any(|rule| rule["required"][0] == "session_id"),
            "chat_records schema should accept session_id"
        );
        for required in [
            "current_time",
            "get_current_time",
            "query_contacts",
            "query_groups",
            "recent_sessions",
            "query_chats",
            "chat_records",
            "chat_history",
            "member_stats",
            "time_stats",
            "session_summary",
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

        let (status, query_groups_json) = request_json(
            &app,
            Method::POST,
            "/tools/query_groups",
            Some(serde_json::json!({
                "limit": 10,
                "offset": 0
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(query_groups_json["success"], true);
        assert_eq!(query_groups_json["tool"], "query_groups");
        assert!(query_groups_json["result"]["count"].as_u64().unwrap_or(0) >= 1);

        let (status, recent_sessions_json) = request_json(
            &app,
            Method::POST,
            "/tools/recent_sessions",
            Some(serde_json::json!({
                "limit": 10
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(recent_sessions_json["success"], true);
        assert_eq!(recent_sessions_json["tool"], "recent_sessions");
        assert!(
            recent_sessions_json["result"]["count"]
                .as_u64()
                .unwrap_or(0)
                >= 1
        );

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

        let (status, member_stats_json) = request_json(
            &app,
            Method::POST,
            "/tools/member_stats",
            Some(serde_json::json!({
                "sessionId": 1,
                "limit": 10
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(member_stats_json["success"], true);
        assert_eq!(member_stats_json["tool"], "member_stats");
        assert!(member_stats_json["result"]["count"].as_u64().unwrap_or(0) >= 1);

        let (status, time_stats_json) = request_json(
            &app,
            Method::POST,
            "/tools/time_stats",
            Some(serde_json::json!({
                "session_id": 1,
                "granularity": "day"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(time_stats_json["success"], true);
        assert_eq!(time_stats_json["tool"], "time_stats");
        assert!(
            time_stats_json["result"]["totalMessages"]
                .as_u64()
                .unwrap_or(0)
                >= 1
        );

        let (status, summary_json) = request_json(
            &app,
            Method::POST,
            "/tools/session_summary",
            Some(serde_json::json!({
                "sessionId": 1
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(summary_json["success"], true);
        assert_eq!(summary_json["tool"], "session_summary");
        assert_eq!(summary_json["result"]["session"]["id"], 1);
        assert_eq!(summary_json["result"]["summary"]["messageCount"], 2);

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
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(bad_args_json["success"], false);
        assert_eq!(bad_args_json["code"], "invalid_params");
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
        assert_eq!(
            init_json["result"]["capabilities"]["tools"]["supported"],
            true
        );
        assert_eq!(
            init_json["result"]["capabilities"]["tools"]["listChanged"],
            true
        );
        assert_eq!(
            init_json["result"]["capabilities"]["resources"]["supported"],
            true
        );
        assert_eq!(
            init_json["result"]["capabilities"]["resources"]["subscribe"],
            true
        );
        assert_eq!(
            init_json["result"]["capabilities"]["roots"]["supported"],
            false
        );
        assert!(init_json["result"]["instructions"]
            .as_str()
            .is_some_and(|text| !text.trim().is_empty()));

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
        assert!(list_json["result"]["tools"]
            .as_array()
            .is_some_and(|arr| arr.iter().all(|tool| tool["inputSchema"].is_object())));
        assert!(list_json["result"]["tools"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(|tool| {
                tool["name"] == "chat_records"
                    && tool["inputSchema"]["properties"]["sessionId"].is_object()
            })));
        for required in [
            "chat_records",
            "query_contacts",
            "query_groups",
            "get_current_time",
            "member_stats",
            "time_stats",
            "session_summary",
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

        let (status, invalid_tool_args_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "call-4",
                "method": "tools/call",
                "params": {
                    "name": "chat_records",
                    "arguments": {
                        "keyword": "hello"
                    }
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(invalid_tool_args_json["error"]["code"], -32602);
        assert_eq!(invalid_tool_args_json["error"]["message"], "invalid_params");
        assert!(invalid_tool_args_json["error"]["data"]["error"]
            .as_str()
            .is_some_and(|text| text.contains("missing required integer field")));

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
    async fn test_ws_endpoint_rejects_missing_upgrade_headers() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let config = McpServerConfig::default();
        let server = McpServer::new(config);
        let app = server.create_router();

        let response = app
            .oneshot(build_ws_request(false))
            .await
            .expect("ws oneshot should succeed");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_ws_endpoint_returns_upgrade_required_in_oneshot_transport() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let config = McpServerConfig::default();
        let server = McpServer::new(config);
        let app = server.create_router();

        let response = app
            .oneshot(build_ws_request(true))
            .await
            .expect("ws oneshot should succeed");
        // In unit tests we use `Router::oneshot`, which does not provide Hyper's
        // on-upgrade extension required by `WebSocketUpgrade`, so 426 is expected.
        assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
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
        assert!(catalog_json["integrations"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(|item| item["id"] == "pencil")));

        let (status, preset_json) =
            request_json(&app, Method::GET, "/integrations/claude-desktop", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(preset_json["id"], "claude-desktop");
        assert!(preset_json["transport"]["sse"]
            .as_str()
            .is_some_and(|url| url.ends_with("/sse")));
        assert!(preset_json["configuration"]["mcpServers"]["xenobot"].is_object());
        assert_eq!(
            preset_json["configuration"]["mcpServers"]["xenobot"]["command"],
            "pnpm"
        );
        assert!(
            preset_json["configuration"]["mcpServers"]["xenobot"]["args"]
                .as_array()
                .is_some_and(|arr| arr.iter().any(|item| item == "mcp-remote"))
        );

        let (status, chatwise_json) =
            request_json(&app, Method::GET, "/integrations/chatwise", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(chatwise_json["id"], "chatwise");
        assert!(chatwise_json["configuration"]["servers"]
            .as_array()
            .is_some_and(|arr| arr.first().is_some_and(|item| item["name"] == "xenobot")));
        assert!(chatwise_json["configuration"]["servers"]
            .as_array()
            .is_some_and(|arr| arr.first().is_some_and(|item| item["transport"] == "sse")));

        let (status, opencode_json) =
            request_json(&app, Method::GET, "/integrations/opencode", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(opencode_json["id"], "opencode");
        assert!(opencode_json["configuration"]["mcpServers"]
            .as_array()
            .is_some_and(|arr| arr.first().is_some_and(|item| item["name"] == "xenobot")));
        assert!(opencode_json["configuration"]["mcpServers"]
            .as_array()
            .is_some_and(|arr| arr.first().is_some_and(|item| item["transport"] == "sse")));

        let (status, pencil_json) =
            request_json(&app, Method::GET, "/integrations/pencil", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(pencil_json["id"], "pencil");
        assert!(pencil_json["configuration"]["servers"]
            .as_array()
            .is_some_and(|arr| arr.first().is_some_and(|item| item["name"] == "xenobot")));
        assert!(pencil_json["configuration"]["servers"]
            .as_array()
            .is_some_and(|arr| arr.first().is_some_and(|item| item["transport"] == "sse")));

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

    fn percent_encode_path(value: &str) -> String {
        value
            .replace('%', "%25")
            .replace(':', "%3A")
            .replace('/', "%2F")
            .replace('?', "%3F")
            .replace('#', "%23")
            .replace(' ', "%20")
    }

    #[tokio::test]
    async fn test_http_resources_contract_and_read() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let config = McpServerConfig::default();
        let server = McpServer::new(config);
        server
            .register_builtin_resources()
            .await
            .expect("register builtin resources");
        let app = server.create_router();

        let (status, list_json) = request_json(&app, Method::GET, "/resources", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list_json["success"], true);
        assert!(list_json["count"].as_u64().unwrap_or(0) >= 1);
        let first_uri = list_json["resources"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|item| item["uri"].as_str())
            .expect("resources list should contain at least one uri")
            .to_string();

        let read_path = format!("/resources/{}", percent_encode_path(&first_uri));
        let (status, read_json) = request_json(&app, Method::GET, &read_path, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(read_json["uri"], first_uri);
        assert!(read_json["content"].is_array());
        assert!(read_json["mimeType"]
            .as_str()
            .is_some_and(|mime| mime.contains("json")));
    }

    #[tokio::test]
    async fn test_streamable_http_resources_contract_and_error_semantics() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let config = McpServerConfig::default();
        let server = McpServer::new(config);
        server
            .register_builtin_resources()
            .await
            .expect("register builtin resources");
        let app = server.create_router();

        let (status, list_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "resources-list-1",
                "method": "resources/list"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list_json["jsonrpc"], "2.0");
        assert_eq!(list_json["id"], "resources-list-1");
        assert!(list_json["result"]["count"].as_u64().unwrap_or(0) >= 1);
        let listed_uris = list_json["result"]["resources"]
            .as_array()
            .expect("resources/list should return resources array")
            .iter()
            .filter_map(|item| item["uri"].as_str())
            .map(|uri| uri.to_string())
            .collect::<std::collections::HashSet<_>>();
        for required_uri in [
            "xenobot://server/info",
            "xenobot://server/capabilities",
            "xenobot://server/integrations",
        ] {
            assert!(
                listed_uris.contains(required_uri),
                "missing required builtin resource URI: {required_uri}"
            );
        }
        let resource_uri = list_json["result"]["resources"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|item| item["uri"].as_str())
            .expect("resources/list should return at least one resource")
            .to_string();

        let (status, read_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "resources-read-1",
                "method": "resources/read",
                "params": {
                    "uri": resource_uri
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(read_json["jsonrpc"], "2.0");
        assert_eq!(read_json["id"], "resources-read-1");
        assert!(read_json["result"]["content"].is_array());
        assert!(read_json["result"]["mimeType"]
            .as_str()
            .is_some_and(|mime| mime.contains("json")));

        let (status, missing_param_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "resources-read-2",
                "method": "resources/read",
                "params": {}
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(missing_param_json["error"]["code"], -32602);
        assert_eq!(missing_param_json["error"]["message"], "invalid_params");

        let (status, missing_resource_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "resources-read-3",
                "method": "resources/read",
                "params": {
                    "uri": "xenobot://missing/resource"
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(missing_resource_json["error"]["code"], -32003);
        assert_eq!(
            missing_resource_json["error"]["message"],
            "resource_not_found"
        );
    }

    #[tokio::test]
    async fn test_streamable_http_alias_methods_for_tools_and_resources() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let db_path = unique_db_path("streamable_alias_methods");
        let previous_db = std::env::var("XENOBOT_DB_PATH").ok();
        std::env::set_var("XENOBOT_DB_PATH", &db_path);
        seed_chat_records_fixture(&db_path);

        let config = McpServerConfig::default();
        let server = McpServer::new(config);
        server
            .register_builtin_tools()
            .await
            .expect("register builtin tools");
        server
            .register_builtin_resources()
            .await
            .expect("register builtin resources");
        let app = server.create_router();

        let (status, tool_list_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "alias-tool-list",
                "method": "tool/list"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(tool_list_json["result"]["tools"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(|item| item["name"] == "get_current_time")));

        let (status, tool_call_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "alias-tool-call",
                "method": "tool/call",
                "params": {
                    "name": "get_current_time",
                    "arguments": {}
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(tool_call_json["result"]["tool"], "get_current_time");
        assert!(tool_call_json["result"]["structuredContent"]["unix"].is_number());

        let (status, nested_tool_call_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "alias-tool-call-nested",
                "method": "tool/call",
                "params": {
                    "tool": {
                        "name": "query_chats",
                        "arguments": {
                            "limit": 5
                        }
                    }
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(nested_tool_call_json["result"]["tool"], "query_chats");
        assert!(nested_tool_call_json["result"]["structuredContent"]["count"].is_number());

        let (status, resource_list_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "alias-resource-list",
                "method": "resource/list"
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let resource_uri = resource_list_json["result"]["resources"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|item| item["uri"].as_str())
            .expect("resource/list alias should return at least one resource")
            .to_string();

        let (status, resource_read_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "alias-resource-read",
                "method": "resource/read",
                "params": {
                    "uri": resource_uri
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(resource_read_json["result"]["content"].is_array());
        assert!(resource_read_json["result"]["mimeType"]
            .as_str()
            .is_some_and(|mime| mime.contains("json")));

        let (status, nested_resource_read_json) = request_json(
            &app,
            Method::POST,
            "/mcp",
            Some(serde_json::json!({
                "jsonrpc": "2.0",
                "id": "alias-resource-read-nested",
                "method": "resource/read",
                "params": {
                    "resource": {
                        "uri": resource_uri
                    }
                }
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(nested_resource_read_json["result"]["content"].is_array());
        assert!(nested_resource_read_json["result"]["mimeType"]
            .as_str()
            .is_some_and(|mime| mime.contains("json")));

        if let Some(previous) = previous_db {
            std::env::set_var("XENOBOT_DB_PATH", previous);
        } else {
            std::env::remove_var("XENOBOT_DB_PATH");
        }
        let _ = std::fs::remove_file(&db_path);
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

    #[test]
    fn test_build_http_base_url_maps_wildcard_bind_to_loopback() {
        let mut config = McpServerConfig::default();
        config.bind_address = "0.0.0.0".to_string();
        config.port = 19191;

        let base_url = build_http_base_url(&config);
        assert_eq!(base_url, "http://127.0.0.1:19191");
    }

    #[test]
    fn test_build_integration_preset_uses_custom_bind_and_alias_target() {
        let mut config = McpServerConfig::default();
        config.bind_address = "192.168.56.10".to_string();
        config.port = 18081;

        let preset =
            build_integration_preset(&config, "claude").expect("claude alias preset should exist");
        assert_eq!(preset.id, "claude-desktop");
        assert_eq!(preset.transport["sse"], "http://192.168.56.10:18081/sse");
        assert_eq!(
            preset.configuration["mcpServers"]["xenobot"]["args"][2],
            "http://192.168.56.10:18081/sse"
        );
    }
}

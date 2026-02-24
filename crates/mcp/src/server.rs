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
            "list_sessions".to_string(),
            Box::new(|args| {
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
                    items.push(row.map_err(|e| {
                        McpError::Tool(format!("read list_sessions row failed: {e}"))
                    })?);
                }
                Ok(serde_json::json!({
                    "count": items.len(),
                    "sessions": items,
                }))
            }),
        )
        .await?;

        self.register_tool(
            "list_contacts".to_string(),
            Box::new(|args| {
                let limit = parse_limit(&args, 50, 500)?;
                let session_id = args.get("session_id").and_then(|v| v.as_i64());
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
            }),
        )
        .await?;

        self.register_tool(
            "recent_messages".to_string(),
            Box::new(|args| {
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
            }),
        )
        .await?;

        self.register_tool(
            "search_messages".to_string(),
            Box::new(|args| {
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
            }),
        )
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

fn parse_limit(args: &Value, default_limit: i64, max_limit: i64) -> Result<i64> {
    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(default_limit);
    Ok(limit.max(1).min(max_limit))
}

fn parse_required_i64(args: &Value, key: &str) -> Result<i64> {
    args.get(key)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| McpError::Argument(format!("missing required integer field: {key}")))
}

fn parse_keyword(args: &Value) -> Result<String> {
    if let Some(keyword) = args
        .get("keyword")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        return Ok(keyword);
    }

    if let Some(keyword) = args
        .get("keywords")
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
                        "command": "npx",
                        "args": ["-y", "mcp-remote", sse_url]
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
            Ok(result) => (StatusCode::OK, Json(result)).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response(),
        },
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": format!("Tool '{}' not found", tool_name) })),
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

    #[tokio::test]
    async fn test_server_creation() {
        let config = McpServerConfig::default();
        let server = McpServer::new(config);
        assert!(server.tools.read().await.is_empty());
        assert!(server.resources.read().await.is_empty());
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

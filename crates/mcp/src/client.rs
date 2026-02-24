//! MCP client implementation.

use crate::config::McpClientConfig;
use crate::error::{McpError, Result};
use crate::protocol::{
    ClientCapabilities, ClientInfo, Content, InitializeRequest, InitializeResponse, McpMessage,
    Notification, NotificationLevel, ResourceCapabilities, ResourceSubscribeRequest,
    ResourceUpdate, ServerInfo, ToolCallRequest, ToolCallResult, ToolCapabilities, ToolResult,
};
use futures_util::{SinkExt, StreamExt};
use reqwest::Client as HttpClient;
use serde_json::Value;
use std::collections::VecDeque;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream,
};

#[derive(Debug, Clone)]
enum ResponseMatcher {
    Initialize,
    ToolCall { call_id: String },
    Any,
}

impl ResponseMatcher {
    fn from_request(message: &McpMessage) -> Self {
        match message {
            McpMessage::InitializeRequest(_) => Self::Initialize,
            McpMessage::ToolCallRequest(request) => Self::ToolCall {
                call_id: request.id.clone(),
            },
            _ => Self::Any,
        }
    }

    fn matches(&self, message: &McpMessage) -> bool {
        match self {
            Self::Initialize => {
                matches!(
                    message,
                    McpMessage::InitializeResponse(_) | McpMessage::ErrorResponse(_)
                )
            }
            Self::ToolCall { call_id } => match message {
                McpMessage::ToolCallResult(result) => result.call_id == *call_id,
                McpMessage::ErrorResponse(_) => true,
                _ => false,
            },
            Self::Any => true,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Initialize => "initialize response",
            Self::ToolCall { .. } => "tool call response",
            Self::Any => "response",
        }
    }
}

/// MCP client for connecting to MCP servers.
pub struct McpClient {
    /// Client configuration.
    config: McpClientConfig,
    /// WebSocket connection (if using WebSocket transport).
    ws_connection: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    /// HTTP client used for Streamable HTTP transport.
    http_client: Option<HttpClient>,
    /// HTTP base URL used for SSE/HTTP transport.
    http_base_url: Option<String>,
    /// Whether the client is initialized.
    initialized: bool,
    /// Server capabilities (after initialization).
    server_capabilities: Option<crate::protocol::ServerCapabilities>,
    /// Next message ID.
    next_message_id: u64,
    /// Inbound messages buffered while waiting for a different response.
    pending_messages: VecDeque<McpMessage>,
}

impl McpClient {
    /// Create a new MCP client with the given configuration.
    pub fn new(config: McpClientConfig) -> Self {
        Self {
            config,
            ws_connection: None,
            http_client: None,
            http_base_url: None,
            initialized: false,
            server_capabilities: None,
            next_message_id: 1,
            pending_messages: VecDeque::new(),
        }
    }

    /// Connect to the MCP server.
    pub async fn connect(&mut self) -> Result<()> {
        let url = self.config.server_url.clone();

        // Determine transport based on URL scheme
        if url.starts_with("ws://") || url.starts_with("wss://") {
            self.connect_websocket(&url).await
        } else if url.starts_with("http://") || url.starts_with("https://") {
            // Try SSE first, then fall back to Streamable HTTP
            self.connect_sse(&url).await
        } else {
            Err(McpError::Config(format!("Unsupported URL scheme: {}", url)))
        }
    }

    /// Connect using WebSocket transport.
    async fn connect_websocket(&mut self, url: &str) -> Result<()> {
        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| McpError::Network(format!("WebSocket connection failed: {}", e)))?;

        self.ws_connection = Some(ws_stream);
        self.http_client = None;
        self.http_base_url = None;
        Ok(())
    }

    /// Connect using SSE transport.
    async fn connect_sse(&mut self, url: &str) -> Result<()> {
        let base_url = normalize_http_base_url(url);
        let client = HttpClient::builder()
            .connect_timeout(Duration::from_secs(self.config.connection_timeout))
            .timeout(Duration::from_secs(self.config.request_timeout))
            .build()
            .map_err(|e| McpError::Config(format!("Failed to build HTTP client: {}", e)))?;

        // Probe SSE endpoint first.
        let sse_url = format!("{}/sse", base_url);
        if let Ok(response) = self.with_default_headers(client.get(&sse_url)).send().await {
            if response.status().is_success() {
                self.http_client = Some(client);
                self.http_base_url = Some(base_url);
                self.ws_connection = None;
                return Ok(());
            }
        }

        // Fallback probe for Streamable HTTP mode.
        let health_url = format!("{}/health", base_url);
        let response = self
            .with_default_headers(client.get(&health_url))
            .send()
            .await
            .map_err(|e| McpError::Network(format!("HTTP connection failed: {}", e)))?;
        if !response.status().is_success() {
            return Err(McpError::Network(format!(
                "HTTP probe failed with status {}",
                response.status()
            )));
        }

        self.http_client = Some(client);
        self.http_base_url = Some(base_url);
        self.ws_connection = None;
        Ok(())
    }

    /// Initialize the client with the server.
    pub async fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        let request = InitializeRequest {
            protocol_version: crate::protocol::MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Some(ClientInfo {
                name: self.config.name.clone(),
                version: self.config.version.clone(),
            }),
        };

        let response_msg = self
            .send_message(McpMessage::InitializeRequest(request))
            .await?;

        match response_msg {
            McpMessage::InitializeResponse(response) => {
                self.initialized = true;
                self.server_capabilities = Some(response.capabilities);
                Ok(())
            }
            McpMessage::ErrorResponse(err) => Err(McpError::Protocol(format!(
                "Initialization failed: {} - {}",
                err.code, err.message
            ))),
            _ => Err(McpError::Protocol(
                "Unexpected response type during initialization".to_string(),
            )),
        }
    }

    /// Send a message to the server and wait for response.
    pub async fn send_message(&mut self, message: McpMessage) -> Result<McpMessage> {
        if self.ws_connection.is_some() {
            self.send_message_ws(message).await
        } else if self.http_base_url.is_some() {
            self.send_message_http(message).await
        } else {
            Err(McpError::Client("Not connected".to_string()))
        }
    }

    async fn send_message_ws(&mut self, message: McpMessage) -> Result<McpMessage> {
        let matcher = ResponseMatcher::from_request(&message);
        self.send_raw_message(&message).await?;

        // Resource subscriptions are asynchronous on WebSocket transport.
        if matches!(message, McpMessage::ResourceSubscribeRequest(_)) {
            return Ok(McpMessage::Notification(Notification {
                level: NotificationLevel::Info,
                message: "resource subscription request sent".to_string(),
                data: None,
            }));
        }

        self.wait_for_matching_response(matcher).await
    }

    async fn send_message_http(&mut self, message: McpMessage) -> Result<McpMessage> {
        match message {
            McpMessage::InitializeRequest(_) => self.initialize_over_http().await,
            McpMessage::ToolCallRequest(request) => self.call_tool_over_http(request).await,
            McpMessage::ResourceSubscribeRequest(request) => {
                self.read_resource_over_http(request).await
            }
            _ => Err(McpError::Unsupported(
                "This message type is only supported over WebSocket transport".to_string(),
            )),
        }
    }

    async fn wait_for_matching_response(&mut self, matcher: ResponseMatcher) -> Result<McpMessage> {
        if let Some(position) = self
            .pending_messages
            .iter()
            .position(|message| matcher.matches(message))
        {
            if let Some(message) = self.pending_messages.remove(position) {
                return Ok(message);
            }
        }

        let timeout = Duration::from_secs(self.config.request_timeout);
        tokio::time::timeout(timeout, async {
            loop {
                let message = self.receive_transport_message().await?;
                if matcher.matches(&message) {
                    return Ok(message);
                }
                self.pending_messages.push_back(message);
            }
        })
        .await
        .map_err(|_| McpError::Timeout(format!("Timed out waiting for {}", matcher.label())))?
    }

    async fn initialize_over_http(&self) -> Result<McpMessage> {
        let client = self.http_client()?;
        let base_url = self.http_base_url()?;
        let tools_url = format!("{}/tools", base_url);

        let tools_supported = match self
            .with_default_headers(client.get(&tools_url))
            .send()
            .await
        {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        };

        let response = InitializeResponse {
            protocol_version: crate::protocol::MCP_PROTOCOL_VERSION.to_string(),
            capabilities: crate::protocol::ServerCapabilities {
                tools: Some(ToolCapabilities {
                    supported: tools_supported,
                    list_changed: Some(false),
                }),
                resources: Some(ResourceCapabilities {
                    supported: true,
                    list_changed: Some(false),
                    subscribe: Some(true),
                }),
                roots: None,
            },
            server_info: Some(ServerInfo {
                name: "Xenobot MCP HTTP Gateway".to_string(),
                version: self.config.version.clone(),
            }),
            instructions: Some(
                "Connected using HTTP transport with optional SSE notifications".to_string(),
            ),
        };

        Ok(McpMessage::InitializeResponse(response))
    }

    async fn call_tool_over_http(&self, request: ToolCallRequest) -> Result<McpMessage> {
        let client = self.http_client()?;
        let base_url = self.http_base_url()?;
        let tool_url = format!("{}/tools/{}", base_url, request.name);
        let args = request
            .arguments
            .unwrap_or_else(|| Value::Object(serde_json::Map::new()));

        let response = self
            .with_default_headers(client.post(&tool_url))
            .json(&args)
            .send()
            .await
            .map_err(|e| McpError::Network(format!("HTTP tool call failed: {}", e)))?;

        let call_id = request.id;
        if response.status().is_success() {
            let payload = response.json::<Value>().await.map_err(|e| {
                McpError::Serialization(format!("Failed to deserialize tool response: {}", e))
            })?;

            return Ok(McpMessage::ToolCallResult(ToolCallResult {
                call_id,
                result: ToolResult::Success {
                    content: vec![Content::Text {
                        text: serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()),
                    }],
                    metadata: None,
                },
            }));
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Ok(McpMessage::ToolCallResult(ToolCallResult {
            call_id,
            result: ToolResult::Error {
                code: format!("HTTP_{}", status.as_u16()),
                message: if body.is_empty() {
                    format!("Tool call failed with status {}", status)
                } else {
                    format!("Tool call failed with status {}: {}", status, body)
                },
                details: None,
            },
        }))
    }

    async fn read_resource_over_http(
        &self,
        request: ResourceSubscribeRequest,
    ) -> Result<McpMessage> {
        let client = self.http_client()?;
        let base_url = self.http_base_url()?;
        let resource_path = request.uri.trim_start_matches('/');
        let resource_url = format!("{}/resources/{}", base_url, resource_path);

        let response = self
            .with_default_headers(client.get(&resource_url))
            .send()
            .await
            .map_err(|e| McpError::Network(format!("HTTP resource request failed: {}", e)))?;

        if response.status().is_success() {
            let payload = response.json::<Value>().await.map_err(|e| {
                McpError::Serialization(format!("Failed to deserialize resource response: {}", e))
            })?;
            return Ok(McpMessage::ResourceUpdate(ResourceUpdate {
                uri: request.uri,
                content: vec![Content::Text {
                    text: serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string()),
                }],
            }));
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Ok(McpMessage::ErrorResponse(crate::protocol::ErrorResponse {
            code: format!("HTTP_{}", status.as_u16()),
            message: if body.is_empty() {
                format!("Resource request failed with status {}", status)
            } else {
                format!("Resource request failed with status {}: {}", status, body)
            },
            details: None,
        }))
    }

    fn http_client(&self) -> Result<&HttpClient> {
        self.http_client
            .as_ref()
            .ok_or_else(|| McpError::Client("HTTP transport is not connected".to_string()))
    }

    fn http_base_url(&self) -> Result<&str> {
        self.http_base_url
            .as_deref()
            .ok_or_else(|| McpError::Client("HTTP base URL is not set".to_string()))
    }

    fn with_default_headers(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let request = request
            .header("x-client-name", self.config.name.as_str())
            .header("x-client-version", self.config.version.as_str());

        if let Some(token) = &self.config.auth_token {
            request.bearer_auth(token)
        } else {
            request
        }
    }

    /// Send a raw message without waiting for response.
    pub async fn send_raw_message(&mut self, message: &McpMessage) -> Result<()> {
        let json = serde_json::to_string(message)
            .map_err(|e| McpError::Serialization(format!("Failed to serialize message: {}", e)))?;

        if let Some(ws) = &mut self.ws_connection {
            ws.send(Message::Text(json)).await.map_err(|e| {
                McpError::Network(format!("Failed to send WebSocket message: {}", e))
            })?;
            Ok(())
        } else if self.http_base_url.is_some() {
            Err(McpError::Unsupported(
                "Raw message sending is only supported over WebSocket transport".to_string(),
            ))
        } else {
            Err(McpError::Client("Not connected".to_string()))
        }
    }

    /// Receive a message from the server.
    pub async fn receive_message(&mut self) -> Result<McpMessage> {
        if let Some(message) = self.pending_messages.pop_front() {
            return Ok(message);
        }
        self.receive_transport_message().await
    }

    async fn receive_transport_message(&mut self) -> Result<McpMessage> {
        if let Some(ws) = &mut self.ws_connection {
            loop {
                let message = ws
                    .next()
                    .await
                    .ok_or_else(|| McpError::Network("Connection closed".to_string()))?
                    .map_err(|e| McpError::Network(format!("Failed to receive message: {}", e)))?;

                match message {
                    Message::Text(text) => {
                        let parsed = serde_json::from_str(&text).map_err(|e| {
                            McpError::Serialization(format!("Failed to deserialize message: {}", e))
                        })?;
                        return Ok(parsed);
                    }
                    Message::Close(_) => {
                        return Err(McpError::Network("Connection closed by server".to_string()));
                    }
                    Message::Ping(_) | Message::Pong(_) => {
                        continue;
                    }
                    _ => {
                        continue;
                    }
                }
            }
        }

        if self.http_base_url.is_some() {
            return Err(McpError::Unsupported(
                "Receiving asynchronous push messages is only supported over WebSocket transport"
                    .to_string(),
            ));
        }

        Err(McpError::Client("Not connected".to_string()))
    }

    /// Call a tool on the server.
    pub async fn call_tool(
        &mut self,
        name: String,
        arguments: Option<Value>,
    ) -> Result<ToolCallResult> {
        let request = ToolCallRequest {
            id: self.next_message_id().to_string(),
            name,
            arguments,
        };

        let response_msg = self
            .send_message(McpMessage::ToolCallRequest(request))
            .await?;

        match response_msg {
            McpMessage::ToolCallResult(result) => Ok(result),
            McpMessage::ErrorResponse(err) => Err(McpError::Tool(format!(
                "Tool call failed: {} - {}",
                err.code, err.message
            ))),
            _ => Err(McpError::Protocol(
                "Unexpected response type for tool call".to_string(),
            )),
        }
    }

    /// Subscribe to a resource.
    pub async fn subscribe_resource(&mut self, uri: String) -> Result<()> {
        let request = ResourceSubscribeRequest { uri };
        if self.ws_connection.is_some() {
            self.send_raw_message(&McpMessage::ResourceSubscribeRequest(request))
                .await?;
            return Ok(());
        }

        let response = self
            .send_message(McpMessage::ResourceSubscribeRequest(request))
            .await?;
        match response {
            McpMessage::ResourceUpdate(update) => {
                self.pending_messages
                    .push_back(McpMessage::ResourceUpdate(update));
                Ok(())
            }
            McpMessage::ErrorResponse(err) => Err(McpError::Resource(format!(
                "Resource subscription failed: {} - {}",
                err.code, err.message
            ))),
            _ => Ok(()),
        }
    }

    /// Poll one SSE event for HTTP transport and cache messages for later reads.
    pub async fn poll_sse_once(&mut self) -> Result<Option<McpMessage>> {
        let client = match self.http_client.as_ref() {
            Some(client) => client,
            None => return Ok(None),
        };
        let base_url = match self.http_base_url.as_deref() {
            Some(url) => url,
            None => return Ok(None),
        };

        let response = self
            .with_default_headers(client.get(format!("{}/sse", base_url)))
            .send()
            .await
            .map_err(|e| McpError::Network(format!("SSE request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(McpError::Network(format!(
                "SSE endpoint returned status {}",
                response.status()
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| McpError::Network(format!("Failed to read SSE payload: {}", e)))?;

        if let Some(message) = parse_first_sse_message(&body)? {
            self.pending_messages.push_back(message.clone());
            return Ok(Some(message));
        }

        Ok(None)
    }

    /// Receive resource updates (non-blocking).
    pub async fn receive_resource_update(&mut self) -> Result<Option<ResourceUpdate>> {
        let msg = self.receive_message().await?;
        match msg {
            McpMessage::ResourceUpdate(update) => Ok(Some(update)),
            _ => Ok(None),
        }
    }

    /// Receive notifications (non-blocking).
    pub async fn receive_notification(&mut self) -> Result<Option<Notification>> {
        let msg = self.receive_message().await?;
        match msg {
            McpMessage::Notification(notification) => Ok(Some(notification)),
            _ => Ok(None),
        }
    }

    /// Disconnect from the server.
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(mut ws) = self.ws_connection.take() {
            ws.close(None)
                .await
                .map_err(|e| McpError::Network(format!("Failed to close connection: {}", e)))?;
        }
        self.http_client = None;
        self.http_base_url = None;
        self.pending_messages.clear();
        self.initialized = false;
        self.server_capabilities = None;
        Ok(())
    }

    /// Get next message ID.
    fn next_message_id(&mut self) -> u64 {
        let id = self.next_message_id;
        self.next_message_id += 1;
        id
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if self.ws_connection.is_some() || self.http_base_url.is_some() {
            eprintln!("Warning: McpClient dropped without explicit disconnect");
        }
    }
}

fn normalize_http_base_url(url: &str) -> String {
    let mut base = url.trim_end_matches('/').to_string();
    if base.ends_with("/sse") {
        base.truncate(base.len() - 4);
    } else if base.ends_with("/ws") {
        base.truncate(base.len() - 3);
    }
    base
}

fn parse_first_sse_message(payload: &str) -> Result<Option<McpMessage>> {
    for event in payload.split("\n\n") {
        let mut data = String::new();
        for line in event.lines() {
            if let Some(rest) = line.strip_prefix("data:") {
                data.push_str(rest.trim());
            }
        }

        if data.is_empty() {
            continue;
        }

        if data == "{\"type\":\"ping\"}" || data == "{\"status\":\"ok\"}" {
            continue;
        }

        let message = serde_json::from_str::<McpMessage>(&data).map_err(|e| {
            McpError::Serialization(format!("Failed to parse SSE message payload: {}", e))
        })?;
        return Ok(Some(message));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation() {
        let config = McpClientConfig::default();
        let client = McpClient::new(config);
        assert!(!client.initialized);
        assert!(client.server_capabilities.is_none());
    }
}

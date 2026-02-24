//! Model Context Protocol (MCP) implementation for Xenobot.
//!
//! This module provides the core protocol definitions for MCP, supporting
//! both Streamable HTTP and SSE (Server-Sent Events) protocols as used in chatlog.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// MCP protocol version.
pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// MCP message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpMessage {
    /// Initialization message from client.
    InitializeRequest(InitializeRequest),
    /// Response to initialization.
    InitializeResponse(InitializeResponse),
    /// Tool invocation request.
    ToolCallRequest(ToolCallRequest),
    /// Tool invocation result.
    ToolCallResult(ToolCallResult),
    /// Resource subscription request.
    ResourceSubscribeRequest(ResourceSubscribeRequest),
    /// Resource update notification.
    ResourceUpdate(ResourceUpdate),
    /// Notification message.
    Notification(Notification),
    /// Error response.
    ErrorResponse(ErrorResponse),
}

/// Initialize request from client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeRequest {
    /// Protocol version.
    pub protocol_version: String,
    /// Client capabilities.
    pub capabilities: ClientCapabilities,
    /// Optional client metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_info: Option<ClientInfo>,
}

/// Initialize response from server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResponse {
    /// Protocol version.
    pub protocol_version: String,
    /// Server capabilities.
    pub capabilities: ServerCapabilities,
    /// Server metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_info: Option<ServerInfo>,
    /// Available instructions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

/// Tool call request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    /// Unique ID for this tool call.
    pub id: String,
    /// Name of the tool to invoke.
    pub name: String,
    /// Arguments for the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// Tool call result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// ID of the tool call this result corresponds to.
    pub call_id: String,
    /// Tool call result.
    #[serde(flatten)]
    pub result: ToolResult,
}

/// Tool result variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResult {
    /// Successful tool execution.
    Success {
        /// Result content.
        content: Vec<Content>,
        /// Optional metadata.
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<HashMap<String, serde_json::Value>>,
    },
    /// Tool execution error.
    Error {
        /// Error code.
        code: String,
        /// Error message.
        message: String,
        /// Optional error details.
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<serde_json::Value>,
    },
}

/// Content in tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    /// Text content.
    Text {
        /// The text content.
        text: String,
    },
    /// Image content.
    Image {
        /// Image data (base64 encoded).
        data: String,
        /// MIME type.
        mime_type: String,
    },
    /// Resource reference.
    Resource {
        /// Resource URI.
        uri: String,
        /// Resource text.
        text: String,
        /// Optional MIME type.
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
}

/// Resource subscription request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSubscribeRequest {
    /// Resource URI to subscribe to.
    pub uri: String,
}

/// Resource update notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUpdate {
    /// Updated resource URI.
    pub uri: String,
    /// Resource content.
    pub content: Vec<Content>,
}

/// Notification message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// Notification level.
    pub level: NotificationLevel,
    /// Notification message.
    pub message: String,
    /// Optional notification data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Notification level.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NotificationLevel {
    /// Information notification.
    Info,
    /// Warning notification.
    Warning,
    /// Error notification.
    Error,
}

/// Error response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error code.
    pub code: String,
    /// Error message.
    pub message: String,
    /// Optional error details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Client capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientCapabilities {
    /// Supported tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolCapabilities>,
    /// Supported resources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceCapabilities>,
    /// Supported roots.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<RootCapabilities>,
}

/// Server capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerCapabilities {
    /// Available tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolCapabilities>,
    /// Available resources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceCapabilities>,
    /// Available roots.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<RootCapabilities>,
}

/// Tool capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolCapabilities {
    /// Whether tools are supported.
    pub supported: bool,
    /// Whether tools can be listed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Resource capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceCapabilities {
    /// Whether resources are supported.
    pub supported: bool,
    /// Whether resources can be listed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
    /// Subscribe support.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,
}

/// Root capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RootCapabilities {
    /// Whether roots are supported.
    pub supported: bool,
    /// Whether roots can be listed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Client information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Client name.
    pub name: String,
    /// Client version.
    pub version: String,
}

/// Server information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    /// Server name.
    pub name: String,
    /// Server version.
    pub version: String,
}

/// Tool definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// Tool input schema (JSON Schema).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
}

/// Resource definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    /// Resource URI.
    pub uri: String,
    /// Resource name.
    pub name: String,
    /// Resource description.
    pub description: String,
    /// Resource MIME type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Root definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootDefinition {
    /// Root URI.
    pub uri: String,
    /// Root name.
    pub name: String,
}

impl McpMessage {
    /// Create a new initialization request.
    pub fn initialize_request(
        protocol_version: String,
        capabilities: ClientCapabilities,
        client_info: Option<ClientInfo>,
    ) -> Self {
        McpMessage::InitializeRequest(InitializeRequest {
            protocol_version,
            capabilities,
            client_info,
        })
    }

    /// Create a new initialization response.
    pub fn initialize_response(
        protocol_version: String,
        capabilities: ServerCapabilities,
        server_info: Option<ServerInfo>,
        instructions: Option<String>,
    ) -> Self {
        McpMessage::InitializeResponse(InitializeResponse {
            protocol_version,
            capabilities,
            server_info,
            instructions,
        })
    }

    /// Create a new tool call request.
    pub fn tool_call_request(
        id: String,
        name: String,
        arguments: Option<serde_json::Value>,
    ) -> Self {
        McpMessage::ToolCallRequest(ToolCallRequest {
            id,
            name,
            arguments,
        })
    }

    /// Create a new successful tool call result.
    pub fn tool_call_success(
        call_id: String,
        content: Vec<Content>,
        metadata: Option<HashMap<String, serde_json::Value>>,
    ) -> Self {
        McpMessage::ToolCallResult(ToolCallResult {
            call_id,
            result: ToolResult::Success { content, metadata },
        })
    }

    /// Create a new error tool call result.
    pub fn tool_call_error(
        call_id: String,
        code: String,
        message: String,
        details: Option<serde_json::Value>,
    ) -> Self {
        McpMessage::ToolCallResult(ToolCallResult {
            call_id,
            result: ToolResult::Error {
                code,
                message,
                details,
            },
        })
    }

    /// Create a new error response.
    pub fn error_response(
        code: String,
        message: String,
        details: Option<serde_json::Value>,
    ) -> Self {
        McpMessage::ErrorResponse(ErrorResponse {
            code,
            message,
            details,
        })
    }

    /// Serialize message to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize message from JSON string.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl Default for InitializeRequest {
    fn default() -> Self {
        Self {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: None,
        }
    }
}

impl Default for InitializeResponse {
    fn default() -> Self {
        Self {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ServerCapabilities::default(),
            server_info: None,
            instructions: None,
        }
    }
}

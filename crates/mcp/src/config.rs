//! Configuration for MCP integration.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// MCP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server bind address.
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    /// Server port.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Server name.
    #[serde(default = "default_server_name")]
    pub name: String,

    /// Server version.
    #[serde(default = "default_server_version")]
    pub version: String,

    /// Allowed origins for CORS.
    #[serde(default)]
    pub allowed_origins: Vec<String>,

    /// Authentication token (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,

    /// Maximum message size in bytes.
    #[serde(default = "default_max_message_size")]
    pub max_message_size: usize,

    /// Enable SSE (Server-Sent Events) protocol.
    #[serde(default = "default_enable_sse")]
    pub enable_sse: bool,

    /// Enable Streamable HTTP protocol.
    #[serde(default = "default_enable_streamable_http")]
    pub enable_streamable_http: bool,

    /// Root directories for resources.
    #[serde(default)]
    pub resource_roots: Vec<PathBuf>,

    /// Available tools configuration.
    #[serde(default)]
    pub tools: Vec<ToolConfig>,
}

/// MCP client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClientConfig {
    /// Server URL to connect to.
    pub server_url: String,

    /// Client name.
    #[serde(default = "default_client_name")]
    pub name: String,

    /// Client version.
    #[serde(default = "default_client_version")]
    pub version: String,

    /// Authentication token (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,

    /// Connection timeout in seconds.
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,

    /// Request timeout in seconds.
    #[serde(default = "default_request_timeout")]
    pub request_timeout: u64,

    /// Enable auto-reconnection.
    #[serde(default = "default_auto_reconnect")]
    pub auto_reconnect: bool,

    /// Reconnection delay in seconds.
    #[serde(default = "default_reconnection_delay")]
    pub reconnection_delay: u64,

    /// Maximum reconnection attempts.
    #[serde(default = "default_max_reconnection_attempts")]
    pub max_reconnection_attempts: u32,
}

/// Tool configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Tool name.
    pub name: String,

    /// Tool description.
    pub description: String,

    /// Tool input schema (JSON Schema).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,

    /// Whether the tool is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
            port: default_port(),
            name: default_server_name(),
            version: default_server_version(),
            allowed_origins: Vec::new(),
            auth_token: None,
            max_message_size: default_max_message_size(),
            enable_sse: default_enable_sse(),
            enable_streamable_http: default_enable_streamable_http(),
            resource_roots: Vec::new(),
            tools: Vec::new(),
        }
    }
}

impl Default for McpClientConfig {
    fn default() -> Self {
        Self {
            server_url: String::from("http://localhost:5030"),
            name: default_client_name(),
            version: default_client_version(),
            auth_token: None,
            connection_timeout: default_connection_timeout(),
            request_timeout: default_request_timeout(),
            auto_reconnect: default_auto_reconnect(),
            reconnection_delay: default_reconnection_delay(),
            max_reconnection_attempts: default_max_reconnection_attempts(),
        }
    }
}

// Default values
fn default_bind_address() -> String {
    String::from("127.0.0.1")
}

fn default_port() -> u16 {
    5030
}

fn default_server_name() -> String {
    String::from("Xenobot MCP Server")
}

fn default_server_version() -> String {
    String::from("0.1.0")
}

fn default_max_message_size() -> usize {
    10 * 1024 * 1024 // 10 MB
}

fn default_enable_sse() -> bool {
    true
}

fn default_enable_streamable_http() -> bool {
    true
}

fn default_client_name() -> String {
    String::from("Xenobot MCP Client")
}

fn default_client_version() -> String {
    String::from("0.1.0")
}

fn default_connection_timeout() -> u64 {
    30
}

fn default_request_timeout() -> u64 {
    60
}

fn default_auto_reconnect() -> bool {
    true
}

fn default_reconnection_delay() -> u64 {
    5
}

fn default_max_reconnection_attempts() -> u32 {
    10
}

fn default_enabled() -> bool {
    true
}

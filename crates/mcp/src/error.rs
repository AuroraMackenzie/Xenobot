//! Error types for MCP operations.

use thiserror::Error;

/// Main error type for MCP operations.
#[derive(Error, Debug)]
pub enum McpError {
    /// Protocol error - malformed message or invalid state.
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Network error (connection, timeout, etc.)
    #[error("Network error: {0}")]
    Network(String),

    /// Authentication/authorization error.
    #[error("Authentication error: {0}")]
    Authentication(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Resource error (not found, permission denied, etc.)
    #[error("Resource error: {0}")]
    Resource(String),

    /// Tool execution error.
    #[error("Tool execution error: {0}")]
    Tool(String),

    /// Server error.
    #[error("Server error: {0}")]
    Server(String),

    /// Client error.
    #[error("Client error: {0}")]
    Client(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid argument error.
    #[error("Invalid argument: {0}")]
    Argument(String),

    /// Unsupported operation or feature.
    #[error("Unsupported: {0}")]
    Unsupported(String),

    /// Timeout error.
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result alias for MCP operations.
pub type Result<T> = std::result::Result<T, McpError>;

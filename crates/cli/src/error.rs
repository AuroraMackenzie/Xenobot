//! Error types for CLI operations.

use thiserror::Error;

/// Main error type for CLI operations.
#[derive(Error, Debug)]
pub enum CliError {
    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// WeChat operation error.
    #[error("WeChat operation error: {0}")]
    WeChat(String),

    /// API communication error.
    #[error("API communication error: {0}")]
    Api(String),

    /// Database error.
    #[error("Database error: {0}")]
    Database(String),

    /// File system error.
    #[error("File system error: {0}")]
    FileSystem(String),

    /// Network error.
    #[error("Network error: {0}")]
    Network(String),

    /// Analysis error.
    #[error("Analysis error: {0}")]
    Analysis(String),

    /// Command execution error.
    #[error("Command execution error: {0}")]
    Command(String),

    /// Invalid argument error.
    #[error("Invalid argument: {0}")]
    Argument(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Parse error.
    #[error("Parse error: {0}")]
    Parse(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result alias for CLI operations.
pub type Result<T> = std::result::Result<T, CliError>;

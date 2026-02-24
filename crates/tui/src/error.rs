//! Error types for TUI operations.

use thiserror::Error;

/// Main error type for TUI operations.
#[derive(Error, Debug)]
pub enum TuiError {
    /// Terminal initialization error.
    #[error("Terminal initialization error: {0}")]
    TerminalInit(String),

    /// Event handling error.
    #[error("Event handling error: {0}")]
    Event(String),

    /// UI rendering error.
    #[error("UI rendering error: {0}")]
    Render(String),

    /// API communication error.
    #[error("API communication error: {0}")]
    Api(String),

    /// WeChat operation error.
    #[error("WeChat operation error: {0}")]
    WeChat(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result alias for TUI operations.
pub type Result<T> = std::result::Result<T, TuiError>;

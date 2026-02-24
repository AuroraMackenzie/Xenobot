//! Error types for WeChat data extraction and decryption.

use thiserror::Error;

/// Main error type for WeChat operations.
#[derive(Error, Debug)]
pub enum WeChatError {
    /// I/O error (file operations, etc.)
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Platform-specific error (macOS APIs)
    #[error("Platform error: {0}")]
    Platform(String),

    /// No running WeChat instances found
    #[error("No running WeChat instances found")]
    NoInstances,

    /// Failed to resolve encryption key from authorized sources
    #[error("Failed to resolve encryption key: {0}")]
    KeyExtraction(String),

    /// Decryption failure
    #[error("Decryption failed: {0}")]
    Decryption(String),

    /// Invalid configuration
    #[error("Invalid configuration: {0}")]
    Config(String),

    /// File monitoring error
    #[error("File monitoring error: {0}")]
    FileMonitor(#[from] notify::Error),

    /// SQLite database error
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Other errors wrapped in anyhow
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Result alias for WeChat operations.
pub type WeChatResult<T> = Result<T, WeChatError>;

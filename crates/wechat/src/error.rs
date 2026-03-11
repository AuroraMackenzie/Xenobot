//! Error types for WeChat data extraction and decryption.

use thiserror::Error;
use xenobot_analysis::parsers::ParseError;

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

    /// Parse error returned by analysis parser registry.
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),

    /// Path is outside the configured authorized roots.
    #[error("path is outside authorized roots: {path}")]
    UnauthorizedPath {
        /// Rejected source path.
        path: std::path::PathBuf,
    },

    /// Parsed export did not match the expected platform.
    #[error("parsed platform mismatch: expected {expected}, got {actual}")]
    PlatformMismatch {
        /// Expected stable platform identifier.
        expected: String,
        /// Actual parsed platform identifier.
        actual: String,
    },

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

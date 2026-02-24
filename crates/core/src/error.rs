//! Error types for Xenobot core functionality.

use thiserror::Error;

/// Main error type for Xenobot.
#[derive(Error, Debug)]
pub enum Error {
    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Database error: {0}")]
    Database(String),
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Network error.
    #[error("Network error: {0}")]
    Network(String),
    #[error("File system error: {0}")]
    FileSystem(String),
    #[error("Platform error: {0}")]
    Platform(String),
    #[error("Data parsing error: {0}")]
    Parse(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("External dependency error: {0}")]
    External(String),
    #[error("Operation timeout: {0}")]
    Timeout(String),
    #[error("Resource not found: {0}")]
    NotFound(String),
    #[error("Resource already exists: {0}")]
    AlreadyExists(String),
    #[error("Invalid state: {0}")]
    InvalidState(String),
    #[error("Unsupported operation: {0}")]
    Unsupported(String),
    /// Custom error with message.
    #[error("{0}")]
    Custom(String),
}

/// Result type for Xenobot operations.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create a configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a database error
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }

    /// Create a platform error
    pub fn platform(msg: impl Into<String>) -> Self {
        Self::Platform(msg.into())
    }

    /// Create a custom error
    pub fn custom(msg: impl Into<String>) -> Self {
        Self::Custom(msg.into())
    }
}

/// Convenience trait for converting errors to core Error type
pub trait IntoCoreError<T> {
    /// Convert to core error with context
    fn with_context(self, context: &str) -> Result<T>;
}

impl<T, E> IntoCoreError<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn with_context(self, context: &str) -> Result<T> {
        self.map_err(|e| Error::External(format!("{}: {}", context, e)))
    }
}

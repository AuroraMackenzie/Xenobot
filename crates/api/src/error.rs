//! Error types for the Xenobot HTTP API server.

use axum::response::IntoResponse;
use thiserror::Error;

/// Main error type for API operations.
#[derive(Error, Debug)]
pub enum ApiError {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP request/response error.
    #[error("HTTP error: {0}")]
    Http(String),

    /// Database error.
    #[error("Database error: {0}")]
    Database(String),

    /// Authentication/authorization error.
    #[error("Authentication error: {0}")]
    Auth(String),

    /// Invalid request parameters.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Resource not found.
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Internal server error.
    #[error("Internal server error: {0}")]
    Internal(String),

    /// Feature not implemented.
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// WeChat service error.
    #[cfg(feature = "wechat")]
    #[error("WeChat error: {0}")]
    WeChat(#[from] xenobot_wechat::WeChatError),

    /// Core Xenobot error.
    #[error("Core error: {0}")]
    Core(#[from] xenobot_core::Error),
}

/// Result alias for API operations.
pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
    /// Convert to HTTP status code.
    pub fn status_code(&self) -> axum::http::StatusCode {
        match self {
            ApiError::Io(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Json(_) => axum::http::StatusCode::BAD_REQUEST,
            ApiError::Http(_) => axum::http::StatusCode::BAD_GATEWAY,
            ApiError::Database(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Auth(_) => axum::http::StatusCode::UNAUTHORIZED,
            ApiError::InvalidRequest(_) => axum::http::StatusCode::BAD_REQUEST,
            ApiError::NotFound(_) => axum::http::StatusCode::NOT_FOUND,
            ApiError::Internal(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::NotImplemented(_) => axum::http::StatusCode::NOT_IMPLEMENTED,
            #[cfg(feature = "wechat")]
            ApiError::WeChat(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::Core(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl From<ApiError> for axum::response::Response {
    fn from(error: ApiError) -> Self {
        let status = error.status_code();
        let body = serde_json::json!({
            "error": error.to_string(),
            "code": status.as_u16(),
        });
        (status, axum::Json(body)).into_response()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        self.into()
    }
}

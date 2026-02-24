//! Error types for Xenobot web frontend integration.

use axum::response::IntoResponse;
use thiserror::Error;

/// Main error type for web operations.
#[derive(Error, Debug)]
pub enum WebError {
    /// Static file serving error.
    #[error("Static file error: {0}")]
    StaticFile(String),

    /// WebSocket connection error.
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    /// Frontend integration error.
    #[error("Frontend integration error: {0}")]
    FrontendIntegration(String),

    /// API error.
    #[error("API error: {0}")]
    Api(#[from] xenobot_api::ApiError),

    /// Core Xenobot error.
    #[error("Core error: {0}")]
    Core(#[from] xenobot_core::Error),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP request/response error.
    #[error("HTTP error: {0}")]
    Http(String),

    /// Invalid request parameters.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

/// Result alias for web operations.
pub type WebResult<T> = Result<T, WebError>;

impl WebError {
    /// Convert to HTTP status code.
    pub fn status_code(&self) -> axum::http::StatusCode {
        match self {
            WebError::StaticFile(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            WebError::WebSocket(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            WebError::FrontendIntegration(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            WebError::Api(e) => e.status_code(),
            WebError::Core(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            WebError::Io(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            WebError::Json(_) => axum::http::StatusCode::BAD_REQUEST,
            WebError::Http(_) => axum::http::StatusCode::BAD_GATEWAY,
            WebError::InvalidRequest(_) => axum::http::StatusCode::BAD_REQUEST,
        }
    }
}

impl From<WebError> for axum::response::Response {
    fn from(error: WebError) -> Self {
        let status = error.status_code();
        let body = serde_json::json!({
            "error": error.to_string(),
            "code": status.as_u16(),
        });
        (status, axum::Json(body)).into_response()
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> axum::response::Response {
        self.into()
    }
}

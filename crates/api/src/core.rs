//! Core API module for Xenobot HTTP API.
//!
//! Provides HTTP endpoints equivalent to Xenobot's `extendedApi` IPC methods.

use axum::{
    routing::{get, post},
    Json, Router,
};
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tracing::instrument;

use crate::ApiError;

static ANALYTICS_ENABLED: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(true));
static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .expect("failed to build reqwest client")
});

/// Core API router.
pub fn router() -> Router {
    Router::new()
        // Theme
        .route("/theme", post(set_theme_source))
        // Dialog
        .route("/dialog/open", post(show_open_dialog))
        // Clipboard
        .route("/clipboard/copy-image", post(copy_image))
        // App
        .route("/app/version", get(get_version))
        .route("/app/check-update", post(check_update))
        .route("/app/simulate-update", post(simulate_update))
        .route("/app/fetch-remote-config", post(fetch_remote_config))
        .route("/app/analytics-enabled", get(get_analytics_enabled))
        .route("/app/analytics-enabled", post(set_analytics_enabled))
        .route("/app/relaunch", post(relaunch))
}

// ==================== Request/Response Types ====================

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenDialogOptions {
    pub title: Option<String>,
    pub default_path: Option<String>,
    pub button_label: Option<String>,
    pub filters: Option<Vec<serde_json::Value>>,
    pub properties: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenDialogReturnValue {
    pub canceled: bool,
    pub file_paths: Vec<String>,
}

// ==================== Handler Implementations ====================

#[derive(Debug, Deserialize)]
struct SetThemeSourceRequest {
    mode: String,
}

#[instrument]
async fn set_theme_source(Json(req): Json<SetThemeSourceRequest>) -> Result<Json<()>, ApiError> {
    let mode = req.mode.trim().to_ascii_lowercase();
    let valid = matches!(mode.as_str(), "light" | "dark" | "system");
    if !valid {
        return Err(ApiError::InvalidRequest(format!(
            "unsupported theme mode: {}",
            req.mode
        )));
    }
    Ok(Json(()))
}

#[instrument]
async fn show_open_dialog(
    Json(_req): Json<OpenDialogOptions>,
) -> Result<Json<OpenDialogReturnValue>, ApiError> {
    // HTTP backend cannot open native file dialogs. Frontend should use drag/drop
    // or pass explicit file paths.
    Ok(Json(OpenDialogReturnValue {
        canceled: true,
        file_paths: Vec::new(),
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CopyImageRequest {
    data_url: String,
}

#[instrument]
async fn copy_image(
    Json(req): Json<CopyImageRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.data_url.trim().is_empty() {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "empty_data_url"
        })));
    }
    Ok(Json(serde_json::json!({
        "success": false,
        "error": "clipboard_copy_not_supported_in_http_mode"
    })))
}

#[instrument]
async fn get_version() -> Result<Json<String>, ApiError> {
    let version = env!("CARGO_PKG_VERSION");
    let commit = option_env!("API_GIT_COMMIT").unwrap_or_default().trim();
    if commit.is_empty() {
        Ok(Json(version.to_string()))
    } else {
        Ok(Json(format!("{}+{}", version, commit)))
    }
}

#[instrument]
async fn check_update() -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "update_check_not_enabled_in_http_mode",
        "checkedAt": chrono::Utc::now().to_rfc3339(),
    })))
}

#[instrument]
async fn simulate_update() -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "simulate_update_noop",
        "time": chrono::Utc::now().to_rfc3339(),
    })))
}

#[derive(Debug, Deserialize)]
struct FetchRemoteConfigRequest {
    url: String,
}

#[instrument]
async fn fetch_remote_config(
    Json(req): Json<FetchRemoteConfigRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let url = req.url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(ApiError::InvalidRequest(
            "invalid remote config URL".to_string(),
        ));
    }

    let response = HTTP_CLIENT
        .get(url)
        .send()
        .await
        .map_err(|e| ApiError::Http(e.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": format!("HTTP {}", status.as_u16()),
        })));
    }

    let body = response
        .text()
        .await
        .map_err(|e| ApiError::Http(e.to_string()))?;
    let data: serde_json::Value = serde_json::from_str(&body).map_err(|e| ApiError::Json(e))?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": data,
    })))
}

#[instrument]
async fn get_analytics_enabled() -> Result<Json<bool>, ApiError> {
    Ok(Json(ANALYTICS_ENABLED.load(Ordering::Relaxed)))
}

#[derive(Debug, Deserialize)]
struct SetAnalyticsEnabledRequest {
    enabled: bool,
}

#[instrument]
async fn set_analytics_enabled(
    Json(req): Json<SetAnalyticsEnabledRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    ANALYTICS_ENABLED.store(req.enabled, Ordering::Relaxed);
    Ok(Json(serde_json::json!({
        "success": true,
        "enabled": req.enabled,
    })))
}

#[instrument]
async fn relaunch() -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({
        "success": false,
        "message": "relaunch_not_supported_in_http_mode",
    })))
}

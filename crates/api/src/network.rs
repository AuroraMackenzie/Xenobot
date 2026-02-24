//! Network API module for Xenobot HTTP API.
//!
//! Provides HTTP endpoints equivalent to Xenobot's `networkApi` IPC methods.

use axum::{
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, time::Duration};
use tracing::instrument;

use crate::ApiError;

/// Network API router.
pub fn router() -> Router {
    Router::new()
        .route("/proxy-config", get(get_proxy_config))
        .route("/proxy-config", post(save_proxy_config))
        .route("/test-proxy-connection", post(test_proxy_connection))
}

// Request/Response types

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfig {
    pub enabled: bool,
    pub mode: Option<String>,
    pub url: Option<String>,
    pub bypass_list: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProxyConfigRequest {
    pub config: Option<ProxyConfig>,
    pub mode: Option<String>,
    pub url: Option<String>,
    pub bypass_list: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestProxyConnectionRequest {
    pub proxy_url: Option<String>,
    pub proxy_url_camel: Option<String>,
}

// Handler functions

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProxyConfigStore {
    mode: String,
    url: Option<String>,
    bypass_list: Vec<String>,
}

impl Default for ProxyConfigStore {
    fn default() -> Self {
        Self {
            mode: "system".to_string(),
            url: None,
            bypass_list: Vec::new(),
        }
    }
}

fn config_dir() -> Result<PathBuf, ApiError> {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn proxy_config_path() -> Result<PathBuf, ApiError> {
    Ok(config_dir()?.join("network_proxy.json"))
}

fn normalize_mode(mode: Option<&str>, enabled: Option<bool>, has_url: bool) -> String {
    if let Some(mode) = mode {
        let mode = mode.trim().to_ascii_lowercase();
        if matches!(mode.as_str(), "off" | "system" | "manual") {
            return mode;
        }
    }
    if enabled.unwrap_or(false) || has_url {
        "manual".to_string()
    } else {
        "system".to_string()
    }
}

fn read_proxy_store() -> Result<ProxyConfigStore, ApiError> {
    let path = proxy_config_path()?;
    if !path.exists() {
        return Ok(ProxyConfigStore::default());
    }

    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(ProxyConfigStore::default());
    }

    serde_json::from_str(&raw).map_err(ApiError::Json)
}

fn write_proxy_store(store: &ProxyConfigStore) -> Result<(), ApiError> {
    let path = proxy_config_path()?;
    let content = serde_json::to_string_pretty(store)?;
    fs::write(path, content)?;
    Ok(())
}

fn build_api_proxy_config(store: ProxyConfigStore) -> ProxyConfig {
    ProxyConfig {
        enabled: store.mode == "manual",
        mode: Some(store.mode),
        url: store.url,
        bypass_list: store.bypass_list,
    }
}

#[axum::debug_handler]
#[instrument]
pub async fn get_proxy_config() -> Result<Json<ProxyConfig>, ApiError> {
    let store = read_proxy_store()?;
    Ok(Json(build_api_proxy_config(store)))
}

#[axum::debug_handler]
#[instrument]
pub async fn save_proxy_config(
    Json(req): Json<SaveProxyConfigRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut store = read_proxy_store()?;
    let payload_config = req.config.unwrap_or(ProxyConfig {
        enabled: req.enabled.unwrap_or(false),
        mode: req.mode.clone(),
        url: req.url.clone(),
        bypass_list: req.bypass_list.clone().unwrap_or_default(),
    });

    let next_mode = normalize_mode(
        payload_config.mode.as_deref().or(req.mode.as_deref()),
        Some(payload_config.enabled),
        payload_config
            .url
            .as_ref()
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false),
    );

    store.mode = next_mode;
    store.url = payload_config.url.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    store.bypass_list = payload_config
        .bypass_list
        .into_iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();
    write_proxy_store(&store)?;

    Ok(Json(serde_json::json!({
        "success": true,
        "config": build_api_proxy_config(store),
    })))
}

#[axum::debug_handler]
#[instrument]
pub async fn test_proxy_connection(
    Json(req): Json<TestProxyConnectionRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let proxy_url = req
        .proxy_url
        .or(req.proxy_url_camel)
        .unwrap_or_default()
        .trim()
        .to_string();
    if proxy_url.is_empty() {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "proxy_url is required",
        })));
    }

    let parsed = reqwest::Url::parse(&proxy_url)
        .map_err(|e| ApiError::InvalidRequest(format!("invalid proxy url: {e}")))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "only http/https proxy is supported",
        })));
    }

    let proxy = reqwest::Proxy::all(parsed.as_str())
        .map_err(|e| ApiError::InvalidRequest(format!("invalid proxy config: {e}")))?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .proxy(proxy)
        .build()
        .map_err(|e| ApiError::Http(e.to_string()))?;

    let start = std::time::Instant::now();
    let result = client
        .get("https://www.gstatic.com/generate_204")
        .send()
        .await;
    let elapsed_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(resp) if resp.status().is_success() => Ok(Json(serde_json::json!({
            "success": true,
            "latencyMs": elapsed_ms,
        }))),
        Ok(resp) => Ok(Json(serde_json::json!({
            "success": false,
            "error": format!("HTTP {}", resp.status().as_u16()),
            "latencyMs": elapsed_ms,
        }))),
        Err(err) => Ok(Json(serde_json::json!({
            "success": false,
            "error": err.to_string(),
            "latencyMs": elapsed_ms,
        }))),
    }
}

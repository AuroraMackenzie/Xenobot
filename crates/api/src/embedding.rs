//! Embedding API module for Xenobot HTTP API.
//!
//! Provides HTTP endpoints equivalent to Xenobot's `embeddingApi` IPC methods.

use axum::{
    extract::Path,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{instrument, warn};

use crate::{secrets, ApiError};

/// Embedding API router.
pub fn router() -> Router {
    Router::new()
        .route("/configs", get(get_all_configs))
        .route("/configs/:id", get(get_config))
        .route("/active-config-id", get(get_active_config_id))
        .route("/is-enabled", get(is_enabled))
        .route("/configs", post(add_config))
        .route("/configs/:id", post(update_config))
        .route("/configs/:id", delete(delete_config))
        .route("/active-config", post(set_active_config))
        .route("/validate-config", post(validate_config))
        .route("/vector-store-stats", get(get_vector_store_stats))
        .route("/clear-vector-store", post(clear_vector_store))
}

// ==================== Request/Response Types ====================

/// Embedding service configuration display (for listing, hides apiKey)
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingServiceConfigDisplay {
    /// Configuration ID (UUID)
    pub id: String,
    /// User-defined name
    pub name: String,
    /// API source: 'reuse_llm' or 'custom'
    pub api_source: String,
    /// Model name (e.g., 'nomi-embed-text')
    pub model: String,
    /// Custom endpoint URL (only for api_source === 'custom')
    pub base_url: Option<String>,
    /// Whether API key is set (doesn't show actual value)
    pub api_key_set: bool,
    /// Creation timestamp
    pub created_at: i64,
    /// Update timestamp
    pub updated_at: i64,
}

/// Embedding service configuration (full information)
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingServiceConfig {
    /// Configuration ID (UUID)
    pub id: String,
    /// User-defined name
    pub name: String,
    /// API source: 'reuse_llm' or 'custom'
    pub api_source: String,
    /// Model name (e.g., 'nomi-embed-text')
    pub model: String,
    /// Custom endpoint URL (only for api_source === 'custom')
    pub base_url: Option<String>,
    /// API key (optional, for custom endpoints)
    pub api_key: Option<String>,
    /// Creation timestamp
    pub created_at: i64,
    /// Update timestamp
    pub updated_at: i64,
}

/// Response wrapper for configuration operations
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigOperationResponse {
    /// Whether operation succeeded
    pub success: bool,
    /// Updated configuration (if success)
    pub config: Option<EmbeddingServiceConfig>,
    /// Error message (if failure)
    pub error: Option<String>,
}

/// Vector store statistics
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorStoreStats {
    /// Whether embedding is enabled
    pub enabled: bool,
    /// Number of vectors stored
    pub count: Option<u64>,
    /// Storage size in bytes
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct EmbeddingStoreState {
    active_config_id: Option<String>,
    configs: Vec<EmbeddingServiceConfig>,
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn new_config_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("embedding_{}", nanos)
}

fn config_dir() -> Result<PathBuf, ApiError> {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn store_path() -> Result<PathBuf, ApiError> {
    Ok(config_dir()?.join("embedding_configs.json"))
}

fn read_store() -> Result<EmbeddingStoreState, ApiError> {
    let path = store_path()?;
    if !path.exists() {
        return Ok(EmbeddingStoreState::default());
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(EmbeddingStoreState::default());
    }
    let mut state: EmbeddingStoreState = serde_json::from_str(&raw).map_err(ApiError::Json)?;
    hydrate_embedding_api_keys(&mut state);
    Ok(state)
}

fn write_store(state: &EmbeddingStoreState) -> Result<(), ApiError> {
    let path = store_path()?;
    persist_embedding_api_keys(state)?;
    let mut persisted = state.clone();
    for cfg in &mut persisted.configs {
        cfg.api_key = None;
    }
    let raw = serde_json::to_string_pretty(&persisted)?;
    fs::write(path, raw)?;
    Ok(())
}

fn hydrate_embedding_api_keys(state: &mut EmbeddingStoreState) {
    for cfg in &mut state.configs {
        match secrets::load_secret("embedding", &cfg.id) {
            Ok(Some(api_key)) => cfg.api_key = Some(api_key),
            Ok(None) => {}
            Err(err) => {
                warn!(
                    "failed to load embedding api key from secure storage for config {}: {}",
                    cfg.id, err
                );
            }
        }
    }
}

fn persist_embedding_api_keys(state: &EmbeddingStoreState) -> Result<(), ApiError> {
    for cfg in &state.configs {
        if let Some(api_key) = cfg.api_key.as_ref() {
            if api_key.trim().is_empty() {
                let _ = secrets::delete_secret("embedding", &cfg.id);
            } else {
                secrets::store_secret("embedding", &cfg.id, api_key)?;
            }
        } else {
            let _ = secrets::delete_secret("embedding", &cfg.id);
        }
    }
    Ok(())
}

fn to_public_config(cfg: &EmbeddingServiceConfig) -> EmbeddingServiceConfig {
    let mut out = cfg.clone();
    out.api_key = None;
    out
}

fn to_display(cfg: &EmbeddingServiceConfig) -> EmbeddingServiceConfigDisplay {
    EmbeddingServiceConfigDisplay {
        id: cfg.id.clone(),
        name: cfg.name.clone(),
        api_source: cfg.api_source.clone(),
        model: cfg.model.clone(),
        base_url: cfg.base_url.clone(),
        api_key_set: cfg
            .api_key
            .as_ref()
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false),
        created_at: cfg.created_at,
        updated_at: cfg.updated_at,
    }
}

// ==================== Handler Implementations ====================

#[instrument]
async fn get_all_configs() -> Result<Json<Vec<EmbeddingServiceConfigDisplay>>, ApiError> {
    let state = read_store()?;
    Ok(Json(state.configs.iter().map(to_display).collect()))
}

#[instrument]
async fn get_config(Path(id): Path<String>) -> Result<Json<EmbeddingServiceConfig>, ApiError> {
    let state = read_store()?;
    let cfg = state
        .configs
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| ApiError::NotFound("embedding config not found".to_string()))?;
    Ok(Json(to_public_config(&cfg)))
}

#[instrument]
async fn get_active_config_id() -> Result<Json<Option<String>>, ApiError> {
    let state = read_store()?;
    Ok(Json(state.active_config_id))
}

#[instrument]
async fn is_enabled() -> Result<Json<bool>, ApiError> {
    let state = read_store()?;
    let enabled = if let Some(active) = state.active_config_id.as_ref() {
        state.configs.iter().any(|c| &c.id == active)
    } else {
        false
    };
    Ok(Json(enabled))
}

#[derive(Debug, Deserialize)]
struct AddConfigRequest {
    name: String,
    api_source: String,
    model: String,
    base_url: Option<String>,
    api_key: Option<String>,
}

#[instrument]
async fn add_config(
    Json(req): Json<AddConfigRequest>,
) -> Result<Json<ConfigOperationResponse>, ApiError> {
    if req.name.trim().is_empty() || req.model.trim().is_empty() {
        return Ok(Json(ConfigOperationResponse {
            success: false,
            config: None,
            error: Some("name and model are required".to_string()),
        }));
    }
    let source = req.api_source.trim();
    if source != "reuse_llm" && source != "custom" {
        return Ok(Json(ConfigOperationResponse {
            success: false,
            config: None,
            error: Some("apiSource must be reuse_llm or custom".to_string()),
        }));
    }

    let mut state = read_store()?;
    if state.configs.len() >= 10 {
        return Ok(Json(ConfigOperationResponse {
            success: false,
            config: None,
            error: Some("maximum 10 configurations allowed".to_string()),
        }));
    }

    let ts = now_ts();
    let cfg = EmbeddingServiceConfig {
        id: new_config_id(),
        name: req.name.trim().to_string(),
        api_source: source.to_string(),
        model: req.model.trim().to_string(),
        base_url: req
            .base_url
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        api_key: req
            .api_key
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        created_at: ts,
        updated_at: ts,
    };
    let out = to_public_config(&cfg);
    state.configs.push(cfg.clone());
    if state.active_config_id.is_none() {
        state.active_config_id = Some(cfg.id);
    }
    write_store(&state)?;

    Ok(Json(ConfigOperationResponse {
        success: true,
        config: Some(out),
        error: None,
    }))
}

#[derive(Debug, Deserialize)]
struct UpdateConfigRequest {
    name: Option<String>,
    api_source: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
}

#[instrument]
async fn update_config(
    Path(id): Path<String>,
    Json(req): Json<UpdateConfigRequest>,
) -> Result<Json<ConfigOperationResponse>, ApiError> {
    let mut state = read_store()?;
    let Some(cfg) = state.configs.iter_mut().find(|c| c.id == id) else {
        return Ok(Json(ConfigOperationResponse {
            success: false,
            config: None,
            error: Some("config not found".to_string()),
        }));
    };

    if let Some(name) = req.name {
        if !name.trim().is_empty() {
            cfg.name = name.trim().to_string();
        }
    }
    if let Some(api_source) = req.api_source {
        if api_source == "reuse_llm" || api_source == "custom" {
            cfg.api_source = api_source;
        }
    }
    if let Some(model) = req.model {
        if !model.trim().is_empty() {
            cfg.model = model.trim().to_string();
        }
    }
    if let Some(base_url) = req.base_url {
        cfg.base_url = if base_url.trim().is_empty() {
            None
        } else {
            Some(base_url.trim().to_string())
        };
    }
    if let Some(api_key) = req.api_key {
        cfg.api_key = if api_key.trim().is_empty() {
            None
        } else {
            Some(api_key.trim().to_string())
        };
    }
    cfg.updated_at = now_ts();
    let out = to_public_config(cfg);
    write_store(&state)?;

    Ok(Json(ConfigOperationResponse {
        success: true,
        config: Some(out),
        error: None,
    }))
}

#[instrument]
async fn delete_config(Path(id): Path<String>) -> Result<Json<ConfigOperationResponse>, ApiError> {
    let mut state = read_store()?;
    let before = state.configs.len();
    state.configs.retain(|cfg| cfg.id != id);
    if before == state.configs.len() {
        return Ok(Json(ConfigOperationResponse {
            success: false,
            config: None,
            error: Some("config not found".to_string()),
        }));
    }
    if state.active_config_id.as_deref() == Some(id.as_str()) {
        state.active_config_id = state.configs.first().map(|c| c.id.clone());
    }
    let _ = secrets::delete_secret("embedding", &id);
    write_store(&state)?;
    Ok(Json(ConfigOperationResponse {
        success: true,
        config: None,
        error: None,
    }))
}

#[derive(Debug, Deserialize)]
struct SetActiveConfigRequest {
    id: String,
}

#[instrument]
async fn set_active_config(
    Json(req): Json<SetActiveConfigRequest>,
) -> Result<Json<ConfigOperationResponse>, ApiError> {
    let mut state = read_store()?;
    let Some(cfg) = state.configs.iter().find(|c| c.id == req.id) else {
        return Ok(Json(ConfigOperationResponse {
            success: false,
            config: None,
            error: Some("config not found".to_string()),
        }));
    };
    state.active_config_id = Some(req.id);
    let out = to_public_config(cfg);
    write_store(&state)?;
    Ok(Json(ConfigOperationResponse {
        success: true,
        config: Some(out),
        error: None,
    }))
}

#[instrument]
async fn validate_config(
    Json(req): Json<EmbeddingServiceConfig>,
) -> Result<Json<ConfigOperationResponse>, ApiError> {
    if req.name.trim().is_empty() || req.model.trim().is_empty() {
        return Ok(Json(ConfigOperationResponse {
            success: false,
            config: None,
            error: Some("name and model are required".to_string()),
        }));
    }
    if req.api_source != "reuse_llm" && req.api_source != "custom" {
        return Ok(Json(ConfigOperationResponse {
            success: false,
            config: None,
            error: Some("apiSource must be reuse_llm or custom".to_string()),
        }));
    }
    if req.api_source == "custom" {
        if let Some(base_url) = req.base_url.as_ref() {
            if !base_url.trim().is_empty() && reqwest::Url::parse(base_url.trim()).is_err() {
                return Ok(Json(ConfigOperationResponse {
                    success: false,
                    config: None,
                    error: Some("invalid baseUrl".to_string()),
                }));
            }
        }
    }
    Ok(Json(ConfigOperationResponse {
        success: true,
        config: Some(req),
        error: None,
    }))
}

#[instrument]
async fn get_vector_store_stats() -> Result<Json<VectorStoreStats>, ApiError> {
    let enabled = is_enabled().await?.0;
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))
        .ok();
    match pool {
        Some(pool) => {
            let row = sqlx::query(
                "SELECT COUNT(*) as count, COALESCE(SUM(LENGTH(embedding)), 0) as total_bytes FROM embedding_cache",
            )
            .fetch_one(pool.as_ref())
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
            let count: i64 = row.try_get("count").unwrap_or(0);
            let size: i64 = row.try_get("total_bytes").unwrap_or(0);
            Ok(Json(VectorStoreStats {
                enabled,
                count: Some(count.max(0) as u64),
                size_bytes: Some(size.max(0) as u64),
            }))
        }
        None => Ok(Json(VectorStoreStats {
            enabled,
            count: Some(0),
            size_bytes: Some(0),
        })),
    }
}

#[instrument]
async fn clear_vector_store() -> Result<Json<ConfigOperationResponse>, ApiError> {
    let pool = crate::database::get_pool()
        .await
        .map_err(|e| ApiError::Database(e.to_string()))
        .ok();
    if let Some(pool) = pool {
        sqlx::query("DELETE FROM embedding_cache")
            .execute(pool.as_ref())
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;
    }
    Ok(Json(ConfigOperationResponse {
        success: true,
        config: None,
        error: None,
    }))
}

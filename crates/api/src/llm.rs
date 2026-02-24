//! LLM API module for Xenobot HTTP API.
//!
//! Provides HTTP endpoints equivalent to Xenobot's `llmApi` IPC methods.

use axum::{
    extract::Path,
    response::{sse::Event, Sse},
    routing::{delete, get, post},
    Json, Router,
};
use futures::stream;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::{instrument, warn};

use crate::{secrets, ApiError};

/// LLM API router.
pub fn router() -> Router {
    Router::new()
        .route("/providers", get(get_providers))
        .route("/configs", get(get_all_configs))
        .route("/active-config-id", get(get_active_config_id))
        .route("/configs", post(add_config))
        .route("/configs/:id", post(update_config))
        .route("/configs/:id", delete(delete_config))
        .route("/active-config", post(set_active_config))
        .route("/validate-api-key", post(validate_api_key))
        .route("/has-config", get(has_config))
        .route("/chat", post(chat))
        .route("/chat-stream", post(chat_stream))
}

// ==================== Request/Response Types ====================

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProviderModel {
    id: String,
    name: String,
    description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LLMProvider {
    id: String,
    name: String,
    description: String,
    default_base_url: String,
    models: Vec<ProviderModel>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AIServiceConfigDisplay {
    id: String,
    name: String,
    provider: String,
    api_key: String,
    api_key_set: bool,
    model: Option<String>,
    base_url: Option<String>,
    max_tokens: Option<u32>,
    disable_thinking: Option<bool>,
    is_reasoning_model: Option<bool>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AIServiceConfig {
    id: String,
    name: String,
    provider: String,
    api_key: String,
    model: Option<String>,
    base_url: Option<String>,
    max_tokens: Option<u32>,
    disable_thinking: Option<bool>,
    is_reasoning_model: Option<bool>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ChatOptions {
    temperature: Option<f32>,
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatStreamChunk {
    content: String,
    is_finished: bool,
    finish_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigOperationResponse {
    success: bool,
    config: Option<AIServiceConfigDisplay>,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatResponse {
    success: bool,
    content: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct LlmStoreState {
    active_config_id: Option<String>,
    configs: Vec<AIServiceConfig>,
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
    format!("llm_{}", nanos)
}

fn config_dir() -> Result<PathBuf, ApiError> {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xenobot");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn llm_store_path() -> Result<PathBuf, ApiError> {
    Ok(config_dir()?.join("llm_configs.json"))
}

fn read_store() -> Result<LlmStoreState, ApiError> {
    let path = llm_store_path()?;
    if !path.exists() {
        return Ok(LlmStoreState::default());
    }
    let raw = fs::read_to_string(path)?;
    if raw.trim().is_empty() {
        return Ok(LlmStoreState::default());
    }
    let mut state: LlmStoreState = serde_json::from_str(&raw).map_err(ApiError::Json)?;
    hydrate_llm_api_keys(&mut state);
    Ok(state)
}

fn write_store(state: &LlmStoreState) -> Result<(), ApiError> {
    let path = llm_store_path()?;
    persist_llm_api_keys(state)?;
    let mut persisted = state.clone();
    for cfg in &mut persisted.configs {
        cfg.api_key.clear();
    }
    let raw = serde_json::to_string_pretty(&persisted)?;
    fs::write(path, raw)?;
    Ok(())
}

fn hydrate_llm_api_keys(state: &mut LlmStoreState) {
    for cfg in &mut state.configs {
        match secrets::load_secret("llm", &cfg.id) {
            Ok(Some(api_key)) => cfg.api_key = api_key,
            Ok(None) => {}
            Err(err) => {
                warn!(
                    "failed to load llm api key from secure storage for config {}: {}",
                    cfg.id, err
                );
            }
        }
    }
}

fn persist_llm_api_keys(state: &LlmStoreState) -> Result<(), ApiError> {
    for cfg in &state.configs {
        if cfg.api_key.trim().is_empty() {
            let _ = secrets::delete_secret("llm", &cfg.id);
        } else {
            secrets::store_secret("llm", &cfg.id, &cfg.api_key)?;
        }
    }
    Ok(())
}

fn to_display(config: &AIServiceConfig) -> AIServiceConfigDisplay {
    AIServiceConfigDisplay {
        id: config.id.clone(),
        name: config.name.clone(),
        provider: config.provider.clone(),
        api_key: if config.api_key.is_empty() {
            "".to_string()
        } else {
            "******".to_string()
        },
        api_key_set: !config.api_key.trim().is_empty(),
        model: config.model.clone(),
        base_url: config.base_url.clone(),
        max_tokens: config.max_tokens,
        disable_thinking: config.disable_thinking,
        is_reasoning_model: config.is_reasoning_model,
        created_at: config.created_at,
        updated_at: config.updated_at,
    }
}

fn provider_catalog() -> Vec<LLMProvider> {
    vec![
        LLMProvider {
            id: "deepseek".to_string(),
            name: "DeepSeek".to_string(),
            description: "DeepSeek API".to_string(),
            default_base_url: "https://api.deepseek.com/v1".to_string(),
            models: vec![
                ProviderModel {
                    id: "deepseek-chat".to_string(),
                    name: "DeepSeek Chat".to_string(),
                    description: Some("General chat model".to_string()),
                },
                ProviderModel {
                    id: "deepseek-coder".to_string(),
                    name: "DeepSeek Coder".to_string(),
                    description: Some("Code model".to_string()),
                },
            ],
        },
        LLMProvider {
            id: "qwen".to_string(),
            name: "Qwen".to_string(),
            description: "Alibaba Qwen API".to_string(),
            default_base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
            models: vec![
                ProviderModel {
                    id: "qwen-turbo".to_string(),
                    name: "Qwen Turbo".to_string(),
                    description: None,
                },
                ProviderModel {
                    id: "qwen-plus".to_string(),
                    name: "Qwen Plus".to_string(),
                    description: None,
                },
                ProviderModel {
                    id: "qwen-max".to_string(),
                    name: "Qwen Max".to_string(),
                    description: None,
                },
            ],
        },
        LLMProvider {
            id: "gemini".to_string(),
            name: "Gemini".to_string(),
            description: "Google Gemini API".to_string(),
            default_base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            models: vec![
                ProviderModel {
                    id: "gemini-3-flash-preview".to_string(),
                    name: "Gemini 3 Flash".to_string(),
                    description: None,
                },
                ProviderModel {
                    id: "gemini-3-pro-preview".to_string(),
                    name: "Gemini 3 Pro".to_string(),
                    description: None,
                },
            ],
        },
        LLMProvider {
            id: "minimax".to_string(),
            name: "MiniMax".to_string(),
            description: "MiniMax API".to_string(),
            default_base_url: "https://api.minimax.chat/v1".to_string(),
            models: vec![ProviderModel {
                id: "MiniMax-M2".to_string(),
                name: "MiniMax-M2".to_string(),
                description: None,
            }],
        },
        LLMProvider {
            id: "glm".to_string(),
            name: "GLM".to_string(),
            description: "Zhipu GLM API".to_string(),
            default_base_url: "https://open.bigmodel.cn/api/paas/v4".to_string(),
            models: vec![
                ProviderModel {
                    id: "glm-4-plus".to_string(),
                    name: "GLM-4-Plus".to_string(),
                    description: None,
                },
                ProviderModel {
                    id: "glm-4-flash".to_string(),
                    name: "GLM-4-Flash".to_string(),
                    description: None,
                },
            ],
        },
        LLMProvider {
            id: "kimi".to_string(),
            name: "Kimi".to_string(),
            description: "Moonshot API".to_string(),
            default_base_url: "https://api.moonshot.cn/v1".to_string(),
            models: vec![ProviderModel {
                id: "moonshot-v1-8k".to_string(),
                name: "Moonshot V1 8K".to_string(),
                description: None,
            }],
        },
        LLMProvider {
            id: "doubao".to_string(),
            name: "Doubao".to_string(),
            description: "Doubao API".to_string(),
            default_base_url: "https://ark.cn-beijing.volces.com/api/v3".to_string(),
            models: vec![ProviderModel {
                id: "doubao-seed-1-6-lite-251015".to_string(),
                name: "Doubao Seed Lite".to_string(),
                description: None,
            }],
        },
        LLMProvider {
            id: "openai-compatible".to_string(),
            name: "OpenAI Compatible".to_string(),
            description: "Compatible with OpenAI-style Chat Completions".to_string(),
            default_base_url: "http://localhost:11434/v1".to_string(),
            models: vec![
                ProviderModel {
                    id: "llama3.2".to_string(),
                    name: "Llama 3.2".to_string(),
                    description: None,
                },
                ProviderModel {
                    id: "qwen2.5".to_string(),
                    name: "Qwen 2.5".to_string(),
                    description: None,
                },
                ProviderModel {
                    id: "deepseek-r1".to_string(),
                    name: "DeepSeek R1".to_string(),
                    description: None,
                },
            ],
        },
    ]
}

fn build_stub_answer(messages: &[ChatMessage], active: Option<&AIServiceConfig>) -> String {
    let user_text = messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.trim())
        .unwrap_or("");
    if user_text.is_empty() {
        return "未收到有效的用户输入。".to_string();
    }
    let model_name = active
        .and_then(|cfg| cfg.model.clone())
        .unwrap_or_else(|| "xenobot-local".to_string());
    format!(
        "[{}] {}\n\n说明：当前为本地安全回退回复（未调用外部模型）。",
        model_name, user_text
    )
}

// ==================== Handler Implementations ====================

#[instrument]
async fn get_providers() -> Result<Json<Vec<LLMProvider>>, ApiError> {
    Ok(Json(provider_catalog()))
}

#[instrument]
async fn get_all_configs() -> Result<Json<Vec<AIServiceConfigDisplay>>, ApiError> {
    let state = read_store()?;
    Ok(Json(state.configs.iter().map(to_display).collect()))
}

#[instrument]
async fn get_active_config_id() -> Result<Json<Option<String>>, ApiError> {
    let state = read_store()?;
    Ok(Json(state.active_config_id))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddConfigRequest {
    name: String,
    provider: String,
    api_key: String,
    model: Option<String>,
    base_url: Option<String>,
    max_tokens: Option<u32>,
    disable_thinking: Option<bool>,
    is_reasoning_model: Option<bool>,
}

#[instrument]
async fn add_config(
    Json(req): Json<AddConfigRequest>,
) -> Result<Json<ConfigOperationResponse>, ApiError> {
    if req.name.trim().is_empty() {
        return Ok(Json(ConfigOperationResponse {
            success: false,
            config: None,
            error: Some("name is required".to_string()),
        }));
    }
    if req.provider.trim().is_empty() {
        return Ok(Json(ConfigOperationResponse {
            success: false,
            config: None,
            error: Some("provider is required".to_string()),
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
    let cfg = AIServiceConfig {
        id: new_config_id(),
        name: req.name.trim().to_string(),
        provider: req.provider.trim().to_string(),
        api_key: req.api_key.trim().to_string(),
        model: req
            .model
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        base_url: req
            .base_url
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        max_tokens: req.max_tokens,
        disable_thinking: req.disable_thinking,
        is_reasoning_model: req.is_reasoning_model,
        created_at: ts,
        updated_at: ts,
    };

    let display = to_display(&cfg);
    let new_id = cfg.id.clone();
    state.configs.push(cfg);
    if state.active_config_id.is_none() {
        state.active_config_id = Some(new_id);
    }
    write_store(&state)?;

    Ok(Json(ConfigOperationResponse {
        success: true,
        config: Some(display),
        error: None,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateConfigRequest {
    name: Option<String>,
    provider: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    max_tokens: Option<u32>,
    disable_thinking: Option<bool>,
    is_reasoning_model: Option<bool>,
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
    if let Some(provider) = req.provider {
        if !provider.trim().is_empty() {
            cfg.provider = provider.trim().to_string();
        }
    }
    if let Some(api_key) = req.api_key {
        cfg.api_key = api_key.trim().to_string();
    }
    if let Some(model) = req.model {
        cfg.model = if model.trim().is_empty() {
            None
        } else {
            Some(model.trim().to_string())
        };
    }
    if let Some(base_url) = req.base_url {
        cfg.base_url = if base_url.trim().is_empty() {
            None
        } else {
            Some(base_url.trim().to_string())
        };
    }
    if req.max_tokens.is_some() {
        cfg.max_tokens = req.max_tokens;
    }
    if req.disable_thinking.is_some() {
        cfg.disable_thinking = req.disable_thinking;
    }
    if req.is_reasoning_model.is_some() {
        cfg.is_reasoning_model = req.is_reasoning_model;
    }
    cfg.updated_at = now_ts();
    let display = to_display(cfg);
    write_store(&state)?;

    Ok(Json(ConfigOperationResponse {
        success: true,
        config: Some(display),
        error: None,
    }))
}

#[instrument]
async fn delete_config(Path(id): Path<String>) -> Result<Json<ConfigOperationResponse>, ApiError> {
    let mut state = read_store()?;
    let before = state.configs.len();
    state.configs.retain(|cfg| cfg.id != id);
    if state.configs.len() == before {
        return Ok(Json(ConfigOperationResponse {
            success: false,
            config: None,
            error: Some("config not found".to_string()),
        }));
    }

    if state.active_config_id.as_deref() == Some(id.as_str()) {
        state.active_config_id = state.configs.first().map(|c| c.id.clone());
    }
    let _ = secrets::delete_secret("llm", &id);
    write_store(&state)?;
    Ok(Json(ConfigOperationResponse {
        success: true,
        config: None,
        error: None,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    let display = to_display(cfg);
    state.active_config_id = Some(req.id);
    write_store(&state)?;
    Ok(Json(ConfigOperationResponse {
        success: true,
        config: Some(display),
        error: None,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidateApiKeyRequest {
    provider: String,
    api_key: String,
    base_url: Option<String>,
    model: Option<String>,
}

#[instrument]
async fn validate_api_key(
    Json(req): Json<ValidateApiKeyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.provider.trim().is_empty() {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "provider is required",
        })));
    }

    if req.provider != "openai-compatible" && req.api_key.trim().is_empty() {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "apiKey is required",
        })));
    }

    if let Some(base_url) = &req.base_url {
        if !base_url.trim().is_empty() {
            let parsed = reqwest::Url::parse(base_url.trim());
            if parsed.is_err() {
                return Ok(Json(serde_json::json!({
                    "success": false,
                    "error": "invalid baseUrl",
                })));
            }
        }
    }

    let _ = req.model;
    Ok(Json(serde_json::json!({
        "success": true,
    })))
}

#[instrument]
async fn has_config() -> Result<Json<bool>, ApiError> {
    let state = read_store()?;
    let has = if let Some(active) = state.active_config_id.as_ref() {
        state.configs.iter().any(|c| &c.id == active)
    } else {
        !state.configs.is_empty()
    };
    Ok(Json(has))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequest {
    messages: Vec<ChatMessage>,
    options: Option<ChatOptions>,
}

#[instrument]
async fn chat(Json(req): Json<ChatRequest>) -> Result<Json<ChatResponse>, ApiError> {
    let state = read_store()?;
    let active = state
        .active_config_id
        .as_ref()
        .and_then(|id| state.configs.iter().find(|cfg| &cfg.id == id));

    if active.is_none() {
        return Ok(Json(ChatResponse {
            success: false,
            content: None,
            error: Some("no active LLM config".to_string()),
        }));
    }

    let _ = req.options;
    let content = build_stub_answer(&req.messages, active);
    Ok(Json(ChatResponse {
        success: true,
        content: Some(content),
        error: None,
    }))
}

#[instrument]
async fn chat_stream(
    Json(req): Json<ChatRequest>,
) -> Sse<impl stream::Stream<Item = Result<Event, Infallible>>> {
    let state = read_store().ok();
    let active = state.as_ref().and_then(|s| {
        s.active_config_id
            .as_ref()
            .and_then(|id| s.configs.iter().find(|cfg| &cfg.id == id))
    });
    let _ = req.options.as_ref().and_then(|v| v.temperature);

    let answer = if active.is_some() {
        build_stub_answer(&req.messages, active)
    } else {
        "未配置 LLM，请先在设置中添加模型配置。".to_string()
    };

    let mut chunks = Vec::new();
    for part in answer.split_whitespace() {
        chunks.push(ChatStreamChunk {
            content: format!("{part} "),
            is_finished: false,
            finish_reason: None,
        });
    }
    chunks.push(ChatStreamChunk {
        content: "".to_string(),
        is_finished: true,
        finish_reason: Some("stop".to_string()),
    });

    let stream = stream::iter(
        chunks
            .into_iter()
            .filter_map(|chunk| serde_json::to_string(&chunk).ok())
            .map(|payload| Ok(Event::default().data(payload)))
            .collect::<Vec<_>>(),
    );
    Sse::new(stream)
}

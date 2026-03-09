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
    time::Duration,
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

fn find_provider_by_id(raw: &str) -> Option<LLMProvider> {
    let normalized = raw.trim().to_ascii_lowercase();
    provider_catalog().into_iter().find(|p| p.id == normalized)
}

fn canonical_model_for_provider(provider: &LLMProvider, model: &str) -> Option<String> {
    provider
        .models
        .iter()
        .find(|m| m.id.eq_ignore_ascii_case(model.trim()))
        .map(|m| m.id.clone())
}

fn validate_provider_and_model(
    provider_raw: &str,
    model_raw: Option<&str>,
) -> Result<(String, Option<String>), String> {
    let provider = find_provider_by_id(provider_raw)
        .ok_or_else(|| format!("unsupported provider: {}", provider_raw.trim()))?;
    let normalized_model = model_raw.map(str::trim).filter(|v| !v.is_empty());

    if let Some(model) = normalized_model {
        if provider.id == "openai-compatible" {
            return Ok((provider.id, Some(model.to_string())));
        }
        if let Some(canonical) = canonical_model_for_provider(&provider, model) {
            return Ok((provider.id, Some(canonical)));
        }
        return Err(format!(
            "model `{}` is not supported for provider `{}`",
            model, provider.id
        ));
    }

    Ok((provider.id, None))
}

fn normalize_base_url(base_url_raw: Option<&str>) -> Result<Option<String>, String> {
    let Some(base_url) = base_url_raw.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };

    reqwest::Url::parse(base_url).map_err(|_| "invalid baseUrl".to_string())?;
    Ok(Some(base_url.to_string()))
}

fn llm_request_timeout_ms() -> u64 {
    const DEFAULT_TIMEOUT_MS: u64 = 15_000;
    const MIN_TIMEOUT_MS: u64 = 500;
    const MAX_TIMEOUT_MS: u64 = 120_000;

    match std::env::var("XENOBOT_LLM_TIMEOUT_MS") {
        Ok(raw) => raw
            .trim()
            .parse::<u64>()
            .ok()
            .map(|v| v.clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS))
            .unwrap_or(DEFAULT_TIMEOUT_MS),
        Err(_) => DEFAULT_TIMEOUT_MS,
    }
}

fn default_provider_model(provider_id: &str) -> Option<String> {
    find_provider_by_id(provider_id)
        .and_then(|provider| provider.models.first().map(|m| m.id.clone()))
}

fn default_provider_base_url(provider_id: &str) -> Option<String> {
    find_provider_by_id(provider_id).map(|provider| provider.default_base_url)
}

fn effective_model(config: &AIServiceConfig) -> String {
    config
        .model
        .clone()
        .or_else(|| default_provider_model(&config.provider))
        .unwrap_or_else(|| "xenobot-local".to_string())
}

fn effective_base_url(config: &AIServiceConfig) -> Option<String> {
    config
        .base_url
        .clone()
        .or_else(|| default_provider_base_url(&config.provider))
}

fn supports_openai_chat_completion(provider_id: &str) -> bool {
    !provider_id.eq_ignore_ascii_case("gemini")
}

fn build_gemini_payload(
    messages: &[ChatMessage],
    options: Option<&ChatOptions>,
    config: &AIServiceConfig,
) -> serde_json::Value {
    let contents = messages
        .iter()
        .map(|msg| {
            let role = if msg.role.eq_ignore_ascii_case("assistant") {
                "model"
            } else {
                "user"
            };
            serde_json::json!({
                "role": role,
                "parts": [{"text": msg.content}],
            })
        })
        .collect::<Vec<_>>();

    let mut payload = serde_json::json!({
        "contents": contents,
    });

    let mut generation = serde_json::Map::new();
    if let Some(temperature) = options.and_then(|opts| opts.temperature) {
        generation.insert("temperature".to_string(), serde_json::json!(temperature));
    }
    if let Some(max_tokens) = options
        .and_then(|opts| opts.max_tokens)
        .or(config.max_tokens)
    {
        generation.insert("maxOutputTokens".to_string(), serde_json::json!(max_tokens));
    }
    if !generation.is_empty() {
        payload["generationConfig"] = serde_json::Value::Object(generation);
    }

    payload
}

fn extract_gemini_text(raw: &serde_json::Value) -> Option<String> {
    let candidate = raw
        .get("candidates")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())?;
    let parts = candidate
        .get("content")
        .and_then(|v| v.get("parts"))
        .and_then(|v| v.as_array())?;
    let mut chunks = Vec::new();
    for part in parts {
        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
            let normalized = text.trim();
            if !normalized.is_empty() {
                chunks.push(normalized.to_string());
            }
        }
    }
    if chunks.is_empty() {
        None
    } else {
        Some(chunks.join("\n"))
    }
}

fn build_openai_chat_payload(
    model: &str,
    messages: &[ChatMessage],
    options: Option<&ChatOptions>,
    config: &AIServiceConfig,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "model": model,
        "messages": messages
            .iter()
            .map(|msg| {
                serde_json::json!({
                    "role": msg.role,
                    "content": msg.content,
                })
            })
            .collect::<Vec<_>>(),
    });
    if let Some(temperature) = options.and_then(|opts| opts.temperature) {
        payload["temperature"] = serde_json::json!(temperature);
    }
    if let Some(max_tokens) = options
        .and_then(|opts| opts.max_tokens)
        .or(config.max_tokens)
    {
        payload["max_tokens"] = serde_json::json!(max_tokens);
    }
    payload
}

fn extract_chat_completion_text(raw: &serde_json::Value) -> Option<String> {
    let first = raw
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())?;
    let message = first.get("message")?;
    if let Some(text) = message.get("content").and_then(|v| v.as_str()) {
        let value = text.trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    if let Some(parts) = message.get("content").and_then(|v| v.as_array()) {
        let mut chunks = Vec::new();
        for part in parts {
            if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                let normalized = text.trim();
                if !normalized.is_empty() {
                    chunks.push(normalized.to_string());
                }
            }
        }
        if !chunks.is_empty() {
            return Some(chunks.join("\n"));
        }
    }
    None
}

fn truncate_error_body(value: &str, limit: usize) -> String {
    let normalized = value.trim();
    if normalized.len() <= limit {
        return normalized.to_string();
    }
    format!("{}...", &normalized[..limit])
}

async fn try_openai_compatible_chat_completion(
    config: &AIServiceConfig,
    messages: &[ChatMessage],
    options: Option<&ChatOptions>,
) -> Result<String, ApiError> {
    let Some(base_url) = effective_base_url(config) else {
        return Err(ApiError::InvalidRequest(
            "missing base URL for active provider".to_string(),
        ));
    };
    let model = effective_model(config);
    let endpoint = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let payload = build_openai_chat_payload(&model, messages, options, config);
    let timeout_ms = llm_request_timeout_ms();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|err| ApiError::Http(format!("failed to build llm client: {err}")))?;
    let mut request = client
        .post(&endpoint)
        .header("content-type", "application/json")
        .json(&payload);
    if !config.api_key.trim().is_empty() {
        request = request.bearer_auth(config.api_key.trim());
    }

    let response = request
        .send()
        .await
        .map_err(|err| ApiError::Http(format!("llm upstream request failed: {err}")))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ApiError::Http(format!(
            "llm upstream returned {}: {}",
            status,
            truncate_error_body(&body, 256)
        )));
    }

    let raw_json = response
        .json::<serde_json::Value>()
        .await
        .map_err(|err| ApiError::Http(format!("invalid llm response payload: {err}")))?;
    extract_chat_completion_text(&raw_json).ok_or_else(|| {
        ApiError::Http("llm upstream response missing choices[0].message.content".to_string())
    })
}

async fn try_gemini_chat_completion(
    config: &AIServiceConfig,
    messages: &[ChatMessage],
    options: Option<&ChatOptions>,
) -> Result<String, ApiError> {
    let Some(base_url) = effective_base_url(config) else {
        return Err(ApiError::InvalidRequest(
            "missing base URL for active provider".to_string(),
        ));
    };
    if config.api_key.trim().is_empty() {
        return Err(ApiError::InvalidRequest(
            "gemini provider requires apiKey".to_string(),
        ));
    }

    let model = effective_model(config);
    let endpoint = format!(
        "{}/models/{}:generateContent",
        base_url.trim_end_matches('/'),
        model
    );
    let payload = build_gemini_payload(messages, options, config);
    let timeout_ms = llm_request_timeout_ms();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .build()
        .map_err(|err| ApiError::Http(format!("failed to build llm client: {err}")))?;
    let response = client
        .post(&endpoint)
        .query(&[("key", config.api_key.trim())])
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|err| ApiError::Http(format!("llm upstream request failed: {err}")))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ApiError::Http(format!(
            "llm upstream returned {}: {}",
            status,
            truncate_error_body(&body, 256)
        )));
    }

    let raw_json = response
        .json::<serde_json::Value>()
        .await
        .map_err(|err| ApiError::Http(format!("invalid llm response payload: {err}")))?;
    extract_gemini_text(&raw_json).ok_or_else(|| {
        ApiError::Http(
            "llm upstream response missing candidates[0].content.parts[].text".to_string(),
        )
    })
}

async fn generate_chat_answer(
    messages: &[ChatMessage],
    options: Option<&ChatOptions>,
    active: &AIServiceConfig,
) -> String {
    let call_result = if supports_openai_chat_completion(&active.provider) {
        try_openai_compatible_chat_completion(active, messages, options).await
    } else if active.provider.eq_ignore_ascii_case("gemini") {
        try_gemini_chat_completion(active, messages, options).await
    } else {
        return build_stub_answer(messages, Some(active));
    };

    match call_result {
        Ok(content) => content,
        Err(err) => {
            warn!(
                "llm remote call failed for provider {}: {}; falling back to local safe response",
                active.provider, err
            );
            build_stub_answer(messages, Some(active))
        }
    }
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
    let (provider, model) = match validate_provider_and_model(&req.provider, req.model.as_deref()) {
        Ok(v) => v,
        Err(error) => {
            return Ok(Json(ConfigOperationResponse {
                success: false,
                config: None,
                error: Some(error),
            }));
        }
    };
    let base_url = match normalize_base_url(req.base_url.as_deref()) {
        Ok(v) => v,
        Err(error) => {
            return Ok(Json(ConfigOperationResponse {
                success: false,
                config: None,
                error: Some(error),
            }));
        }
    };

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
        provider,
        api_key: req.api_key.trim().to_string(),
        model,
        base_url,
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
    let UpdateConfigRequest {
        name,
        provider,
        api_key,
        model,
        base_url,
        max_tokens,
        disable_thinking,
        is_reasoning_model,
    } = req;

    let mut state = read_store()?;
    let Some(cfg) = state.configs.iter_mut().find(|c| c.id == id) else {
        return Ok(Json(ConfigOperationResponse {
            success: false,
            config: None,
            error: Some("config not found".to_string()),
        }));
    };

    if let Some(name) = name {
        if !name.trim().is_empty() {
            cfg.name = name.trim().to_string();
        }
    }
    if let Some(api_key) = api_key {
        cfg.api_key = api_key.trim().to_string();
    }

    if provider.is_some() || model.is_some() {
        let pending_provider = provider
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or(cfg.provider.as_str());
        let pending_model = match model.as_deref() {
            Some(value) => Some(value.trim()),
            None => cfg.model.as_deref(),
        };
        let pending_model = pending_model.filter(|v| !v.is_empty());

        let (normalized_provider, normalized_model) =
            match validate_provider_and_model(pending_provider, pending_model) {
                Ok(v) => v,
                Err(error) => {
                    return Ok(Json(ConfigOperationResponse {
                        success: false,
                        config: None,
                        error: Some(error),
                    }));
                }
            };
        cfg.provider = normalized_provider;
        cfg.model = normalized_model;
    }

    if base_url.is_some() {
        cfg.base_url = match normalize_base_url(base_url.as_deref()) {
            Ok(v) => v,
            Err(error) => {
                return Ok(Json(ConfigOperationResponse {
                    success: false,
                    config: None,
                    error: Some(error),
                }));
            }
        };
    }
    if max_tokens.is_some() {
        cfg.max_tokens = max_tokens;
    }
    if disable_thinking.is_some() {
        cfg.disable_thinking = disable_thinking;
    }
    if is_reasoning_model.is_some() {
        cfg.is_reasoning_model = is_reasoning_model;
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
    let (provider, _model) = match validate_provider_and_model(&req.provider, req.model.as_deref())
    {
        Ok(v) => v,
        Err(error) => {
            return Ok(Json(serde_json::json!({
                "success": false,
                "error": error,
            })));
        }
    };

    if provider != "openai-compatible" && req.api_key.trim().is_empty() {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": "apiKey is required",
        })));
    }

    if let Err(error) = normalize_base_url(req.base_url.as_deref()) {
        return Ok(Json(serde_json::json!({
            "success": false,
            "error": error,
        })));
    }

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

    let active = active.expect("checked above");
    let content = generate_chat_answer(&req.messages, req.options.as_ref(), active).await;
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
    let answer = if let Some(active_cfg) = active {
        generate_chat_answer(&req.messages, req.options.as_ref(), active_cfg).await
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

#[cfg(test)]
mod tests {
    use super::{
        extract_chat_completion_text, extract_gemini_text, generate_chat_answer,
        normalize_base_url, supports_openai_chat_completion, validate_provider_and_model,
        AIServiceConfig, ChatMessage,
    };
    use axum::{routing::post, Json, Router};
    use serde_json::json;

    #[test]
    fn validate_provider_and_model_allows_openai_custom_model() {
        let (provider, model) =
            validate_provider_and_model("OPENAI-COMPATIBLE", Some("my-local-model")).unwrap();
        assert_eq!(provider, "openai-compatible");
        assert_eq!(model.as_deref(), Some("my-local-model"));
    }

    #[test]
    fn validate_provider_and_model_normalizes_known_model_ids() {
        let (provider, model) = validate_provider_and_model("qwen", Some("QWEN-PLUS")).unwrap();
        assert_eq!(provider, "qwen");
        assert_eq!(model.as_deref(), Some("qwen-plus"));
    }

    #[test]
    fn validate_provider_and_model_rejects_unknown_provider() {
        let err = validate_provider_and_model("unknown-provider", Some("x")).unwrap_err();
        assert!(err.contains("unsupported provider"));
    }

    #[test]
    fn validate_provider_and_model_rejects_model_not_in_catalog() {
        let err = validate_provider_and_model("qwen", Some("not-a-model")).unwrap_err();
        assert!(err.contains("not supported for provider `qwen`"));
    }

    #[test]
    fn normalize_base_url_accepts_none_or_valid_url() {
        assert_eq!(normalize_base_url(None).unwrap(), None);
        assert_eq!(
            normalize_base_url(Some("https://api.example.com/v1")).unwrap(),
            Some("https://api.example.com/v1".to_string())
        );
    }

    #[test]
    fn normalize_base_url_rejects_invalid_url() {
        let err = normalize_base_url(Some("not-a-url")).unwrap_err();
        assert_eq!(err, "invalid baseUrl");
    }

    #[test]
    fn extract_chat_completion_text_reads_string_and_part_arrays() {
        let string_payload = json!({
            "choices": [{
                "message": { "content": "hello world" }
            }]
        });
        assert_eq!(
            extract_chat_completion_text(&string_payload).as_deref(),
            Some("hello world")
        );

        let part_payload = json!({
            "choices": [{
                "message": {
                    "content": [
                        { "type": "text", "text": "alpha" },
                        { "type": "text", "text": "beta" }
                    ]
                }
            }]
        });
        assert_eq!(
            extract_chat_completion_text(&part_payload).as_deref(),
            Some("alpha\nbeta")
        );
    }

    #[test]
    fn extract_gemini_text_reads_candidate_parts() {
        let payload = json!({
            "candidates": [{
                "content": {
                    "parts": [
                        { "text": "first" },
                        { "text": "second" }
                    ]
                }
            }]
        });
        assert_eq!(
            extract_gemini_text(&payload).as_deref(),
            Some("first\nsecond")
        );
    }

    #[test]
    fn supports_openai_chat_completion_excludes_gemini() {
        assert!(supports_openai_chat_completion("openai-compatible"));
        assert!(supports_openai_chat_completion("qwen"));
        assert!(!supports_openai_chat_completion("gemini"));
    }

    #[tokio::test]
    async fn generate_chat_answer_uses_remote_when_available() {
        async fn handler() -> Json<serde_json::Value> {
            Json(json!({
                "choices": [{
                    "message": { "content": "remote-llm-answer" }
                }]
            }))
        }

        let app = Router::new().route("/chat/completions", post(handler));
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                // Some sandboxed environments deny local port binding.
                return;
            }
            Err(err) => panic!("bind test listener: {err}"),
        };
        let addr = listener.local_addr().expect("listener addr");
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let cfg = AIServiceConfig {
            id: "cfg_remote".to_string(),
            name: "Remote".to_string(),
            provider: "openai-compatible".to_string(),
            api_key: "".to_string(),
            model: Some("mock-model".to_string()),
            base_url: Some(format!("http://{}", addr)),
            max_tokens: None,
            disable_thinking: None,
            is_reasoning_model: None,
            created_at: 0,
            updated_at: 0,
        };
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }];
        let output = generate_chat_answer(&messages, None, &cfg).await;
        server.abort();
        assert_eq!(output, "remote-llm-answer");
    }

    #[tokio::test]
    async fn generate_chat_answer_falls_back_when_remote_fails() {
        let cfg = AIServiceConfig {
            id: "cfg_fallback".to_string(),
            name: "Fallback".to_string(),
            provider: "openai-compatible".to_string(),
            api_key: "".to_string(),
            model: Some("mock-model".to_string()),
            base_url: Some("http://127.0.0.1:9".to_string()),
            max_tokens: None,
            disable_thinking: None,
            is_reasoning_model: None,
            created_at: 0,
            updated_at: 0,
        };
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }];
        let output = generate_chat_answer(&messages, None, &cfg).await;
        assert!(output.contains("本地安全回退回复"));
    }

    #[tokio::test]
    async fn generate_chat_answer_uses_gemini_remote_when_available() {
        async fn handler() -> Json<serde_json::Value> {
            Json(json!({
                "candidates": [{
                    "content": {
                        "parts": [{ "text": "gemini-remote-answer" }]
                    }
                }]
            }))
        }

        let app = Router::new().route("/models/*path", post(handler));
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                return;
            }
            Err(err) => panic!("bind test listener: {err}"),
        };
        let addr = listener.local_addr().expect("listener addr");
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let cfg = AIServiceConfig {
            id: "cfg_gemini".to_string(),
            name: "Gemini".to_string(),
            provider: "gemini".to_string(),
            api_key: "test-key".to_string(),
            model: Some("gemini-3-flash-preview".to_string()),
            base_url: Some(format!("http://{}", addr)),
            max_tokens: None,
            disable_thinking: None,
            is_reasoning_model: None,
            created_at: 0,
            updated_at: 0,
        };
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }];
        let output = generate_chat_answer(&messages, None, &cfg).await;
        server.abort();
        assert_eq!(output, "gemini-remote-answer");
    }
}

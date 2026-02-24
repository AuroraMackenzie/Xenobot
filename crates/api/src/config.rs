//! Configuration for the Xenobot HTTP API server.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use xenobot_core::XenobotConfig;

/// API server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Socket address to bind to.
    pub bind_addr: SocketAddr,

    /// Unix domain socket path for API serving (preferred in sandboxed environments).
    ///
    /// When set, server listens on this socket and ignores `bind_addr`.
    pub unix_socket_path: Option<PathBuf>,

    /// File mode for unix domain socket file (octal, e.g. 0o700).
    pub unix_socket_mode: u32,

    /// API base path (e.g., "/api/v1").
    pub api_base_path: String,

    /// Enable CORS.
    pub enable_cors: bool,

    /// Allowed CORS origins.
    pub cors_allowed_origins: Vec<String>,

    /// Request timeout in seconds.
    pub request_timeout_seconds: u64,

    /// Enable request logging.
    pub enable_request_logging: bool,

    /// Enable response compression.
    pub enable_compression: bool,

    /// Maximum request body size in bytes.
    pub max_body_size: usize,

    /// Rate limiting configuration.
    pub rate_limiting: RateLimitingConfig,

    /// Authentication configuration.
    pub auth: AuthConfig,

    /// Feature flags.
    pub features: ApiFeatures,

    /// Webhook dead-letter replay worker configuration.
    pub webhook_replay: WebhookReplayConfig,
}

/// Rate limiting configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    /// Enable rate limiting.
    pub enabled: bool,

    /// Requests per minute.
    pub requests_per_minute: u32,

    /// Burst size.
    pub burst_size: u32,
}

/// Authentication configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable authentication.
    pub enabled: bool,

    /// Authentication token header name.
    pub token_header: String,

    /// Required token value.
    pub required_token: Option<String>,

    /// Enable API key authentication.
    pub enable_api_keys: bool,
}

/// API feature flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiFeatures {
    /// Enable chat API.
    pub enable_chat: bool,

    /// Enable merge API.
    pub enable_merge: bool,

    /// Enable AI API.
    pub enable_ai: bool,

    /// Enable LLM API.
    pub enable_llm: bool,

    /// Enable agent API.
    pub enable_agent: bool,

    /// Enable embedding API.
    pub enable_embedding: bool,

    /// Enable core API.
    pub enable_core: bool,

    /// Enable NLP API.
    pub enable_nlp: bool,

    /// Enable network API.
    pub enable_network: bool,

    /// Enable cache API.
    pub enable_cache: bool,

    /// Enable session API.
    pub enable_session: bool,

    /// Enable events API.
    pub enable_events: bool,

    /// Enable WeChat integration.
    pub enable_wechat: bool,
}

/// Webhook dead-letter replay configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookReplayConfig {
    /// Enable automatic dead-letter replay worker.
    pub enabled: bool,

    /// Worker wake-up interval in seconds.
    pub interval_seconds: u64,

    /// Max dead-letter entries to process per tick.
    pub max_entries_per_tick: usize,

    /// Max concurrent webhook deliveries per tick.
    pub max_concurrency: usize,

    /// Per-request timeout in seconds.
    pub request_timeout_seconds: u64,

    /// Skip replay for entries whose attempts are already above this threshold.
    pub max_attempts: u32,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:5030".parse().unwrap(),
            unix_socket_path: None,
            unix_socket_mode: 0o700,
            api_base_path: "/api/v1".to_string(),
            enable_cors: true,
            cors_allowed_origins: vec![
                "http://localhost:3000".to_string(),
                "http://localhost:5173".to_string(),
            ],
            request_timeout_seconds: 30,
            enable_request_logging: true,
            enable_compression: true,
            max_body_size: 10 * 1024 * 1024, // 10MB
            rate_limiting: RateLimitingConfig::default(),
            auth: AuthConfig::default(),
            features: ApiFeatures::default(),
            webhook_replay: WebhookReplayConfig::default(),
        }
    }
}

impl Default for RateLimitingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            requests_per_minute: 60,
            burst_size: 10,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            token_header: "X-API-Token".to_string(),
            required_token: None,
            enable_api_keys: false,
        }
    }
}

impl Default for ApiFeatures {
    fn default() -> Self {
        Self {
            enable_chat: true,
            enable_merge: true,
            enable_ai: true,
            enable_llm: true,
            enable_agent: true,
            enable_embedding: true,
            enable_core: true,
            enable_nlp: true,
            enable_network: true,
            enable_cache: true,
            enable_session: true,
            enable_events: true,
            enable_wechat: cfg!(feature = "wechat"),
        }
    }
}

impl Default for WebhookReplayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_seconds: 20,
            max_entries_per_tick: 64,
            max_concurrency: 8,
            request_timeout_seconds: 8,
            max_attempts: 20,
        }
    }
}

impl ApiConfig {
    /// Create API configuration from core Xenobot configuration.
    pub fn from_core_config(core_config: &XenobotConfig) -> Self {
        let mut config = Self::default();

        config.bind_addr = format!("{}:{}", core_config.http.host, core_config.http.port)
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:5030".parse().unwrap());

        config.api_base_path = core_config.http.api_base_path.clone();
        config.enable_cors = core_config.http.enable_cors;
        config.request_timeout_seconds = core_config.http.request_timeout;
        config.enable_request_logging = core_config.http.enable_request_logging;

        // Enable features based on core config
        config.features.enable_wechat = core_config.features.enable_real_time_extraction;
        config.webhook_replay.enabled = core_config.features.enable_webhook;

        config
    }
}

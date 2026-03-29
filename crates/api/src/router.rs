//! Axum router configuration for Xenobot HTTP API.

use crate::config::ApiConfig;
use axum::{Json, Router};
use tower_http::cors::CorsLayer;

/// Build the main API router with all enabled modules.
pub fn build_router(config: &ApiConfig) -> Router {
    let mut router = Router::new();
    let status_payload = build_status_payload(config);

    // Add CORS layer if enabled
    if config.enable_cors {
        router = router.layer(create_cors_layer(config));
    }

    // Add API modules based on feature flags
    if config.features.enable_chat {
        router = router.nest("/chat", crate::chat::router());
    }

    // Media routes are useful for chat/AI UIs and do not depend on frontend runtime.
    router = router.nest("/media", crate::media::router());
    router = router.nest("/memory", crate::memory::router());

    if config.features.enable_merge {
        router = router.nest("/merge", crate::merge::router());
    }

    if config.features.enable_ai {
        router = router.nest("/ai", crate::ai::router());
    }

    if config.features.enable_llm {
        router = router.nest("/llm", crate::llm::router());
    }

    if config.features.enable_agent {
        router = router.nest("/agent", crate::agent::router());
    }

    if config.features.enable_embedding {
        router = router.nest("/embedding", crate::embedding::router());
    }

    if config.features.enable_core {
        router = router.nest("/core", crate::core::router());
    }

    if config.features.enable_nlp {
        router = router.nest("/nlp", crate::nlp::router());
    }

    if config.features.enable_network {
        router = router.nest("/network", crate::network::router());
    }

    if config.features.enable_cache {
        router = router.nest("/cache", crate::cache::router());
    }

    if config.features.enable_session {
        router = router.nest("/session", crate::session::router());
    }

    if config.features.enable_events {
        router = router.nest("/events", crate::events::router());
    }

    // Add service index and health check endpoints
    router = router.route("/", axum::routing::get(api_index));
    router = router.route("/health", axum::routing::get(health_check));
    router = router.route(
        "/status",
        axum::routing::get({
            let payload = status_payload.clone();
            move || {
                let body = payload.clone();
                async move { Json(body) }
            }
        }),
    );

    router
}

/// Service index endpoint for quick browser/manual checks.
async fn api_index() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "service": "xenobot-api",
        "status": "running",
        "health": "/health",
        "statusEndpoint": "/status",
        "endpoints": [
            "/chat",
            "/media",
            "/memory",
            "/merge",
            "/ai",
            "/llm",
            "/agent",
            "/embedding",
            "/core",
            "/nlp",
            "/network",
            "/cache",
            "/session",
            "/events"
        ]
    }))
}

/// Health check endpoint.
async fn health_check() -> &'static str {
    "OK"
}

fn build_status_payload(config: &ApiConfig) -> serde_json::Value {
    serde_json::json!({
        "service": "xenobot-api",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "running",
        "health": "/health",
        "statusEndpoint": "/status",
        "bindAddr": config.bind_addr.to_string(),
        "apiBasePath": config.api_base_path,
        "corsEnabled": config.enable_cors,
        "requestTimeoutSeconds": config.request_timeout_seconds,
        "maxBodySizeBytes": config.max_body_size,
        "runtime": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH
        },
        "features": {
            "chat": config.features.enable_chat,
            "merge": config.features.enable_merge,
            "ai": config.features.enable_ai,
            "llm": config.features.enable_llm,
            "agent": config.features.enable_agent,
            "embedding": config.features.enable_embedding,
            "core": config.features.enable_core,
            "nlp": config.features.enable_nlp,
            "network": config.features.enable_network,
            "cache": config.features.enable_cache,
            "session": config.features.enable_session,
            "events": config.features.enable_events,
            "wechat": config.features.enable_wechat
        }
    })
}

/// Create CORS layer based on configuration.
fn create_cors_layer(config: &ApiConfig) -> CorsLayer {
    let mut cors = tower_http::cors::CorsLayer::new();

    if config.cors_allowed_origins.is_empty() {
        cors = cors.allow_origin(tower_http::cors::Any);
    } else {
        let origins: Vec<_> = config
            .cors_allowed_origins
            .iter()
            .filter_map(|origin| origin.parse().ok())
            .collect();

        cors = cors.allow_origin(origins);
    }

    cors.allow_methods([
        axum::http::Method::GET,
        axum::http::Method::POST,
        axum::http::Method::PUT,
        axum::http::Method::DELETE,
        axum::http::Method::OPTIONS,
    ])
    .allow_headers([
        axum::http::header::CONTENT_TYPE,
        axum::http::header::AUTHORIZATION,
        axum::http::header::ACCEPT,
    ])
    .allow_credentials(config.enable_cors)
}

/// API route path builder.
pub struct ApiPathBuilder {
    base_path: String,
}

impl ApiPathBuilder {
    /// Create a new path builder with the given base path.
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: base_path.to_string(),
        }
    }

    /// Build full API path.
    pub fn build(&self, endpoint: &str) -> String {
        if endpoint.starts_with('/') {
            format!("{}{}", self.base_path, endpoint)
        } else {
            format!("{}/{}", self.base_path, endpoint)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Method, Request, StatusCode};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn root_route_returns_service_index_payload() {
        let app = build_router(&ApiConfig::default());
        let request = Request::builder()
            .method(Method::GET)
            .uri("/")
            .body(Body::empty())
            .expect("build request");
        let response = app.oneshot(request).await.expect("route response");
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
        assert_eq!(json["service"], "xenobot-api");
        assert_eq!(json["status"], "running");
        assert_eq!(json["health"], "/health");
        assert_eq!(json["statusEndpoint"], "/status");
        assert!(json["endpoints"].is_array());
    }

    #[tokio::test]
    async fn health_route_returns_ok_plain_text() {
        let app = build_router(&ApiConfig::default());
        let request = Request::builder()
            .method(Method::GET)
            .uri("/health")
            .body(Body::empty())
            .expect("build request");
        let response = app.oneshot(request).await.expect("route response");
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        assert_eq!(std::str::from_utf8(&bytes).expect("utf8"), "OK");
    }

    #[tokio::test]
    async fn status_route_returns_runtime_payload_and_feature_flags() {
        let app = build_router(&ApiConfig::default());
        let request = Request::builder()
            .method(Method::GET)
            .uri("/status")
            .body(Body::empty())
            .expect("build request");
        let response = app.oneshot(request).await.expect("route response");
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json body");
        assert_eq!(json["service"], "xenobot-api");
        assert_eq!(json["status"], "running");
        assert_eq!(json["health"], "/health");
        assert_eq!(json["statusEndpoint"], "/status");
        assert!(json["version"].is_string());
        assert!(json["bindAddr"].is_string());
        assert!(json["features"].is_object());
        assert_eq!(json["features"]["chat"], true);
        assert_eq!(json["features"]["session"], true);
        assert_eq!(json["runtime"]["arch"], std::env::consts::ARCH);
    }
}

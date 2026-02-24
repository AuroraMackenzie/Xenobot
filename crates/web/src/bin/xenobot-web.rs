//! Xenobot web server entry point.
//!
//! This binary starts an Axum HTTP server that serves the preserved Vue 3 frontend
//! and provides API endpoints and WebSocket real-time updates.

use axum::{extract::Extension, routing::get, Router};
use std::{net::SocketAddr, sync::Arc};

use tracing::{info, Level};
use tracing_subscriber::fmt;

use xenobot_api::config::ApiConfig;
use xenobot_api::router::build_router;
use xenobot_api::webhook_replay::spawn_webhook_dead_letter_replayer;
use xenobot_core::config::XenobotConfig;
use xenobot_web::assets::static_files_service;
use xenobot_web::websocket::{ws_handler, WebSocketState};

/// Main entry point.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    fmt().with_max_level(Level::INFO).with_target(false).init();

    info!("Starting Xenobot web server...");

    // Load configuration
    let _config = XenobotConfig::default().unwrap_or_default();
    let api_config = ApiConfig::default();
    let _replay_worker = spawn_webhook_dead_letter_replayer(&api_config);

    // Create WebSocket state
    let ws_state = Arc::new(WebSocketState::new());

    // Build API router
    let api_router = build_router(&api_config);

    // Build static file service
    let static_service = static_files_service();

    // Combine routers: API, WebSocket, static files (in order of precedence)
    let app = Router::new()
        // Add WebSocket state as extension
        .layer(Extension(ws_state))
        // Mount API under /api
        .nest("/api", api_router)
        // WebSocket endpoint
        .route("/ws", get(ws_handler))
        // Health check
        .route("/health", get(|| async { "OK" }))
        // Serve static files under root (catch-all for frontend)
        .merge(static_service);

    // Start server
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    info!("Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

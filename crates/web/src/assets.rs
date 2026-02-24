//! Static file serving and asset management.

use axum::{
    handler::HandlerWithoutStateExt,
    http::{StatusCode, Uri},
    Router,
};
use std::path::{Path, PathBuf};
use tower_http::services::ServeDir;

/// Serve static files from the frontend directory.
pub fn static_files_service() -> Router {
    // Path to frontend directory (relative to crate root)
    let frontend_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("frontend");
    let public_dir = frontend_dir.join("public");
    let dist_dir = frontend_dir.join("dist");

    // Try to serve from dist (built frontend) first, then public
    let serve_dir = if dist_dir.exists() {
        ServeDir::new(dist_dir)
    } else {
        ServeDir::new(public_dir)
    }
    .not_found_service(handle_404.into_service());

    Router::new()
        .nest_service("/", serve_dir)
        .fallback(handle_404)
}

/// 404 handler that serves index.html for SPA routing.
async fn handle_404(uri: Uri) -> (StatusCode, String) {
    (
        StatusCode::NOT_FOUND,
        format!("No route for {}", uri.path()),
    )
}

/// Get the path to the frontend directory.
pub fn frontend_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("frontend")
}

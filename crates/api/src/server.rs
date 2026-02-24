//! HTTP server implementation for Xenobot API.

use crate::{config::ApiConfig, router::build_router, webhook_replay, ApiError};
use axum::Router;
use std::fs;
use std::net::SocketAddr;
use tokio::signal;
use tracing::{info, warn};

/// HTTP server for Xenobot API.
pub struct ApiServer {
    config: ApiConfig,
    router: Router,
}

impl ApiServer {
    /// Create a new API server with the given configuration.
    pub fn new(config: ApiConfig) -> Self {
        let router = build_router(&config);

        Self { config, router }
    }

    /// Run the server until shutdown signal.
    pub async fn run(self) -> Result<(), ApiError> {
        let addr = self.config.bind_addr;
        let unix_socket_path = self.config.unix_socket_path.clone();
        let unix_socket_mode = self.config.unix_socket_mode;
        let replay_worker = webhook_replay::spawn_webhook_dead_letter_replayer(&self.config);

        info!("Starting Xenobot API server");
        if let Some(path) = unix_socket_path.as_ref() {
            info!("API transport: unix socket ({})", path.display());
        } else {
            info!("API transport: tcp ({})", addr);
        }
        info!("API base path: {}", self.config.api_base_path);
        info!("CORS enabled: {}", self.config.enable_cors);

        crate::database::init_database()
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?;

        // Build the final router with middleware
        let router = self.build_router_with_middleware();

        // Create shutdown signal
        let shutdown = shutdown_signal();

        if let Some(socket_path) = unix_socket_path {
            #[cfg(unix)]
            {
                validate_unix_socket_path(&socket_path)?;

                if let Some(parent) = socket_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        ApiError::Io(std::io::Error::new(
                            e.kind(),
                            format!(
                                "Failed to create unix socket directory {}: {}",
                                parent.display(),
                                e
                            ),
                        ))
                    })?;
                }
                if socket_path.exists() {
                    if socket_path.is_dir() {
                        return Err(ApiError::Io(std::io::Error::new(
                            std::io::ErrorKind::AlreadyExists,
                            format!(
                                "Unix socket path points to a directory: {}",
                                socket_path.display()
                            ),
                        )));
                    }
                    fs::remove_file(&socket_path).map_err(|e| {
                        ApiError::Io(std::io::Error::new(
                            e.kind(),
                            format!(
                                "Failed to remove stale unix socket {}: {}",
                                socket_path.display(),
                                e
                            ),
                        ))
                    })?;
                }

                let listener = tokio::net::UnixListener::bind(&socket_path).map_err(|e| {
                    ApiError::Io(std::io::Error::new(
                        e.kind(),
                        format!(
                            "Failed to bind unix socket {}: {}. \
On macOS sandbox, use a short writable path such as $TMPDIR/xenobot.sock or run `xenobot-cli api smoke`.",
                            socket_path.display(), e
                        ),
                    ))
                })?;
                apply_unix_socket_mode(&socket_path, unix_socket_mode)?;
                info!("Server listening on unix socket {}", socket_path.display());
                info!("Unix socket file mode: {:o}", unix_socket_mode & 0o777);

                serve_unix_listener(listener, router, shutdown).await?;

                let _ = fs::remove_file(&socket_path);
            }
            #[cfg(not(unix))]
            {
                return Err(ApiError::InvalidRequest(
                    "unix socket transport is not supported on this platform".to_string(),
                ));
            }
        } else {
            let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
                ApiError::Io(std::io::Error::new(
                    std::io::ErrorKind::AddrNotAvailable,
                    format!("Failed to bind to {}: {}", addr, e),
                ))
            })?;

            info!("Server listening on {}", addr);

            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown)
                .await
                .map_err(|e| ApiError::Http(format!("Server error: {}", e)))?;
        }

        if let Some(handle) = replay_worker {
            handle.abort();
            let _ = handle.await;
        }

        info!("Server shutdown complete");
        Ok(())
    }

    /// Build router with all middleware layers.
    fn build_router_with_middleware(&self) -> Router {
        let mut router = self.router.clone();

        // Add request timeout
        if self.config.request_timeout_seconds > 0 {
            router = router.layer(tower_http::timeout::TimeoutLayer::new(
                std::time::Duration::from_secs(self.config.request_timeout_seconds),
            ));
        }

        // Add request logging
        if self.config.enable_request_logging {
            router = router.layer(tower_http::trace::TraceLayer::new_for_http());
        }

        // Add compression
        if self.config.enable_compression {
            router = router.layer(tower_http::compression::CompressionLayer::new());
        }

        // Add body size limit
        router = router.layer(tower_http::limit::RequestBodyLimitLayer::new(
            self.config.max_body_size,
        ));

        router
    }

    /// Get the server address.
    pub fn addr(&self) -> SocketAddr {
        self.config.bind_addr
    }

    /// Get the API configuration.
    pub fn config(&self) -> &ApiConfig {
        &self.config
    }
}

#[cfg(unix)]
async fn serve_unix_listener(
    listener: tokio::net::UnixListener,
    router: Router,
    shutdown: impl std::future::Future<Output = ()>,
) -> Result<(), ApiError> {
    use hyper::server::conn::http1;
    use hyper::service::service_fn;
    use hyper_util::rt::TokioIo;
    use std::sync::Arc;
    use tower::util::ServiceExt;

    tokio::pin!(shutdown);
    let shared_router = Arc::new(router);

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                info!("Received shutdown signal for unix socket server");
                break;
            }
            accepted = listener.accept() => {
                let (stream, _) = accepted.map_err(|e| {
                    ApiError::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionAborted,
                        format!("Failed to accept unix socket connection: {}", e),
                    ))
                })?;
                let io = TokioIo::new(stream);
                let router_for_conn = shared_router.clone();

                tokio::spawn(async move {
                    let service = service_fn(move |request: hyper::Request<hyper::body::Incoming>| {
                        let svc = (*router_for_conn).clone().into_service();
                        let request = request.map(axum::body::Body::new);
                        async move {
                            let response = svc.oneshot(request).await.unwrap_or_else(|err| match err {});
                            Ok::<_, std::convert::Infallible>(response)
                        }
                    });

                    if let Err(err) = http1::Builder::new().serve_connection(io, service).await {
                        warn!("unix socket connection error: {}", err);
                    }
                });
            }
        }
    }

    Ok(())
}

#[cfg(unix)]
fn unix_socket_max_path_bytes() -> usize {
    if cfg!(target_os = "macos") {
        103
    } else {
        107
    }
}

#[cfg(unix)]
fn validate_unix_socket_path(socket_path: &std::path::Path) -> Result<(), ApiError> {
    use std::os::unix::ffi::OsStrExt;

    let path_bytes_len = socket_path.as_os_str().as_bytes().len();
    let max_len = unix_socket_max_path_bytes();
    if path_bytes_len > max_len {
        return Err(ApiError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "Unix socket path too long ({} bytes, max {} bytes on this platform): {}. \
Use a shorter path such as /tmp/xenobot.sock",
                path_bytes_len,
                max_len,
                socket_path.display()
            ),
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn apply_unix_socket_mode(socket_path: &std::path::Path, mode: u32) -> Result<(), ApiError> {
    use std::os::unix::fs::PermissionsExt;

    let normalized_mode = mode & 0o777;
    fs::set_permissions(socket_path, fs::Permissions::from_mode(normalized_mode)).map_err(|e| {
        ApiError::Io(std::io::Error::new(
            e.kind(),
            format!(
                "Failed to set unix socket mode {:o} for {}: {}",
                normalized_mode,
                socket_path.display(),
                e
            ),
        ))
    })
}

/// Create a shutdown signal for graceful shutdown.
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        info!("Received Ctrl+C signal, shutting down...");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
        info!("Received SIGTERM signal, shutting down...");
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

/// Utility function to start server from configuration.
pub async fn start_server(config: ApiConfig) -> Result<(), ApiError> {
    let server = ApiServer::new(config);
    server.run().await
}

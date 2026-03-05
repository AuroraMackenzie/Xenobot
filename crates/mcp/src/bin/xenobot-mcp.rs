//! Binary entrypoint for the Xenobot MCP server.

use clap::Parser;
use std::path::PathBuf;
use xenobot_mcp::config::McpServerConfig;
use xenobot_mcp::server::McpServer;

/// Run Xenobot MCP HTTP/SSE server.
#[derive(Debug, Parser)]
#[command(author, version, about = "Run Xenobot MCP server")]
struct Cli {
    /// Bind host/IP.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Bind port.
    #[arg(long, default_value_t = 8081)]
    port: u16,

    /// Override SQLite database path used by built-in tools.
    #[arg(long, env = "XENOBOT_DB_PATH")]
    db_path: Option<PathBuf>,

    /// Logical server name reported by initialize responses.
    #[arg(long, default_value = "Xenobot MCP Server")]
    name: String,

    /// Logical server version reported by initialize responses.
    #[arg(long, default_value = "0.1.0")]
    server_version: String,

    /// Disable SSE endpoint.
    #[arg(long, default_value_t = false)]
    no_sse: bool,

    /// Disable Streamable HTTP (`/mcp`) endpoint.
    #[arg(long, default_value_t = false)]
    no_streamable_http: bool,

    /// Maximum accepted message size in bytes.
    #[arg(long, default_value_t = 10 * 1024 * 1024)]
    max_message_size: usize,

    /// Allowed CORS origin (repeatable).
    #[arg(long = "allowed-origin")]
    allowed_origins: Vec<String>,

    /// Optional bearer token placeholder for future auth gate wiring.
    #[arg(long, env = "XENOBOT_MCP_AUTH_TOKEN")]
    auth_token: Option<String>,

    /// Additional MCP resource root (repeatable).
    #[arg(long = "resource-root")]
    resource_roots: Vec<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Some(db_path) = cli.db_path.as_ref() {
        std::env::set_var("XENOBOT_DB_PATH", db_path);
    }

    let config = McpServerConfig {
        bind_address: cli.host,
        port: cli.port,
        name: cli.name,
        version: cli.server_version,
        allowed_origins: cli.allowed_origins,
        auth_token: cli.auth_token,
        max_message_size: cli.max_message_size,
        enable_sse: !cli.no_sse,
        enable_streamable_http: !cli.no_streamable_http,
        resource_roots: cli.resource_roots,
        tools: Vec::new(),
    };

    println!("xenobot-mcp start requested");
    println!("bind: {}:{}", config.bind_address, config.port);
    println!("sse enabled: {}", config.enable_sse);
    println!("streamable http enabled: {}", config.enable_streamable_http);
    match std::env::var("XENOBOT_DB_PATH") {
        Ok(path) => println!("db path: {path}"),
        Err(_) => println!("db path: <default-user-data-dir>/xenobot/xenobot.db"),
    }

    McpServer::new(config)
        .start()
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(())
}

extern crate chrono;

// Build script for Xenobot MCP crate
//
// This script configures MCP protocol support with SSE and WebSocket transports.

fn main() {
    // Target information
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    println!("cargo:rustc-env=MCP_TARGET_OS={}", target_os);
    println!("cargo:rustc-env=MCP_TARGET_ARCH={}", target_arch);

    // Configure transport protocols
    println!("cargo:rustc-cfg=feature=\"mcp_sse\"");
    println!("cargo:rustc-env=HAS_MCP_SSE=1");

    // WebSocket support (via tungstenite)
    println!("cargo:rustc-cfg=feature=\"mcp_websocket\"");
    println!("cargo:rustc-env=HAS_MCP_WEBSOCKET=1");

    // Platform-specific MCP configurations
    match target_os.as_str() {
        "macos" => {
            println!("cargo:rustc-env=MCP_PLATFORM=macos");
            println!("cargo:rustc-cfg=mcp_platform_macos");

            if target_arch == "aarch64" || target_arch == "arm64" {
                println!("cargo:rustc-env=MCP_APPLE_SILICON=1");
                println!("cargo:rustc-cfg=mcp_apple_silicon");

                // Apple Silicon optimizations for MCP
                println!("cargo:rustc-env=MCP_OPTIMIZED_FOR_APPLE_SILICON=1");
            }
        }
        "linux" => {
            println!("cargo:rustc-env=MCP_PLATFORM=linux");
            println!("cargo:rustc-cfg=mcp_platform_linux");

            // Linux: better epoll support for high-concurrency MCP
            println!("cargo:rustc-cfg=mcp_use_epoll");
        }
        "windows" => {
            println!("cargo:rustc-env=MCP_PLATFORM=windows");
            println!("cargo:rustc-cfg=mcp_platform_windows");

            // Windows: IOCP for MCP async I/O
            println!("cargo:rustc-cfg=mcp_use_iocp");
        }
        _ => {
            println!("cargo:rustc-env=MCP_PLATFORM=unknown");
            println!("cargo:warning=Unknown MCP target OS: {}", target_os);
        }
    }

    // Feature configuration
    let has_wechat = cfg!(feature = "wechat");
    println!(
        "cargo:rustc-env=MCP_HAS_WECHAT={}",
        if has_wechat { "1" } else { "0" }
    );

    // MCP protocol version
    println!("cargo:rustc-env=MCP_PROTOCOL_VERSION=2024-11-05");

    // Default MCP configuration
    println!("cargo:rustc-env=MCP_DEFAULT_HOST=127.0.0.1");
    println!("cargo:rustc-env=MCP_DEFAULT_PORT=8081");
    println!("cargo:rustc-env=MCP_DEFAULT_SSE_PATH=/mcp/sse");
    println!("cargo:rustc-env=MCP_DEFAULT_WS_PATH=/mcp/ws");

    // Build timestamp
    println!(
        "cargo:rustc-env=MCP_BUILD_TIMESTAMP={}",
        chrono::Utc::now().to_rfc3339()
    );

    // Git info for MCP version
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("cargo:rustc-env=MCP_GIT_COMMIT={}", commit_hash);
        }
    }

    // MCP tool capabilities
    println!("cargo:rustc-env=MCP_TOOL_COUNT=12"); // 12 AI tools
    println!("cargo:rustc-env=MCP_SUPPORTED_TRANSPORTS=SSE,WebSocket,HTTP");

    // Re-run triggers
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src/");
}

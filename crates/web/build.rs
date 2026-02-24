//! Build script for Xenobot Web crate
//!
//! This script configures web frontend integration and static asset serving.

fn main() {
    // Target information
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    println!("cargo:rustc-env=WEB_TARGET_OS={}", target_os);
    println!("cargo:rustc-env=WEB_TARGET_ARCH={}", target_arch);
    println!("cargo:rustc-env=WEB_TARGET_ENV={}", target_env);

    // Web platform configuration
    match target_os.as_str() {
        "macos" => {
            println!("cargo:rustc-env=WEB_PLATFORM=macos");
            println!("cargo:rustc-cfg=web_platform_macos");

            if target_arch == "aarch64" || target_arch == "arm64" {
                println!("cargo:rustc-env=WEB_APPLE_SILICON=1");
                println!("cargo:rustc-cfg=web_apple_silicon");
            }
        }
        "linux" => {
            println!("cargo:rustc-env=WEB_PLATFORM=linux");
            println!("cargo:rustc-cfg=web_platform_linux");
        }
        "windows" => {
            println!("cargo:rustc-env=WEB_PLATFORM=windows");
            println!("cargo:rustc-cfg=web_platform_windows");
        }
        _ => {
            println!("cargo:rustc-env=WEB_PLATFORM=unknown");
            println!("cargo:warning=Unknown web target OS: {}", target_os);
        }
    }

    // WebSocket support configuration
    println!("cargo:rustc-cfg=feature=\"websocket\"");
    println!("cargo:rustc-env=HAS_WEBSOCKET=1");

    // Static asset serving configuration
    let static_dir = if cfg!(debug_assertions) {
        // Development: serve from source directory
        "../web-frontend/dist"
    } else {
        // Production: serve from embedded assets
        "embedded"
    };
    println!("cargo:rustc-env=WEB_STATIC_DIR={}", static_dir);

    // Build timestamp
    println!(
        "cargo:rustc-env=WEB_BUILD_TIMESTAMP={}",
        chrono::Utc::now().to_rfc3339()
    );

    // Git info for web version
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("cargo:rustc-env=WEB_GIT_COMMIT={}", commit_hash);
        }
    }

    // Default web server configuration
    println!("cargo:rustc-env=WEB_DEFAULT_HOST=127.0.0.1");
    println!("cargo:rustc-env=WEB_DEFAULT_PORT=3000");
    println!("cargo:rustc-env=WEB_API_PROXY_HOST=127.0.0.1");
    println!("cargo:rustc-env=WEB_API_PROXY_PORT=8080");

    // Frontend asset information (preserving existing Vue 3/TypeScript frontend)
    println!("cargo:rustc-env=WEB_FRONTEND_TECH=vue3_typescript");
    println!("cargo:rustc-env=WEB_FRONTEND_DIR=../web-frontend");

    // CORS configuration for web development
    match target_os.as_str() {
        "macos" | "linux" => {
            println!("cargo:rustc-env=WEB_CORS_ORIGINS=http://localhost:3000,http://127.0.0.1:3000,http://localhost:5173");
        }
        "windows" => {
            println!(
                "cargo:rustc-env=WEB_CORS_ORIGINS=http://localhost:3000,http://127.0.0.1:3000"
            );
        }
        _ => {
            println!("cargo:rustc-env=WEB_CORS_ORIGINS=*");
        }
    }

    // Check for frontend build
    let frontend_dist = std::path::Path::new("../web-frontend/dist");
    if frontend_dist.exists() {
        println!("cargo:rustc-env=WEB_HAS_BUILT_FRONTEND=1");
        println!("cargo:rustc-env=WEB_FRONTEND_BUILD_EXISTS=yes");
    } else {
        println!("cargo:rustc-env=WEB_HAS_BUILT_FRONTEND=0");
        println!("cargo:rustc-env=WEB_FRONTEND_BUILD_EXISTS=no");
    }

    // Re-run triggers
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=../web-frontend/dist");
}

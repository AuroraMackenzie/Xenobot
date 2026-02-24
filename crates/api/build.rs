//! Build script for Xenobot API crate
//!
//! This script configures API server build settings and SSE (Server-Sent Events) support.

fn main() {
    // Target information
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    println!("cargo:rustc-env=API_TARGET_OS={}", target_os);
    println!("cargo:rustc-env=API_TARGET_ARCH={}", target_arch);

    // Configure SSE support
    println!("cargo:rustc-cfg=feature=\"sse\"");
    println!("cargo:rustc-env=HAS_SSE=1");

    // Platform-specific configurations
    match target_os.as_str() {
        "macos" => {
            println!("cargo:rustc-env=API_PLATFORM=macos");
            println!("cargo:rustc-cfg=api_platform_macos");

            // macOS-specific optimizations for API server
            if target_arch == "aarch64" || target_arch == "arm64" {
                println!("cargo:rustc-env=API_APPLE_SILICON=1");
                println!("cargo:rustc-cfg=api_apple_silicon");
            }
        }
        "linux" => {
            println!("cargo:rustc-env=API_PLATFORM=linux");
            println!("cargo:rustc-cfg=api_platform_linux");

            // Linux-specific: epoll for better I/O performance
            println!("cargo:rustc-cfg=use_epoll");
        }
        "windows" => {
            println!("cargo:rustc-env=API_PLATFORM=windows");
            println!("cargo:rustc-cfg=api_platform_windows");

            // Windows-specific: IOCP for async I/O
            println!("cargo:rustc-cfg=use_iocp");
        }
        _ => {
            println!("cargo:rustc-env=API_PLATFORM=unknown");
            println!("cargo:warning=Unknown API target OS: {}", target_os);
        }
    }

    // Feature configuration
    let has_wechat = cfg!(feature = "wechat");
    println!(
        "cargo:rustc-env=HAS_WECHAT_FEATURE={}",
        if has_wechat { "1" } else { "0" }
    );

    // CORS configuration (adjust based on target)
    if target_os == "macos" || target_os == "linux" {
        println!("cargo:rustc-env=API_CORS_ORIGINS=*"); // Development default
    } else {
        println!("cargo:rustc-env=API_CORS_ORIGINS=http://localhost:3000,http://127.0.0.1:3000");
    }

    // Build timestamp
    println!(
        "cargo:rustc-env=API_BUILD_TIMESTAMP={}",
        chrono::Utc::now().to_rfc3339()
    );

    // Git info for API version
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("cargo:rustc-env=API_GIT_COMMIT={}", commit_hash);
        }
    }

    // Default API host and port
    println!("cargo:rustc-env=API_DEFAULT_HOST=127.0.0.1");
    println!("cargo:rustc-env=API_DEFAULT_PORT=8080");

    // Re-run triggers
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src/");
}

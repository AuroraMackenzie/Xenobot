extern crate chrono;

// Build script for Xenobot TUI crate
//
// This script configures TUI-specific build settings and terminal UI features.

fn main() {
    // Target information
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_family = std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();

    println!("cargo:rustc-env=TUI_TARGET_OS={}", target_os);
    println!("cargo:rustc-env=TUI_TARGET_ARCH={}", target_arch);
    println!("cargo:rustc-env=TUI_TARGET_FAMILY={}", target_family);

    // TUI platform configuration
    match target_os.as_str() {
        "macos" => {
            println!("cargo:rustc-env=TUI_PLATFORM=macos");
            println!("cargo:rustc-cfg=tui_platform_macos");

            if target_arch == "aarch64" || target_arch == "arm64" {
                println!("cargo:rustc-env=TUI_APPLE_SILICON=1");
                println!("cargo:rustc-cfg=tui_apple_silicon");
            }
        }
        "linux" => {
            println!("cargo:rustc-env=TUI_PLATFORM=linux");
            println!("cargo:rustc-cfg=tui_platform_linux");
        }
        "windows" => {
            println!("cargo:rustc-env=TUI_PLATFORM=windows");
            println!("cargo:rustc-cfg=tui_platform_windows");
        }
        _ => {
            println!("cargo:rustc-env=TUI_PLATFORM=unknown");
            println!("cargo:warning=Unknown TUI target OS: {}", target_os);
        }
    }

    // Feature configuration
    let has_api = cfg!(feature = "api");
    let has_wechat = cfg!(feature = "wechat");

    println!(
        "cargo:rustc-env=TUI_HAS_API={}",
        if has_api { "1" } else { "0" }
    );
    println!(
        "cargo:rustc-env=TUI_HAS_WECHAT={}",
        if has_wechat { "1" } else { "0" }
    );

    // TUI tab count - based on known TUI tabs
    println!("cargo:rustc-env=TUI_TAB_COUNT=6");

    // Terminal capabilities
    match target_family.as_str() {
        "unix" => {
            println!("cargo:rustc-env=TUI_TERMINAL_FAMILY=unix");
            println!("cargo:rustc-cfg=tui_terminal_unix");
        }
        "windows" => {
            println!("cargo:rustc-env=TUI_TERMINAL_FAMILY=windows");
            println!("cargo:rustc-cfg=tui_terminal_windows");
        }
        _ => {
            println!("cargo:rustc-env=TUI_TERMINAL_FAMILY=unknown");
            println!("cargo:warning=Unknown terminal family: {}", target_family);
        }
    }

    // Default TUI dimensions
    println!("cargo:rustc-env=TUI_DEFAULT_WIDTH=80");
    println!("cargo:rustc-env=TUI_DEFAULT_HEIGHT=24");

    // Build timestamp
    println!(
        "cargo:rustc-env=TUI_BUILD_TIMESTAMP={}",
        chrono::Utc::now().to_rfc3339()
    );

    // Git info for TUI version
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("cargo:rustc-env=TUI_GIT_COMMIT={}", commit_hash);
        }
    }

    // TUI color support
    match target_os.as_str() {
        "macos" | "linux" => {
            println!("cargo:rustc-env=TUI_COLOR_SUPPORT=truecolor");
            println!("cargo:rustc-env=TUI_HAS_TRUE_COLOR=1");
        }
        "windows" => {
            println!("cargo:rustc-env=TUI_COLOR_SUPPORT=256");
            println!("cargo:rustc-env=TUI_HAS_TRUE_COLOR=0");
        }
        _ => {
            println!("cargo:rustc-env=TUI_COLOR_SUPPORT=basic");
            println!("cargo:rustc-env=TUI_HAS_TRUE_COLOR=0");
        }
    }

    // Default TUI refresh rate (in milliseconds)
    println!("cargo:rustc-env=TUI_DEFAULT_REFRESH_MS=100");

    // Re-run triggers
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src/");
}

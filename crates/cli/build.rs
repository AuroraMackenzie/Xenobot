extern crate chrono;

// Build script for Xenobot CLI crate
//
// This script configures CLI-specific build settings and command-line features.

fn main() {
    // Target information
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    println!("cargo:rustc-env=CLI_TARGET_OS={}", target_os);
    println!("cargo:rustc-env=CLI_TARGET_ARCH={}", target_arch);
    println!("cargo:rustc-env=CLI_TARGET_ENV={}", target_env);

    // CLI platform configuration
    match target_os.as_str() {
        "macos" => {
            println!("cargo:rustc-env=CLI_PLATFORM=macos");
            println!("cargo:rustc-cfg=cli_platform_macos");

            if target_arch == "aarch64" || target_arch == "arm64" {
                println!("cargo:rustc-env=CLI_APPLE_SILICON=1");
                println!("cargo:rustc-cfg=cli_apple_silicon");
            }
        }
        "linux" => {
            println!("cargo:rustc-env=CLI_PLATFORM=linux");
            println!("cargo:rustc-cfg=cli_platform_linux");
        }
        "windows" => {
            println!("cargo:rustc-env=CLI_PLATFORM=windows");
            println!("cargo:rustc-cfg=cli_platform_windows");
        }
        _ => {
            println!("cargo:rustc-env=CLI_PLATFORM=unknown");
            println!("cargo:warning=Unknown CLI target OS: {}", target_os);
        }
    }

    // Feature configuration
    let has_api = cfg!(feature = "api");
    let has_wechat = cfg!(feature = "wechat");
    let has_analysis = cfg!(feature = "analysis");

    println!(
        "cargo:rustc-env=CLI_HAS_API={}",
        if has_api { "1" } else { "0" }
    );
    println!(
        "cargo:rustc-env=CLI_HAS_WECHAT={}",
        if has_wechat { "1" } else { "0" }
    );
    println!(
        "cargo:rustc-env=CLI_HAS_ANALYSIS={}",
        if has_analysis { "1" } else { "0" }
    );

    // Command count - based on known CLI commands
    println!("cargo:rustc-env=CLI_COMMAND_COUNT=11");

    // Default log level
    println!("cargo:rustc-env=CLI_DEFAULT_LOG_LEVEL=info");

    // Build timestamp
    println!(
        "cargo:rustc-env=CLI_BUILD_TIMESTAMP={}",
        chrono::Utc::now().to_rfc3339()
    );

    // Git info for CLI version
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("cargo:rustc-env=CLI_GIT_COMMIT={}", commit_hash);
        }
    }

    // CLI binary name
    let binary_name = if target_os == "windows" {
        "xenobot.exe"
    } else {
        "xenobot"
    };
    println!("cargo:rustc-env=CLI_BINARY_NAME={}", binary_name);

    // Default configuration paths
    match target_os.as_str() {
        "macos" => {
            println!(
                "cargo:rustc-env=CLI_CONFIG_PATH=~/Library/Application Support/xenobot/config.toml"
            );
            println!("cargo:rustc-env=CLI_DATA_PATH=~/Library/Application Support/xenobot/data");
        }
        "linux" => {
            println!("cargo:rustc-env=CLI_CONFIG_PATH=~/.config/xenobot/config.toml");
            println!("cargo:rustc-env=CLI_DATA_PATH=~/.local/share/xenobot");
        }
        "windows" => {
            println!("cargo:rustc-env=CLI_CONFIG_PATH=%APPDATA%\\xenobot\\config.toml");
            println!("cargo:rustc-env=CLI_DATA_PATH=%APPDATA%\\xenobot\\data");
        }
        _ => {
            println!("cargo:rustc-env=CLI_CONFIG_PATH=./config.toml");
            println!("cargo:rustc-env=CLI_DATA_PATH=./data");
        }
    }

    // Re-run triggers
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src/");
}

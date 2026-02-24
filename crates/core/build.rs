//! Build script for Xenobot Core crate
//!
//! This script configures core build-time settings and environment variables.

fn main() {
    // Set target information
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_family = std::env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();

    println!("cargo:rustc-env=TARGET_OS={}", target_os);
    println!("cargo:rustc-env=TARGET_ARCH={}", target_arch);
    println!("cargo:rustc-env=TARGET_FAMILY={}", target_family);

    // Platform-specific configurations
    match target_os.as_str() {
        "macos" => {
            println!("cargo:rustc-env=PLATFORM=macos");
            println!("cargo:rustc-cfg=platform_macos");

            if target_arch == "aarch64" || target_arch == "arm64" {
                println!("cargo:rustc-env=APPLE_SILICON=1");
                println!("cargo:rustc-cfg=apple_silicon");
            }
        }
        "linux" => {
            println!("cargo:rustc-env=PLATFORM=linux");
            println!("cargo:rustc-cfg=platform_linux");
        }
        "windows" => {
            println!("cargo:rustc-env=PLATFORM=windows");
            println!("cargo:rustc-cfg=platform_windows");
        }
        _ => {
            println!("cargo:rustc-env=PLATFORM=unknown");
            println!("cargo:warning=Unknown target OS: {}", target_os);
        }
    }

    // Core features configuration
    println!("cargo:rustc-env=BUILD_FEATURES=core");

    // Version information
    println!(
        "cargo:rustc-env=BUILD_TIMESTAMP={}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
    );

    // Git version info (if available)
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("cargo:rustc-env=GIT_COMMIT_HASH={}", commit_hash);
        }
    }

    if let Ok(output) = std::process::Command::new("git")
        .args(["describe", "--tags", "--dirty", "--always"])
        .output()
    {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("cargo:rustc-env=GIT_VERSION={}", version);
        }
    }

    // Re-run if build script changes
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
}

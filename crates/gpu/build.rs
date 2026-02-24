//! Build script for Xenobot GPU crate
//!
//! This script configures Metal MPS GPU acceleration for MacBook arm64 silicon.
//! It ensures the necessary Metal and MPS frameworks are available and
//! configures the build for optimal GPU performance.

fn main() {
    // Check for macOS arm64 target
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    println!("cargo:rustc-env=TARGET_OS={}", target_os);
    println!("cargo:rustc-env=TARGET_ARCH={}", target_arch);

    // Metal framework is only available on macOS
    if target_os == "macos" {
        println!("cargo:rustc-cfg=feature=\"metal\"");
        println!("cargo:rustc-env=HAS_METAL=1");

        // Metal framework linking for macOS
        println!("cargo:rustc-link-lib=framework=Metal");
        println!("cargo:rustc-link-lib=framework=MetalPerformanceShaders");
        println!("cargo:rustc-link-lib=framework=MetalPerformanceShadersGraph");

        // Additional Metal-related frameworks
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=CoreGraphics");

        // Check for arm64 silicon
        if target_arch == "aarch64" || target_arch == "arm64" {
            println!("cargo:rustc-cfg=feature=\"metal_mps\"");
            println!("cargo:rustc-env=HAS_METAL_MPS=1");
            println!("cargo:rustc-env=MACOS_ARM64=1");

            // Enable MPS-specific optimizations for Apple Silicon
            println!("cargo:rustc-cfg=apple_silicon");
            println!("cargo:rustc-env=APPLE_SILICON=1");
        } else {
            println!("cargo:rustc-env=HAS_METAL_MPS=0");
            println!("cargo:warning=Metal MPS GPU acceleration requires macOS arm64 (Apple Silicon). Falling back to CPU.");
        }
    } else {
        println!("cargo:rustc-env=HAS_METAL=0");
        println!("cargo:rustc-env=HAS_METAL_MPS=0");
        println!(
            "cargo:warning=Metal framework not available on {} {}. GPU acceleration disabled.",
            target_os, target_arch
        );
    }

    // Candle framework configuration
    println!("cargo:rustc-env=CANDLE_FEATURES=metal");

    // Enable GPU-specific features in Candle
    if cfg!(feature = "metal") {
        println!("cargo:rustc-cfg=candle_metal");
    }

    // Set optimization flags for GPU operations
    if cfg!(feature = "metal_mps") {
        println!("cargo:rustc-cfg=metal_mps");
        println!("cargo:rustc-env=RUSTFLAGS=-C target-cpu=apple-m1 -C target-feature=+neon,+fp-armv8,+fullfp16");
    }

    // Version information
    println!(
        "cargo:rustc-env=BUILD_TIMESTAMP={}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
    );

    // Re-run if build script changes
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
}

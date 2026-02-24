fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    println!("cargo:rustc-env=TARGET_OS={}", target_os);
    println!("cargo:rustc-env=TARGET_ARCH={}", target_arch);

    if target_os == "macos" {
        println!("cargo:rustc-env=HAS_MACOS=1");

        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=Security");
        println!("cargo:rustc-link-lib=framework=CoreServices");

        if target_arch == "aarch64" || target_arch == "arm64" {
            println!("cargo:rustc-env=MACOS_ARM64=1");
        }
    } else if target_os == "windows" {
        println!("cargo:rustc-env=HAS_WINDOWS=1");
    }

    println!("cargo:rerun-if-changed=build.rs");
}

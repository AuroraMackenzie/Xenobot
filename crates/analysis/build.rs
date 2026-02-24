fn main() {
    println!("cargo:rustc-env=BUILD_FEATURES=analysis");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "macos" {
        println!("cargo:rustc-cfg=feature=\"jieba\"");
    }

    println!("cargo:rerun-if-changed=build.rs");
}

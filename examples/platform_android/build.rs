//! Root build script — delegates to foundation_platform::codegen::build_wasm_app.
//! WasmBundleGenerator handles scanning, bin stub generation, compilation,
//! JS wrapper generation, and runtime copying.

use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let public_dir = root.join("src-tauri").join("public");

    // Primary WASM app: app/ → public/app/
    let app_dir = root.join("app");
    if app_dir.join("Cargo.toml").exists() {
        foundation_platform::codegen::build_wasm_app(&app_dir, &public_dir.join("app"));
    }

    // Second WASM app: app-hello/ → public/app-hello/
    let hello_dir = root.join("app-hello");
    if hello_dir.join("Cargo.toml").exists() {
        foundation_platform::codegen::build_wasm_app(&hello_dir, &public_dir.join("app-hello"));
    }
}

//! Root build script — delegates to foundation_platform::codegen::build_wasm_app.
//! WasmBundleGenerator handles scanning, bin stub generation, compilation,
//! JS wrapper generation, and runtime copying.

use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let app_dir = root.join("app");
    let public_dir = root.join("src-tauri").join("public");

    foundation_platform::codegen::build_wasm_app(&app_dir, &public_dir);
}

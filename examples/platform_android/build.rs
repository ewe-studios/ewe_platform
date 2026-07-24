//! Root build script — builds Surface 2 (WebView) apps and syncs resources.
//!
//! Surface 3 (wasmtime) apps cannot be built here: `build_wasmtime_app` spawns
//! a nested `cargo build`, which deadlocks on the workspace lock the outer
//! cargo holds while build scripts run. Build them BEFORE running this build:
//!
//! Every app lands in `src-tauri/public/{app_id}/v{version}/`. That is the
//! layout Tauri bundles, the asset manager serves from, and OTA writes into.

use std::path::PathBuf;

use foundation_platform::codegen;

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let src_tauri = root.join("src-tauri");
    let public_dir = src_tauri.join("public");

    // ── F42: Native module injection (when foundation_platform_native is added) ──
    // use foundation_platform_native::{PlatformCodegen, modal};
    // let mut codegen = PlatformCodegen::new();
    // modal::register(&mut codegen);
    // codegen.inject_native_code(&src_tauri);

    // ── Surface 2: WebView WASM apps (compiled inline, no cargo nesting) ──
    for app_id in ["app"] {
        let app_dir = root.join(app_id);
        if app_dir.join("Cargo.toml").exists() {
            let version = codegen::crate_version(&app_dir);
            codegen::build_wasm_app(
                &app_dir,
                &codegen::app_version_dir(&public_dir, app_id, &version),
            );
        }
    }

    // Surface 3: built externally — see the module doc above for why.
    // When app-shell.wasm is present in public/app-shell/v{version}/,
    // the sync below picks it up automatically.

    // Derive bundle.resources from what actually exists in public/.
    // This picks up Surface 2 AND Surface 3 apps — whichever were built
    // before this script runs become part of the bundle.
    codegen::sync_bundle_resources(&src_tauri);
}

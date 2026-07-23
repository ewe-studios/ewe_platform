//! Root build script — declares which apps this project publishes.
//!
//! Every app, whichever surface it targets, lands in the same shape:
//! `src-tauri/public/{app_id}/v{version}/`. That is the layout Tauri bundles,
//! the asset manager serves from, and an OTA writes into — one layout end to
//! end, so nothing translates between them.
//!
//! Each app's version comes from its OWN `Cargo.toml`. `app/` at v0.2.0 and
//! `app-hello/` at v0.1.0 naturally land at different directories — no shared
//! version file, no collision.

use std::path::PathBuf;

use foundation_platform::codegen;

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let src_tauri = root.join("src-tauri");
    let public_dir = src_tauri.join("public");

    // ── Surface 2: WebView WASM apps ──
    for app_id in ["app", "app-hello"] {
        let app_dir = root.join(app_id);
        if app_dir.join("Cargo.toml").exists() {
            let version = codegen::crate_version(&app_dir);
            codegen::build_wasm_app(
                &app_dir,
                &codegen::app_version_dir(&public_dir, app_id, &version),
            );
        }
    }

    // ── Surface 3: wasmtime shell apps ──
    for app_id in ["app-shell"] {
        let shell_dir = root.join(app_id);
        if shell_dir.join("Cargo.toml").exists() {
            let version = codegen::crate_version(&shell_dir);
            codegen::build_wasmtime_app(
                &shell_dir,
                &codegen::app_version_dir(&public_dir, app_id, &version),
            );
        }
    }

    // Register whatever was just built in tauri.conf.json's bundle.resources.
    // Derived from `public/` rather than from the lists above: an app built
    // but absent from the config ships with no assets and shows a blank page
    // on device.
    codegen::sync_bundle_resources(&src_tauri);
}

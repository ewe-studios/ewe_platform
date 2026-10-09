// Generated — AppAssets for "app" (F22)
// Route prefix: /app/
//
// MobileDirectory: assets served from disk at runtime via `build(root)`.
// The resolved resource dir comes from `PlatformSession.resource_root`.
// On Android this is the Tauri-extracted resource path; on desktop
// it is the app bundle resource dir.

use foundation_macros::MobileDirectory;
use foundation_platform::MobileApp;
use std::path::PathBuf;

#[derive(MobileDirectory)]
#[source = "$CARGO_MANIFEST_DIR/public/app"]
pub struct AppAssets {
    pub root: PathBuf
}

impl AppAssets {
    pub fn build(root: PathBuf) -> MobileApp<AppAssets> { MobileApp::new(AppAssets { root }) }
}

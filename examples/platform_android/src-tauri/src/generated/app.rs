// Generated — AppAssets for "app" (F22)
// Route prefix: /app/
// Embedded: AppAssets::build() — for APK builds (release bytes in .so)
// Mobile:    AppAssets::new(root) — for desktop dev / OTA (disk-backed)

use foundation_macros::EmbedDirectoryAs;
use foundation_platform::WebviewApp;

#[derive(EmbedDirectoryAs)]
#[source = "$CARGO_MANIFEST_DIR/public/app"]
pub struct AppAssets;

impl AppAssets {
    pub fn build() -> WebviewApp<AppAssets> { WebviewApp::new(AppAssets {}) }
}

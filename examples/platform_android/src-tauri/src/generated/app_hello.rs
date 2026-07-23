// Generated — AppAssets for "app-hello" (F22, mounted per F40)
// Route prefix: /app-hello/
//
// `build(session)` mounts the responder at this app's active version
// directory and lets it read through the session's asset manager.
// That indirection is what makes Android work: APK-bundled assets are
// never on disk, so only the VFS overlay can reach them.
//
// `build_at(root)` is the unmounted form — a plain disk root, for
// embeddings that have no PlatformAssetManager.

use foundation_macros::MobileDirectory;
use foundation_platform::{MobileApp, PlatformSession};
use std::path::PathBuf;

pub const APP_ID: &str = "app-hello";

#[derive(MobileDirectory)]
#[source = "$CARGO_MANIFEST_DIR/public/app-hello"]
pub struct AppAssets {
    pub root: PathBuf
}

impl AppAssets {
    pub fn build(session: &PlatformSession) -> MobileApp<AppAssets> {
        MobileApp::mounted_at(AppAssets { root: session.app_root(APP_ID) }, APP_ID)
    }

    pub fn build_at(root: PathBuf) -> MobileApp<AppAssets> { MobileApp::new(AppAssets { root }) }
}

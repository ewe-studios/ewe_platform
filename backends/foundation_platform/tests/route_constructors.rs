use foundation_platform::*;

#[test]
fn webview_app_sets_correct_defaults() {
    let d = webview_app();
    assert_eq!(d.source, RouteSource::WebviewApp);
    assert_eq!(d.view_kind, ViewKind::WebView);
    assert_eq!(d.profile, Profile::App);
    assert_eq!(d.cache_policy, CachePolicy::CacheFirst);
    assert_eq!(d.presentation, Presentation::Morph);
    assert!(d.native_view_id.is_none());
    assert!(d.target.is_none());
    assert!(d.capabilities.is_empty());
}

#[test]
fn ipc_shell_sets_correct_defaults() {
    let d = ipc_shell();
    assert_eq!(d.source, RouteSource::IpcShell);
    assert_eq!(d.profile, Profile::TrustedRemote);
    assert!(d.target.is_none());
}

#[test]
fn ipc_shell_with_sets_target() {
    let d = ipc_shell_with("business_logic");
    assert_eq!(d.source, RouteSource::IpcShell);
    assert_eq!(d.target.as_deref(), Some("business_logic"));
}

#[test]
fn remote_fetch_sets_correct_defaults() {
    let d = remote_fetch();
    assert_eq!(d.source, RouteSource::RemoteServer);
    assert_eq!(d.cache_policy, CachePolicy::NetworkFirst);
}

#[test]
fn builder_chain_overrides_defaults() {
    let d = remote_fetch()
        .with_presentation(Presentation::Push)
        .with_cache_policy(CachePolicy::CacheFirst)
        .with_profile(Profile::TrustedRemote)
        .with_allowed_capabilities(&[CapabilityId("camera".into())]);

    assert_eq!(d.presentation, Presentation::Push);
    assert_eq!(d.cache_policy, CachePolicy::CacheFirst);
    assert_eq!(d.profile, Profile::TrustedRemote);
    assert_eq!(d.capabilities.len(), 1);
}

#[test]
fn protocol_content_type_is_correct() {
    assert_eq!(Protocol::Columnar.content_type(), "application/primal-columnar");
    assert_eq!(Protocol::Arrow.content_type(), "application/primal-arrow");
    assert_eq!(Protocol::ArrowIpc.content_type(), "application/vnd.apache.arrow.stream");
    assert_eq!(Protocol::Json.content_type(), "application/primal-json");
    assert_eq!(Protocol::Html.content_type(), "text/html; charset=utf-8");
}

#[test]
fn builder_ignores_irrelevant_fields() {
    // native_view_id should be invisible in webview_app path
    let d = webview_app();
    assert!(d.native_view_id.is_none());

    // target should be invisible unless ipc_shell_with is used
    let d2 = ipc_shell();
    assert!(d2.target.is_none());
}

// ── MobileDirectory path resolution (F22 wiring, F40 layout) ──────────
//
// These check the *wiring*: that the example's `public/` tree really contains
// what a request resolves to. They drive the responder rather than
// re-implementing its path logic inline — a copy of `serve_response` here
// would keep passing after the real one changed, which is how the previous
// version of these tests survived the move to versioned directories while
// asserting a layout that no longer existed.

use foundation_platform::codegen::app_version_dir;
use foundation_platform::pattern::PatternRouter;
use foundation_platform::{webview_app, MobileApp, PlatformSession, RouteResponder};
use foundation_nostd::embeddable::FileInfo;
use foundation_nostd::mobile::MobileDirectory;
use foundation_ui_traits::{IntentSource, Method, NavigationIntent};

struct ExampleAssets {
    root: std::path::PathBuf,
}

impl MobileDirectory for ExampleAssets {
    const FILES_METADATA: &'static [FileInfo] = &[];
    fn root_str(&self) -> &str {
        self.root.to_str().unwrap_or("")
    }
}

fn example_src_tauri() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/platform_android/src-tauri")
}

/// Serve `url` from the example's real `public/app/v{version}/` directory.
fn serve_from_example(url: &str) -> tauri::http::Response<Vec<u8>> {
    let src_tauri = example_src_tauri();
    let version = "0.1.0"; // the example crate's version — what crate_version would return
    let root = app_version_dir(&src_tauri.join("public"), "app", version);
    assert!(
        root.is_dir(),
        "the example must have been built into its versioned layout; missing {root:?}"
    );

    let intent = NavigationIntent {
        url: url.to_string(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    };

    let mut router = PatternRouter::new();
    router.route("/app/*", webview_app());
    router.route("/app", webview_app());
    let decision = router
        .resolve_intent(&intent)
        .unwrap_or_else(|| panic!("no route matched {url}"));

    // No asset manager: reads go straight to disk under `root`, which is what
    // makes this a check of the on-disk layout rather than of the VFS.
    let session = PlatformSession::new_test(src_tauri);
    MobileApp::mounted_at(ExampleAssets { root }, "app").respond(&intent, &decision, &session)
}

#[test]
fn mobile_directory_serves_app_index_html() {
    let response = serve_from_example("ewe://localhost/app/");

    assert_eq!(response.status(), 200);
    let body = String::from_utf8_lossy(response.body()).to_string();
    assert!(
        body.contains("<!DOCTYPE html>"),
        "a directory request must serve the app's entry point, got: {body:.120}"
    );
}

#[test]
fn mobile_directory_serves_app_bundle_js() {
    let response = serve_from_example("ewe://localhost/app/bundle.js");

    assert_eq!(response.status(), 200);
    assert!(!response.body().is_empty(), "bundle.js should not be empty");
    assert_eq!(
        response.headers().get("content-type").and_then(|v| v.to_str().ok()),
        Some("application/javascript"),
    );
}

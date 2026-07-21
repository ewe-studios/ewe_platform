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

// ── MobileDirectory path resolution (F22 codegen wiring) ──────────

#[test]
fn mobile_directory_serves_app_index_html() {
    use foundation_platform::pattern::extract_path;
    use std::path::Path;

    // Exact same logic as serve_response + MobileDisk::read_utf8_for.
    let url = "ewe://localhost/app/";
    let path = extract_path(url).trim_start_matches('/').to_string();
    assert_eq!(path, "app/");

    let file = if Path::new(&path).extension().is_some() {
        path.clone()
    } else {
        format!("{path}index.html").trim_start_matches('/').to_string()
    };
    assert_eq!(file, "app/index.html");

    // The actual file on disk.
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/platform_android/src-tauri/public");
    let location = root.join(&file);
    assert!(location.exists(), "expected file at {:?}", location);

    let data = std::fs::read(&location).unwrap();
    let body = String::from_utf8_lossy(&data);
    assert!(!body.is_empty(), "index.html should not be empty");
    assert!(body.contains("<!DOCTYPE html>"), "should contain doctype");
}

#[test]
fn mobile_directory_serves_app_bundle_js() {
    use foundation_platform::pattern::extract_path;
    use std::path::Path;

    let url = "ewe://localhost/app/bundle.js";
    let path = extract_path(url).trim_start_matches('/').to_string();
    assert_eq!(path, "app/bundle.js");

    let file = if Path::new(&path).extension().is_some() {
        path.clone()
    } else {
        format!("{path}index.html").trim_start_matches('/').to_string()
    };
    assert_eq!(file, "app/bundle.js");

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/platform_android/src-tauri/public");
    let location = root.join(&file);
    assert!(location.exists(), "expected file at {:?}", location);

    let data = std::fs::read(&location).unwrap();
    assert!(!data.is_empty(), "bundle.js should not be empty");
}

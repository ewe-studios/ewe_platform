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

//! Mode page rendering tests moved out of `src/ewe.rs` (spec 70 feature 59,
//! rule 7).
//!
//! `mode_badge_for` and `build_mode_page` were private (dead-code allowed);
//! they are now public so their escaping behaviour stays covered.

use foundation_platform::ewe::{build_mode_page, mode_badge_for};
use foundation_platform::{
    CachePolicy, Presentation, Profile, ProtocolHint, RouteDecision, RouteSource, ViewKind,
};

#[test]
fn build_mode_page_works_with_json_body() {
    let d = RouteDecision {
        source: RouteSource::WebviewApp,
        presentation: Presentation::Morph,
        view_kind: ViewKind::WebView,
        protocol: ProtocolHint::Default,
        cache_policy: CachePolicy::CacheFirst,
        profile: Profile::App,
        native_view_id: None,
        target: None,
        capabilities: vec![],
        auth_origin: None,
        handler_id: None,
        sub_path: None,
    };
    let (cls, label) = mode_badge_for(&d);
    let html = build_mode_page("/app/home", &(cls, label), &d, r#"{"type":"wasm_signal"}"#);
    assert!(html.contains("ewe:///app/home"));
    assert!(html.contains("wasm_signal"));
    assert!(html.contains("App"));
    assert!(!html.contains("panic"));
}

#[test]
fn build_mode_page_escapes_html_in_body() {
    let d = RouteDecision {
        source: RouteSource::RemoteServer,
        presentation: Presentation::Morph,
        view_kind: ViewKind::WebView,
        protocol: ProtocolHint::Default,
        cache_policy: CachePolicy::NetworkFirst,
        profile: Profile::TrustedRemote,
        native_view_id: None,
        target: None,
        capabilities: vec![],
        auth_origin: None,
        handler_id: None,
        sub_path: None,
    };
    let (cls, label) = mode_badge_for(&d);
    let html = build_mode_page(
        "/remote/data",
        &(cls, label),
        &d,
        "<script>alert('xss')</script>",
    );
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script>alert"));
}

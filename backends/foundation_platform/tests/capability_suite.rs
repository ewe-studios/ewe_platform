//! Tests for the capability registry and 5-layer defense chain.

use std::sync::Arc;

use foundation_platform::capability::{test_native_registry, TestNativeCap};
use foundation_platform::*;

#[test]
fn invoke_unknown_capability_returns_error() {
    let reg = test_native_registry();
    let session = Arc::new(PlatformSession::new_test(std::path::PathBuf::from(".")));
    let request = NativeCapabilityRequest {
        id: "req-1".into(),
        page_identity: PageIdentity { session_id: session.session_id(), route: "/app".into(), visit_id: 0 },
        capability: "unknown".into(),
        action: "test".into(),
        payload: serde_json::Value::Null,
    };

    // We need the session to have an active page for stale-page guard to pass
    session.record_navigation("/app");

    let response = reg.invoke(&session, &request, None);
    assert!(response.status.is_err());
    assert!(response.status.unwrap_err().contains("unknown capability"));
}

#[test]
fn invoke_with_correct_capability_succeeds() {
    let reg = test_native_registry();
    let session = Arc::new(PlatformSession::new_test(std::path::PathBuf::from(".")));
    let route = remote_fetch()
        .with_profile(Profile::App)
        .with_allowed_capabilities(&[CapabilityId("camera".into())]);

    session.record_navigation("/camera");
    let active = session.active_page_identity().unwrap();

    let request = NativeCapabilityRequest {
        id: "req-1".into(),
        page_identity: active,
        capability: "camera".into(),
        action: "capture".into(),
        payload: serde_json::Value::Null,
    };

    let response = reg.invoke(&session, &request, Some(&route));
    assert!(response.status.is_ok());
}

#[test]
fn profile_too_low_denies_capability() {
    let reg = test_native_registry();
    let session = Arc::new(PlatformSession::new_test(std::path::PathBuf::from(".")));
    // UntrustedRemote is below the camera's min_profile (TrustedRemote)
    let route = remote_fetch()
        .with_profile(Profile::UntrustedRemote);

    session.record_navigation("/app");
    let active = session.active_page_identity().unwrap();

    let request = NativeCapabilityRequest {
        id: "req-1".into(),
        page_identity: active,
        capability: "camera".into(),
        action: "capture".into(),
        payload: serde_json::Value::Null,
    };

    let response = reg.invoke(&session, &request, Some(&route));
    assert!(response.status.is_err());
    assert!(response.status.unwrap_err().contains("profile"));
}

#[test]
fn per_route_allowlist_blocks_unlisted_capability() {
    let reg = test_native_registry();
    let session = Arc::new(PlatformSession::new_test(std::path::PathBuf::from(".")));
    // App profile allows NativeApi, but camera is NOT in the allowlist
    let route = webview_app()
        .with_allowed_capabilities(&[CapabilityId("microphone".into())]); // camera not here

    session.record_navigation("/chat");
    let active = session.active_page_identity().unwrap();

    let request = NativeCapabilityRequest {
        id: "req-1".into(),
        page_identity: active,
        capability: "camera".into(),
        action: "capture".into(),
        payload: serde_json::Value::Null,
    };

    let response = reg.invoke(&session, &request, Some(&route));
    assert!(response.status.is_err());
    assert!(response.status.unwrap_err().contains("not allowed on this route"));
}

#[test]
fn stale_page_guard_rejects_old_request() {
    let reg = test_native_registry();
    let session = Arc::new(PlatformSession::new_test(std::path::PathBuf::from(".")));

    // Record a page visit, then navigate away
    let old_page = session.record_navigation("/app/old");
    session.record_navigation("/app/new"); // old_page is now stale

    let request = NativeCapabilityRequest {
        id: "req-1".into(),
        page_identity: old_page, // stale!
        capability: "camera".into(),
        action: "capture".into(),
        payload: serde_json::Value::Null,
    };

    let response = reg.invoke(&session, &request, None);
    assert!(response.status.is_err());
    assert!(response.status.unwrap_err().contains("stale page"));
}

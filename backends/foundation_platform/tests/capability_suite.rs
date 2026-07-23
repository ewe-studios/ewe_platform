//! Tests for the platform capability registry with 5-layer defense.

use std::sync::Arc;

use foundation_platform::capability::{test_registry, TestPlatformIpc};
use foundation_platform::*;
use foundation_wasm::ipc::{Ipc, IpcContentType, IpcRequest};

#[test]
fn invoke_unknown_capability_returns_error() {
    let reg = test_registry();
    let session = Arc::new(PlatformSession::new_test(std::path::PathBuf::from(".")));
    session.record_navigation("/app");
    let active = session.active_page_identity().unwrap();

    let request = IpcRequest {
        ipc: "unknown".into(),
        action: "test".into(),
        payload: vec![],
        content_type: IpcContentType::Json,
            target: None,
    };

    let result = reg.invoke(&session, &request, &active, None);
    assert!(result.is_err());
    assert!(format!("{result:?}").contains("UnknownIpc"));
}

#[test]
fn invoke_with_correct_capability_succeeds() {
    let reg = test_registry();
    let session = Arc::new(PlatformSession::new_test(std::path::PathBuf::from(".")));
    let route = remote_fetch()
        .with_profile(Profile::App)
        .with_allowed_capabilities(&[CapabilityId("camera".into())]);

    session.record_navigation("/camera");
    let active = session.active_page_identity().unwrap();

    let request = IpcRequest {
        ipc: "camera".into(),
        action: "capture".into(),
        payload: vec![],
        content_type: IpcContentType::Json,
            target: None,
    };

    assert!(reg.invoke(&session, &request, &active, Some(&route)).is_ok());
}

#[test]
fn profile_too_low_denies_capability() {
    let reg = test_registry();
    let session = Arc::new(PlatformSession::new_test(std::path::PathBuf::from(".")));
    let route = remote_fetch().with_profile(Profile::UntrustedRemote);

    session.record_navigation("/app");
    let active = session.active_page_identity().unwrap();

    let request = IpcRequest {
        ipc: "camera".into(),
        action: "capture".into(),
        payload: vec![],
        content_type: IpcContentType::Json,
            target: None,
    };

    let result = reg.invoke(&session, &request, &active, Some(&route));
    assert!(result.is_err());
    assert!(format!("{result:?}").contains("profile"));
}

#[test]
fn per_route_allowlist_blocks_unlisted_capability() {
    let reg = test_registry();
    let session = Arc::new(PlatformSession::new_test(std::path::PathBuf::from(".")));
    let route = webview_app()
        .with_allowed_capabilities(&[CapabilityId("microphone".into())]);

    session.record_navigation("/chat");
    let active = session.active_page_identity().unwrap();

    let request = IpcRequest {
        ipc: "camera".into(),
        action: "capture".into(),
        payload: vec![],
        content_type: IpcContentType::Json,
            target: None,
    };

    let result = reg.invoke(&session, &request, &active, Some(&route));
    assert!(result.is_err());
    assert!(format!("{result:?}").contains("not allowed"));
}

#[test]
fn stale_page_guard_rejects_old_request() {
    let reg = test_registry();
    let session = Arc::new(PlatformSession::new_test(std::path::PathBuf::from(".")));

    let old_page = session.record_navigation("/app/old");
    session.record_navigation("/app/new");

    let request = IpcRequest {
        ipc: "camera".into(),
        action: "capture".into(),
        payload: vec![],
        content_type: IpcContentType::Json,
            target: None,
    };

    let result = reg.invoke(&session, &request, &old_page, None);
    assert!(result.is_err());
    assert!(format!("{result:?}").contains("stale page"));
}

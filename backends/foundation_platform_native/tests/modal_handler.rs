//! Integration tests for the modal IPC handler (F42).
//!
//! Validates that ModalIpc correctly handles present_modal / dismiss_modal
//! actions through the platform session.

use foundation_platform::PlatformSession;
use foundation_platform_native::modal;

#[test]
fn modal_register_adds_handler_to_session() {
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));
    modal::register(&session);

    // The handler should now be registered under "chrome"
    let handler = session.get_ipc("chrome");
    assert!(handler.is_some(), "ModalIpc should be registered as 'chrome'");
}

#[test]
fn modal_present_returns_modal_id() {
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));
    modal::register(&session);

    let handler = session.get_ipc("chrome").unwrap();
    let request = foundation_wasm::ipc::IpcRequest {
        ipc: "chrome".into(),
        action: "present_modal".into(),
        payload: br#"{"route":"/app/settings","style":"bottom_sheet","title":"Settings"}"#.to_vec(),
        content_type: foundation_wasm::ipc::IpcContentType::Json,
        target: None,
    };

    let response = handler
        .invoke_with_session(&session, &request)
        .expect("present_modal should succeed");

    let body: serde_json::Value = serde_json::from_slice(&response.payload).unwrap();
    assert!(body["modal_id"].as_str().is_some(), "response should contain modal_id");
    assert!(body["modal_id"].as_str().unwrap().starts_with("modal_"));
}

#[test]
fn modal_dismiss_returns_ok() {
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));
    modal::register(&session);

    let handler = session.get_ipc("chrome").unwrap();
    let request = foundation_wasm::ipc::IpcRequest {
        ipc: "chrome".into(),
        action: "dismiss_modal".into(),
        payload: br#"{"modal_id":"modal_1"}"#.to_vec(),
        content_type: foundation_wasm::ipc::IpcContentType::Json,
        target: None,
    };

    let response = handler
        .invoke_with_session(&session, &request)
        .expect("dismiss_modal should succeed");

    assert_eq!(response.payload, br#"{"ok":true}"#);
}

#[test]
fn modal_unknown_action_errors() {
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));
    modal::register(&session);

    let handler = session.get_ipc("chrome").unwrap();
    let request = foundation_wasm::ipc::IpcRequest {
        ipc: "chrome".into(),
        action: "nonexistent".into(),
        payload: b"{}".to_vec(),
        content_type: foundation_wasm::ipc::IpcContentType::Json,
        target: None,
    };

    let result = handler.invoke_with_session(&session, &request);
    assert!(result.is_err(), "unknown action should error");
}

#[test]
fn modal_handler_has_correct_kind() {
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));
    modal::register(&session);

    let handler = session.get_ipc("chrome").unwrap();
    assert_eq!(
        handler.kind(),
        foundation_wasm::ipc::IpcKind::Capability
    );
}

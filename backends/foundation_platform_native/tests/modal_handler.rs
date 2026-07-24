//! Integration tests for the modal IPC handler (F42 / F43 callback-based).

use std::sync::Arc;
use foundation_platform::ipc::{IpcCallback, PlatformIpc};
use foundation_platform::PlatformSession;
use foundation_platform_native::native::modal;

/// Shorthand: wrap callback-based invoke_with_session into a sync result.
fn invoke_sync(
    handler: &dyn PlatformIpc,
    request: &foundation_wasm::ipc::IpcRequest<Vec<u8>>,
) -> Result<foundation_wasm::ipc::IpcResponse<Vec<u8>>, foundation_wasm::ipc::IpcError> {
    let (tx, rx) = std::sync::mpsc::channel();
    handler.invoke_with_session(request, Box::new(move |r| { let _ = tx.send(r); })).unwrap();
    rx.recv().unwrap()
}

fn new_session() -> Arc<PlatformSession> {
    PlatformSession::new_test(std::path::PathBuf::from("."))
}

#[test]
fn modal_register_adds_handler_to_session() {
    let session = new_session();
    modal::register(Arc::clone(&session));
    let handler = session.get_ipc("chrome");
    assert!(handler.is_some(), "ModalIpc should be registered as 'chrome'");
}

#[test]
fn modal_present_returns_modal_id() {
    let session = new_session();
    modal::register(Arc::clone(&session));
    let handler = session.get_ipc("chrome").unwrap();
    let request = foundation_wasm::ipc::IpcRequest {
        ipc: "chrome".into(), action: "present_modal".into(),
        payload: br#"{"route":"/app/settings","style":"bottom_sheet","title":"Settings"}"#.to_vec(),
        content_type: foundation_wasm::ipc::IpcContentType::Json, target: None,
    };
    let response = invoke_sync(handler, &request).expect("present_modal should succeed");
    let body: serde_json::Value = serde_json::from_slice(&response.payload).unwrap();
    assert!(body["modal_id"].as_str().is_some());
    assert!(body["modal_id"].as_str().unwrap().starts_with("modal_"));
}

#[test]
fn modal_dismiss_returns_ok() {
    let session = new_session();
    modal::register(Arc::clone(&session));
    let handler = session.get_ipc("chrome").unwrap();
    let request = foundation_wasm::ipc::IpcRequest {
        ipc: "chrome".into(), action: "dismiss_modal".into(),
        payload: br#"{"modal_id":"modal_1"}"#.to_vec(),
        content_type: foundation_wasm::ipc::IpcContentType::Json, target: None,
    };
    let response = invoke_sync(handler, &request).expect("dismiss_modal should succeed");
    assert_eq!(response.payload, br#"{"ok":true}"#);
}

#[test]
fn modal_unknown_action_errors() {
    let session = new_session();
    modal::register(Arc::clone(&session));
    let handler = session.get_ipc("chrome").unwrap();
    let request = foundation_wasm::ipc::IpcRequest {
        ipc: "chrome".into(), action: "nonexistent".into(),
        payload: b"{}".to_vec(), content_type: foundation_wasm::ipc::IpcContentType::Json,
        target: None,
    };
    let result = invoke_sync(handler, &request);
    assert!(result.is_err(), "unknown action should error");
}

#[test]
fn modal_handler_has_correct_kind() {
    let session = new_session();
    modal::register(Arc::clone(&session));
    let handler = session.get_ipc("chrome").unwrap();
    assert_eq!(handler.kind(), foundation_wasm::ipc::IpcKind::Capability);
}

#[test]
fn modal_dismiss_all_returns_ok() {
    let session = new_session();
    modal::register(Arc::clone(&session));
    let handler = session.get_ipc("chrome").unwrap();
    let request = foundation_wasm::ipc::IpcRequest {
        ipc: "chrome".into(), action: "dismiss_all_modals".into(),
        payload: b"{}".to_vec(), content_type: foundation_wasm::ipc::IpcContentType::Json,
        target: None,
    };
    let response = invoke_sync(handler, &request).expect("dismiss_all_modals should succeed");
    assert_eq!(response.payload, br#"{"ok":true}"#);
}

#[test]
fn modal_present_after_first_increments_depth() {
    let session = new_session();
    modal::register(Arc::clone(&session));
    let handler = session.get_ipc("chrome").unwrap();

    let req1 = foundation_wasm::ipc::IpcRequest {
        ipc: "chrome".into(), action: "present_modal".into(),
        payload: br#"{"route":"/app/a"}"#.to_vec(),
        content_type: foundation_wasm::ipc::IpcContentType::Json, target: None,
    };
    let resp1 = invoke_sync(handler, &req1).unwrap();
    let body1: serde_json::Value = serde_json::from_slice(&resp1.payload).unwrap();
    assert!(body1["modal_id"].as_str().unwrap().starts_with("modal_"));

    let req2 = foundation_wasm::ipc::IpcRequest {
        ipc: "chrome".into(), action: "present_modal".into(),
        payload: br#"{"route":"/app/b"}"#.to_vec(),
        content_type: foundation_wasm::ipc::IpcContentType::Json, target: None,
    };
    let resp2 = invoke_sync(handler, &req2).unwrap();
    let body2: serde_json::Value = serde_json::from_slice(&resp2.payload).unwrap();
    assert!(body2["modal_id"].as_str().unwrap().starts_with("modal_"));
    assert_ne!(body1["modal_id"], body2["modal_id"]);

    assert_eq!(session.webview_stack().depth(), 3);

    let top_label = body2["modal_id"].as_str().unwrap();
    let dismiss = foundation_wasm::ipc::IpcRequest {
        ipc: "chrome".into(), action: "dismiss_modal".into(),
        payload: serde_json::to_vec(&serde_json::json!({"modal_id": top_label})).unwrap(),
        content_type: foundation_wasm::ipc::IpcContentType::Json, target: None,
    };
    invoke_sync(handler, &dismiss).unwrap();
    assert_eq!(session.webview_stack().depth(), 2);

    let dismiss_all = foundation_wasm::ipc::IpcRequest {
        ipc: "chrome".into(), action: "dismiss_all_modals".into(),
        payload: b"{}".to_vec(), content_type: foundation_wasm::ipc::IpcContentType::Json,
        target: None,
    };
    invoke_sync(handler, &dismiss_all).unwrap();
    assert_eq!(session.webview_stack().depth(), 1, "root slot remains after dismiss_all");
}

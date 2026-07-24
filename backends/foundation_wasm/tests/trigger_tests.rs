//! F41 — trigger tests (was F27 capability+IPC trigger tests).
//!
//! Capability and IPC triggers are now unified through `IpcRequest`/`IpcResponse`.

use foundation_wasm::ipc::{IpcContentType, IpcError, IpcRequest, IpcResponse};
use foundation_wasm::TriggerRegistry;

// ── IPC trigger tests ──────────────────────────────────────────────────

#[test]
fn ipc_trigger_dispatches_to_registered_handler() {
    let mut reg = TriggerRegistry::new();
    reg.set_ipc_handler(|req: IpcRequest<Vec<u8>>| {
        Ok(IpcResponse {
            payload: req.payload.clone(),
            content_type: req.content_type,
        })
    });

    let req = IpcRequest {
        ipc: "echo".into(),
        action: "ping".into(),
        payload: b"hello".to_vec(),
        content_type: IpcContentType::Json,
        target: None,
    };

    let resp = reg.dispatch_ipc(req).unwrap();
    assert_eq!(resp.payload, b"hello");
}

#[test]
fn ipc_trigger_errors_when_no_handler() {
    let reg = TriggerRegistry::new();
    let req = IpcRequest {
        ipc: "nonexistent".into(),
        action: "do".into(),
        payload: vec![],
        content_type: IpcContentType::Json,
        target: None,
    };

    match reg.dispatch_ipc(req) {
        Err(IpcError::ExecutionFailed) => {}
        other => panic!("expected ExecutionFailed, got {other:?}"),
    }
}

#[test]
fn ipc_trigger_replacement() {
    let mut reg = TriggerRegistry::new();

    reg.set_ipc_handler(|_| {
        Ok(IpcResponse {
            payload: b"first".to_vec(),
            content_type: IpcContentType::Json,
        })
    });

    reg.set_ipc_handler(|_| {
        Ok(IpcResponse {
            payload: b"second".to_vec(),
            content_type: IpcContentType::Json,
        })
    });

    let req = IpcRequest {
        ipc: "dummy".into(),
        action: "dummy".into(),
        payload: vec![],
        content_type: IpcContentType::Json,
        target: None,
    };
    let resp = reg.dispatch_ipc(req).unwrap();
    assert_eq!(resp.payload, b"second");
}

#[test]
fn trigger_handlers_receive_request_by_value() {
    let mut reg = TriggerRegistry::new();
    reg.set_ipc_handler(|req: IpcRequest<Vec<u8>>| {
        // Handler OWNS the request — can move fields out
        let _owned: String = req.ipc;
        let _owned: Vec<u8> = req.payload;
        Ok(IpcResponse {
            payload: vec![],
            content_type: IpcContentType::Json,
        })
    });

    let req = IpcRequest {
        ipc: "test".into(),
        action: "test".into(),
        payload: b"data".to_vec(),
        content_type: IpcContentType::Json,
        target: None,
    };
    assert!(reg.dispatch_ipc(req).is_ok());
}

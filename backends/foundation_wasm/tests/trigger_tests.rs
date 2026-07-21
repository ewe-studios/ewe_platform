//! WHY: F27 — `TriggerRegistry` must correctly dispatch capability and IPC
//! triggers from host → WASM, and return errors when no handler is registered.
//!
//! WHAT: set handler → dispatch → verify response; dispatch without handler
//! → error; replace handler → new handler used.
//!
//! HOW: mock handlers that echo or transform the request.

use foundation_wasm::{
    CapabilityError, CapabilityContentType, CapabilityRequest, CapabilityResponse,
    TriggerRegistry,
};
use foundation_wasm::ipc::{IpcContentType, IpcError, IpcRequest, IpcResponse};

// ── Capability trigger tests ───────────────────────────────────────────

#[test]
fn cap_trigger_dispatches_to_registered_handler() {
    let mut reg = TriggerRegistry::new();
    reg.set_capability_handler(|req: CapabilityRequest<Vec<u8>>| {
        Ok(CapabilityResponse {
            capability: req.capability.clone(),
            action: req.action.clone(),
            payload: b"handled".to_vec(),
            content_type: CapabilityContentType::Json,
        })
    });

    let req = CapabilityRequest {
        capability: "camera".into(),
        action: "capture".into(),
        payload: b"{}".to_vec(),
        content_type: CapabilityContentType::Json,
    };

    let resp = reg.dispatch_capability(req).unwrap();
    assert_eq!(resp.capability, "camera");
    assert_eq!(resp.action, "capture");
    assert_eq!(resp.payload, b"handled");
}

#[test]
fn cap_trigger_errors_when_no_handler() {
    let reg = TriggerRegistry::new();
    let req = CapabilityRequest {
        capability: "clipboard".into(),
        action: "read".into(),
        payload: vec![],
        content_type: CapabilityContentType::Json,
    };

    match reg.dispatch_capability(req) {
        Err(CapabilityError::ExecutionFailed(msg)) => {
            assert!(msg.contains("no capability trigger handler"));
        }
        other => panic!("expected ExecutionFailed, got {other:?}"),
    }
}

#[test]
fn cap_trigger_replacement() {
    let mut reg = TriggerRegistry::new();

    reg.set_capability_handler(|_| {
        Ok(CapabilityResponse {
            capability: "dummy".into(),
            action: "dummy".into(),
            payload: b"first".to_vec(),
            content_type: CapabilityContentType::Json,
        })
    });

    // Replace
    reg.set_capability_handler(|_| {
        Ok(CapabilityResponse {
            capability: "dummy".into(),
            action: "dummy".into(),
            payload: b"second".to_vec(),
            content_type: CapabilityContentType::Json,
        })
    });

    let req = CapabilityRequest {
        capability: "dummy".into(),
        action: "dummy".into(),
        payload: vec![],
        content_type: CapabilityContentType::Json,
    };
    let resp = reg.dispatch_capability(req).unwrap();
    assert_eq!(resp.payload, b"second");
}

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
        Err(IpcError::ExecutionFailed(msg)) => {
            assert!(msg.contains("no IPC trigger handler"));
        }
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
fn trigger_handlers_independent() {
    let mut reg = TriggerRegistry::new();

    // Only IPC handler registered, capability should fail
    reg.set_ipc_handler(|_| {
        Ok(IpcResponse {
            payload: b"ipc-ok".to_vec(),
            content_type: IpcContentType::Json,
        })
    });

    let cap_req = CapabilityRequest {
        capability: "cam".into(),
        action: "do".into(),
        payload: vec![],
        content_type: CapabilityContentType::Json,
    };
    assert!(reg.dispatch_capability(cap_req).is_err());

    // But IPC still works
    let ipc_req = IpcRequest {
        ipc: "test".into(),
        action: "do".into(),
        payload: b"x".to_vec(),
        content_type: IpcContentType::Json,
        target: None,
    };
    assert!(reg.dispatch_ipc(ipc_req).is_ok());
}

#[test]
fn trigger_handlers_receive_request_by_value() {
    let mut reg = TriggerRegistry::new();
    reg.set_capability_handler(|req: CapabilityRequest<Vec<u8>>| {
        // Handler OWNS the request — can move fields out
        let _owned: String = req.capability;
        let _owned: Vec<u8> = req.payload;
        Ok(CapabilityResponse {
            capability: "ok".into(),
            action: "ok".into(),
            payload: vec![],
            content_type: CapabilityContentType::Json,
        })
    });

    let req = CapabilityRequest {
        capability: "test".into(),
        action: "test".into(),
        payload: b"data".to_vec(),
        content_type: CapabilityContentType::Json,
    };
    assert!(reg.dispatch_capability(req).is_ok());
}

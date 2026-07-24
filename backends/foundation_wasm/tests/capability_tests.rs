//! F41 — unified IPC tests (was F23 capability tests).
//!
//! Tests the `Ipc` trait, `IpcRegistry`, `IpcRequest`/`IpcResponse`, and
//! name validation — all through the new unified API.

use foundation_wasm::ipc::{
    is_valid_ipc_name, Ipc, IpcContentType, IpcError, IpcKind, IpcRegistry, IpcRequest, IpcResponse,
};

// ── Mock IPCs ───────────────────────────────────────────────────────────

struct EchoIpc {
    name: &'static str,
}

impl Ipc<Vec<u8>, Vec<u8>> for EchoIpc {
    fn name(&self) -> &str {
        self.name
    }
    fn kind(&self) -> IpcKind {
        IpcKind::Query
    }

    fn invoke(&self, request: &IpcRequest) -> Result<IpcResponse, IpcError> {
        Ok(IpcResponse {
            payload: request.payload.clone(),
            content_type: request.content_type,
        })
    }
}

struct FailingIpc;

impl Ipc<Vec<u8>, Vec<u8>> for FailingIpc {
    fn name(&self) -> &str {
        "fail"
    }
    fn kind(&self) -> IpcKind {
        IpcKind::Query
    }

    fn invoke(&self, _request: &IpcRequest) -> Result<IpcResponse, IpcError> {
        Err(IpcError::ExecutionFailed)
    }
}

// ── Registry tests ──────────────────────────────────────────────────────

#[test]
fn registry_register_and_invoke() {
    let reg = IpcRegistry::new();
    reg.register(EchoIpc { name: "echo" });

    let req = IpcRequest {
        ipc: "echo".into(),
        action: "ping".into(),
        payload: b"hello".to_vec(),
        content_type: IpcContentType::Json,
        target: None,
    };

    let resp = reg.invoke(&req).unwrap();
    assert_eq!(resp.payload, b"hello");
}

#[test]
fn registry_unknown_ipc() {
    let reg = IpcRegistry::new();
    let req = IpcRequest {
        ipc: "nonexistent".into(),
        action: "do".into(),
        payload: vec![],
        content_type: IpcContentType::Json,
        target: None,
    };

    match reg.invoke(&req) {
        Err(IpcError::UnknownIpc) => assert!(true),
        other => panic!("expected UnknownIpc, got {other:?}"),
    }
}

#[test]
fn registry_execution_error() {
    let reg = IpcRegistry::new();
    reg.register(FailingIpc);

    let req = IpcRequest {
        ipc: "fail".into(),
        action: "boom".into(),
        payload: vec![],
        content_type: IpcContentType::Json,
        target: None,
    };

    match reg.invoke(&req) {
        Err(IpcError::ExecutionFailed) => {}
        other => panic!("expected ExecutionFailed, got {other:?}"),
    }
}

#[test]
fn registry_get_and_names() {
    let reg = IpcRegistry::new();
    reg.register(EchoIpc { name: "echo" });
    reg.register(FailingIpc);

    assert!(reg.get("echo").is_some());
    assert!(reg.get("fail").is_some());
    assert!(reg.get("nonexistent").is_none());

    let mut names = reg.names();
    names.sort();
    assert_eq!(names, vec!["echo".to_string(), "fail".to_string()]);
}

#[test]
fn registry_replacement() {
    let reg = IpcRegistry::new();
    let old = reg.register(EchoIpc { name: "dup" });
    assert!(old.is_none());

    let old = reg.register(EchoIpc { name: "dup" });
    assert!(old.is_some());
}

// ── Name validation ─────────────────────────────────────────────────────

#[test]
fn valid_ipc_names() {
    assert!(is_valid_ipc_name("camera"));
    assert!(is_valid_ipc_name("biometric_auth"));
    assert!(is_valid_ipc_name("file-system"));
    assert!(is_valid_ipc_name("a"));
    assert!(is_valid_ipc_name(&"x".repeat(64)));
}

#[test]
fn invalid_ipc_names() {
    assert!(!is_valid_ipc_name(""));
    assert!(!is_valid_ipc_name(&"x".repeat(65)));
    assert!(!is_valid_ipc_name("has spaces"));
    assert!(!is_valid_ipc_name("has/slash"));
    assert!(!is_valid_ipc_name("has.dot"));
}

// ── Content type roundtrip ──────────────────────────────────────────────

#[test]
fn content_type_roundtrip() {
    let json = IpcContentType::Json;
    let arrow = IpcContentType::Arrow;

    assert_eq!(json, json.clone());
    assert_ne!(json, arrow);
    assert_eq!(format!("{json:?}"), "Json");
}

// ── IpcKind::Capability tag ─────────────────────────────────────────────

#[test]
fn ipc_kind_capability_tag() {
    let cap = IpcKind::Capability;
    assert_ne!(cap, IpcKind::Query);
    assert_ne!(cap, IpcKind::Emit);
    assert_eq!(cap, IpcKind::Capability);
}

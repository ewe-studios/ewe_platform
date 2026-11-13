//! IPC registry tests moved out of `src/ipc/mod.rs` (spec 70 feature 59,
//! rule 7). Every item the tests use is public.

use foundation_platform::ipc::{IpcRegistry, PlatformIpc};
use foundation_platform::PlatformSession;
use foundation_wasm::ipc::{Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse};

struct EchoIpc;

impl Ipc for EchoIpc {
    fn name(&self) -> &'static str {
        "echo"
    }
    fn kind(&self) -> IpcKind {
        IpcKind::Query
    }
    fn invoke(&self, request: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Ok(IpcResponse {
            payload: request.payload.clone(),
            content_type: request.content_type,
        })
    }
}

impl PlatformIpc for EchoIpc {
    fn invoke_with_session(
        &self,
        _session: &PlatformSession,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Ok(IpcResponse {
            payload: request.payload.clone(),
            content_type: request.content_type,
        })
    }
}

#[test]
fn registry_register_and_invoke() {
    let reg = IpcRegistry::new();
    reg.register(EchoIpc);

    let session = PlatformSession::new_test(
        std::path::PathBuf::from("."),
        foundation_context::Ctx::fixed(std::sync::Arc::new(
            foundation_context::MonotonicClock::new(),
        )),
    );
    let req = IpcRequest {
        ipc: "echo".into(),
        action: "ping".into(),
        payload: b"hello".to_vec(),
        content_type: IpcContentType::Json,
        target: None,
    };
    let resp = reg.invoke(&session, &req).unwrap();
    assert_eq!(resp.payload, b"hello");
}

#[test]
fn registry_unknown_ipc() {
    let reg = IpcRegistry::new();
    let session = PlatformSession::new_test(
        std::path::PathBuf::from("."),
        foundation_context::Ctx::fixed(std::sync::Arc::new(
            foundation_context::MonotonicClock::new(),
        )),
    );
    let req = IpcRequest {
        ipc: "nonexistent".into(),
        action: "do".into(),
        payload: vec![],
        content_type: IpcContentType::Json,
        target: None,
    };
    match reg.invoke(&session, &req) {
        Err(IpcError::UnknownIpc(name)) => assert_eq!(name, "nonexistent"),
        other => panic!("expected UnknownIpc, got {other:?}"),
    }
}

#[test]
fn registry_get_and_names() {
    let reg = IpcRegistry::new();
    reg.register(EchoIpc);
    assert!(reg.get("echo").is_some());
    assert!(reg.get("nonexistent").is_none());
    assert_eq!(reg.names(), vec!["echo"]);
}

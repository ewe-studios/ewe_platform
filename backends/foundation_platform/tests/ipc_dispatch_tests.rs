//! IPC dispatch tests moved out of `src/backend/ipc_dispatch.rs` (spec 70
//! feature 59, rule 7).
//!
//! `SessionTransport::parse_ipc_route` was private; it is now public so its
//! parsing cases stay covered.

use foundation_platform::ipc::PlatformIpc;
use foundation_platform::{BackendTransport, PlatformSession, SessionTransport};
use foundation_wasm::ipc::{Ipc, IpcError, IpcKind, IpcRequest, IpcResponse};

#[test]
fn parse_ipc_route_simple() {
    let (name, action) = SessionTransport::parse_ipc_route("/api/echo");
    assert_eq!(name, "echo");
    assert_eq!(action, "invoke");
}

#[test]
fn parse_ipc_route_with_action_query() {
    let (name, action) = SessionTransport::parse_ipc_route("/api/echo?action=ping");
    assert_eq!(name, "echo");
    assert_eq!(action, "ping");
}

#[test]
fn parse_ipc_route_with_path_action() {
    let (name, action) = SessionTransport::parse_ipc_route("/api/system/get_info");
    assert_eq!(name, "system");
    assert_eq!(action, "get_info");
}

#[test]
fn parse_ipc_route_ipc_prefix() {
    let (name, action) = SessionTransport::parse_ipc_route("/ipc/ticker/tick");
    assert_eq!(name, "ticker");
    assert_eq!(action, "tick");
}

#[test]
fn parse_ipc_route_no_api_prefix() {
    let (name, action) = SessionTransport::parse_ipc_route("echo");
    assert_eq!(name, "echo");
    assert_eq!(action, "invoke");
}

#[test]
fn session_transport_dispatch_ipc_real_registry() {
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
            _: &PlatformSession,
            request: &IpcRequest<Vec<u8>>,
        ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
            Ok(IpcResponse {
                payload: request.payload.clone(),
                content_type: request.content_type,
            })
        }
    }

    let session = PlatformSession::new_test(
        std::path::PathBuf::from("."),
        foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
            foundation_core::valtron::MonotonicClock::new(),
        )),
    );
    session.register_ipc(EchoIpc);

    let transport = SessionTransport::new(session.clone());
    let result = transport.dispatch_ipc(None, "/api/echo?action=ping");
    assert_eq!(result, vec![] as Vec<u8>);
}

#[test]
fn http_backend_in_session() {
    let session = PlatformSession::new_test(
        std::path::PathBuf::from("."),
        foundation_core::valtron::Ctx::fixed(std::sync::Arc::new(
            foundation_core::valtron::MonotonicClock::new(),
        )),
    );
    // HttpBackend is now on the session — verify it exists.
    let _ = session.http_backend();
}

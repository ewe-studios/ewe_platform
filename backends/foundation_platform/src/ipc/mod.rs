//! IPC Registry — central backend communication mechanism (F25).
//!
//! WHY: The platform has multiple communication channels (Tauri invoke, events,
//! route responders, capabilities) with different contracts. The IPC Registry
//! unifies them behind a single trait and a single `invokeIpc()` JS API.
//!
//! WHAT: `PlatformIpc` trait (extends `foundation_wasm::ipc::Ipc`), `IpcRegistry`,
//! and the `__ewe_ipc` Tauri command bridge. The shared wire types and base
//! `Ipc` trait live in `foundation_wasm::ipc`.
//!
//! ## F43: callback-based invoke
//!
//! `invoke_with_session` takes a `Box<dyn FnOnce> + Send` callback instead of
//! returning `Result<IpcResponse, IpcError>` synchronously. This bridges:
//!  - Sync handlers (WebViewStack): call the callback immediately
//!  - Async handlers (Kotlin plugin bridge, network): defer the callback
//!  - WASM `ipc_dispatch`: registers the callback in a token registry,
//!    sends the request through `host_ipc_invoke_async`, returns immediately
//!
//! ## Design notes
//!
//! The trait takes `Box<dyn FnOnce>` internally (object-safe). The registry's
//! public `invoke` method boxes automatically — callers pass `FnOnce` directly:
//!
//! ```ignore
//! reg.invoke("echo", &req, |result| { /* fires once */ });
//! ```
//!
//! Handlers store their own `Arc<PlatformSession>` at construction — no
//! session param in the trait, no borrow across async boundaries.

use std::collections::HashMap;
use std::sync::RwLock;

use foundation_ui_traits::RouteDecision;
use foundation_wasm::ipc::{IpcError, IpcRequest, IpcResponse};

use crate::route_handler::RouteResponder;
use crate::session::PlatformSession;

pub mod streaming;

// ── IpcCallback ──────────────────────────────────────────────────────────

/// Internal callback type. Handler implementations receive this; callers
/// pass plain `FnOnce` closures through [`IpcRegistry::invoke`].
pub type IpcCallback = Box<dyn FnOnce(Result<IpcResponse<Vec<u8>>, IpcError>) + Send>;

// ── PlatformIpc trait ──────────────────────────────────────────────────

/// A platform-level IPC handler. Extends [`foundation_wasm::ipc::Ipc`].
///
/// # F43: callback-based invoke
///
/// Returns `Ok(())` on dispatch — the `callback` fires with the result.
/// Sync handlers call it immediately; async ones defer it.
pub trait PlatformIpc: foundation_wasm::ipc::Ipc {
    /// Dispatch the IPC request. `callback` fires exactly once with the result.
    fn invoke_with_session(
        &self,
        request: &IpcRequest<Vec<u8>>,
        callback: IpcCallback,
    ) -> Result<(), IpcError>;

    fn as_route_handler(&self) -> Option<&dyn RouteResponder> { None }
}

// ── IpcRegistry ────────────────────────────────────────────────────────

pub struct IpcRegistry {
    handlers: RwLock<HashMap<String, Box<dyn PlatformIpc + 'static>>>,
}

impl IpcRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self { handlers: RwLock::new(HashMap::new()) }
    }

    pub fn register<I: PlatformIpc + 'static>(&self, ipc: I) -> Option<Box<dyn PlatformIpc>> {
        self.handlers.write().unwrap().insert(ipc.name().to_string(), Box::new(ipc))
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn PlatformIpc> {
        let guard = self.handlers.read().unwrap();
        guard.get(name).map(|b| {
            unsafe { &*std::ptr::from_ref::<dyn PlatformIpc>(b.as_ref()) }
        })
    }

    /// Invoke an IPC. Boxes the callback automatically.
    ///
    /// ```ignore
    /// reg.invoke(&req, |result| { /* Ok(response) or Err */ });
    /// ```
    pub fn invoke<F>(
        &self,
        request: &IpcRequest<Vec<u8>>,
        callback: F,
    ) -> Result<(), IpcError>
    where
        F: FnOnce(Result<IpcResponse<Vec<u8>>, IpcError>) + Send + 'static,
    {
        match self.get(&request.ipc) {
            Some(ipc) => ipc.invoke_with_session(request, Box::new(callback)),
            None => Err(IpcError::UnknownIpc(request.ipc.clone())),
        }
    }

    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.handlers.read().unwrap().keys().cloned().collect()
    }
}

impl Default for IpcRegistry {
    fn default() -> Self { Self::new() }
}

// ── Convenience ────────────────────────────────────────────────────────

#[must_use]
pub fn ipc_route() -> RouteDecision {
    crate::route::ipc_shell()
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use foundation_wasm::ipc::{IpcContentType, IpcKind};

    struct EchoIpc;

    impl foundation_wasm::ipc::Ipc for EchoIpc {
        fn name(&self) -> &str { "echo" }
        fn kind(&self) -> IpcKind { IpcKind::Query }
        fn invoke(&self, request: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
            Ok(IpcResponse { payload: request.payload.clone(), content_type: request.content_type })
        }
    }

    impl PlatformIpc for EchoIpc {
        fn invoke_with_session(
            &self,
            request: &IpcRequest<Vec<u8>>,
            callback: IpcCallback,
        ) -> Result<(), IpcError> {
            callback(Ok(IpcResponse {
                payload: request.payload.clone(),
                content_type: request.content_type,
            }));
            Ok(())
        }
    }

    #[test]
    fn registry_register_and_invoke() {
        let reg = IpcRegistry::new();
        reg.register(EchoIpc);

        let req = IpcRequest {
            ipc: "echo".into(), action: "ping".into(),
            payload: b"hello".to_vec(), content_type: IpcContentType::Json, target: None,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        reg.invoke(&req, move |result| { let _ = tx.send(result); }).unwrap();
        let resp = rx.recv().unwrap().unwrap();
        assert_eq!(resp.payload, b"hello");
    }

    #[test]
    fn registry_unknown_ipc() {
        let reg = IpcRegistry::new();
        let req = IpcRequest {
            ipc: "nonexistent".into(), action: "do".into(),
            payload: vec![], content_type: IpcContentType::Json, target: None,
        };
        match reg.invoke(&req, |_| {}) {
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
}

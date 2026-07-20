//! IPC Registry — central backend communication mechanism (F25).
//!
//! WHY: The platform has multiple communication channels (Tauri invoke, events,
//! route responders, capabilities) with different contracts. The IPC Registry
//! unifies them behind a single `Ipc` trait and a single `invokeIpc()` JS API.
//!
//! WHAT: `Ipc` trait, `IpcKind`, `IpcRegistry`, and the `__ewe_ipc` Tauri
//! command bridge. The shared wire types live in `foundation_wasm::ipc`.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use foundation_wasm::ipc::{IpcRequest, IpcResponse, IpcError};
use foundation_ui_traits::RouteDecision;

use crate::route_handler::RouteResponder;
use crate::session::PlatformSession;

// ── IpcKind ─────────────────────────────────────────────────────────────

/// The transport kind for an IPC handler.
///
/// Streaming is NOT a variant — streaming needs a different contract
/// (bidirectional chunks, backpressure). See `StreamingIpc` (F26).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcKind {
    /// Request → Response. Single reply (like Tauri invoke).
    Query,
    /// Fire-and-forget. No response expected (like Tauri events).
    Emit,
    /// Request → Response, rendered as a page. Can also serve as a
    /// `RouteResponder` when registered as a route handler.
    Page,
}

// ── Ipc trait ───────────────────────────────────────────────────────────

/// An IPC handler registered in the platform's IPC registry.
///
/// IPCs are the primary mechanism for frontend ↔ backend communication.
/// They unify Tauri commands, events, and route responders behind a single
/// `invoke()` contract.
pub trait Ipc: Send + Sync + 'static {
    /// Unique name. Used as the lookup key and the JS invocation target.
    fn name(&self) -> &str;

    /// The IPC kind determines default transport behavior.
    fn kind(&self) -> IpcKind;

    /// Execute the IPC and return a response.
    fn invoke(
        &self,
        session: &PlatformSession,
        request: &IpcRequest,
    ) -> Result<IpcResponse, IpcError>;

    /// Optional: serve this IPC as an HTTP route handler.
    /// When `Some`, the IPC can be registered as a route via
    /// `session.register_route_with("/api/system/*", ipc_shell(), &handler)`.
    fn as_route_handler(&self) -> Option<&dyn RouteResponder> { None }
}

// ── IpcRegistry ────────────────────────────────────────────────────────

/// Registry of all IPC handlers. Thread-safe via `RwLock`.
///
/// Registered on `PlatformSession` at startup. IPCs are invoked from JS
/// (via the `__ewe_ipc` Tauri command) or programmatically from route
/// handlers (via `session.get_ipc(name)?.invoke(...)`).
pub struct IpcRegistry {
    handlers: RwLock<HashMap<String, Box<dyn Ipc>>>,
}

impl IpcRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register an IPC handler. Returns the previous handler under the
    /// same name, if any.
    pub fn register<I: Ipc>(&self, ipc: I) -> Option<Box<dyn Ipc>> {
        self.handlers
            .write()
            .unwrap()
            .insert(ipc.name().to_string(), Box::new(ipc))
    }

    /// Look up an IPC by name.
    ///
    /// The returned reference must not outlive the registry — the caller
    /// must be in a scope where no concurrent writes can occur.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn Ipc> {
        let guard = self.handlers.read().unwrap();
        guard.get(name).map(|b| {
            // SAFETY: the registry owns the Box<dyn Ipc>. The reference
            // lives as long as the registry — we leak it past the guard
            // because registrations only happen at startup.
            unsafe { &*(b.as_ref() as *const dyn Ipc) }
        })
    }

    /// Invoke an IPC by name through the registry.
    pub fn invoke(
        &self,
        session: &PlatformSession,
        request: &IpcRequest,
    ) -> Result<IpcResponse, IpcError> {
        match self.get(&request.ipc) {
            Some(ipc) => ipc.invoke(session, request),
            None => Err(IpcError::UnknownIpc(request.ipc.clone())),
        }
    }

    /// Return all registered IPC names.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.handlers
            .read()
            .unwrap()
            .keys()
            .cloned()
            .collect()
    }
}

impl Default for IpcRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Convenience constructors ────────────────────────────────────────────

/// Create an IPC route decision. Uses `IpcShell` source with `TrustedRemote`
/// profile. Use this when registering an IPC that also serves as a route
/// handler (IpcKind::Page).
#[must_use]
pub fn ipc_route() -> RouteDecision {
    crate::route::ipc_shell()
}

// (Tauri command __ewe_ipc is defined in builder.rs alongside __ewe_capabilities.)

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoIpc;

    impl Ipc for EchoIpc {
        fn name(&self) -> &str { "echo" }
        fn kind(&self) -> IpcKind { IpcKind::Query }
        fn invoke(
            &self,
            _session: &PlatformSession,
            request: &IpcRequest,
        ) -> Result<IpcResponse, IpcError> {
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

        let req = IpcRequest {
            ipc: "echo".into(),
            action: "ping".into(),
            payload: b"hello".to_vec(),
            content_type: foundation_wasm::ipc::IpcContentType::Json,
            target: None,
        };

        // invoke requires a session — use new_test
        let session = PlatformSession::new_test(std::path::PathBuf::from("."));
        let resp = reg.invoke(&session, &req).unwrap();
        assert_eq!(resp.payload, b"hello");
    }

    #[test]
    fn registry_unknown_ipc() {
        let reg = IpcRegistry::new();
        let session = PlatformSession::new_test(std::path::PathBuf::from("."));
        let req = IpcRequest {
            ipc: "nonexistent".into(),
            action: "do".into(),
            payload: vec![],
            content_type: foundation_wasm::ipc::IpcContentType::Json,
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
}

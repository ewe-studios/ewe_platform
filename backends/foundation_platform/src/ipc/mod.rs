//! IPC Registry — central backend communication mechanism (F25).
//!
//! WHY: The platform has multiple communication channels (Tauri invoke, events,
//! route responders, capabilities) with different contracts. The IPC Registry
//! unifies them behind a single trait and a single `invokeIpc()` JS API.
//!
//! WHAT: `PlatformIpc` trait (extends `foundation_wasm::ipc::Ipc`), `IpcRegistry`,
//! and the `__ewe_ipc` Tauri command bridge. The shared wire types and base
//! `Ipc` trait live in `foundation_wasm::ipc`.

use std::collections::HashMap;
use std::sync::RwLock;

use foundation_ui_traits::RouteDecision;
use foundation_wasm::ipc::{IpcError, IpcRequest, IpcResponse};

use crate::route_handler::RouteResponder;
use crate::session::PlatformSession;

pub mod streaming;

// ── PlatformIpc trait ──────────────────────────────────────────────────

/// A platform-level IPC handler. Extends [`foundation_wasm::ipc::Ipc`] with
/// platform-specific methods: session-aware invocation and optional route
/// handler support.
///
/// The base trait (`name()`, `kind()`, `invoke()`) lives in `foundation_wasm`
/// so WASM apps can depend on the contract without pulling in the platform.
pub trait PlatformIpc: foundation_wasm::ipc::Ipc {
    /// Execute the IPC with platform session context.
    ///
    /// # Errors
    ///
    /// Returns [`IpcError`] if the handler encounters a domain or platform-level
    /// failure.
    fn invoke_with_session(
        &self,
        session: &PlatformSession,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError>;

    /// Optional: serve this IPC as an HTTP route handler.
    fn as_route_handler(&self) -> Option<&dyn RouteResponder> {
        None
    }
}

// ── IpcRegistry ────────────────────────────────────────────────────────

/// Registry of all IPC handlers. Thread-safe via `RwLock`.
pub struct IpcRegistry {
    handlers: RwLock<HashMap<String, Box<dyn PlatformIpc + 'static>>>,
}

impl IpcRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a platform IPC handler.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn register<I: PlatformIpc + 'static>(&self, ipc: I) -> Option<Box<dyn PlatformIpc>> {
        self.handlers
            .write()
            .unwrap()
            .insert(ipc.name().to_string(), Box::new(ipc))
    }

    /// Look up an IPC by name. Returns the platform trait object.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn PlatformIpc> {
        let guard = self.handlers.read().unwrap();
        guard
            .get(name)
            .map(|b| unsafe { &*std::ptr::from_ref::<dyn PlatformIpc>(b.as_ref()) })
    }

    /// Invoke an IPC through the registry with session context.
    ///
    /// # Errors
    ///
    /// Returns [`IpcError::UnknownIpc`] if no handler is registered under
    /// the given name, or the handler's error if invocation fails.
    pub fn invoke(
        &self,
        session: &PlatformSession,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        match self.get(&request.ipc) {
            Some(ipc) => ipc.invoke_with_session(session, request),
            None => Err(IpcError::UnknownIpc(request.ipc.clone())),
        }
    }

    /// Return all registered IPC names.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.handlers.read().unwrap().keys().cloned().collect()
    }
}

impl Default for IpcRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Convenience ────────────────────────────────────────────────────────

/// Create an IPC route decision (`IpcShell`, `TrustedRemote` profile).
#[must_use]
pub fn ipc_route() -> RouteDecision {
    crate::route::ipc_shell()
}

// ── Tests ───────────────────────────────────────────────────────────────

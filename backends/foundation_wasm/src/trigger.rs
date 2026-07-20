//! Host→WASM trigger registry (F27).
//!
//! WHY: F23 (capabilities) and F25 (IPC) implement the wasm→host direction.
//! The reverse — host delivering capability/IPC requests INTO the WASM module
//! — needs a registered handler on the WASM side.
//!
//! WHAT: `TriggerRegistry` with `set_capability_handler()` and
//! `set_ipc_handler()`. Single-handler model — the WASM app registers one
//! handler for each trigger type. The JS runtime calls the registry's
//! `dispatch_*()` methods when the host delivers a trigger.
//!
//! HOW: Plain struct, no interior mutability. The owner wraps it however
//! they need (`static Mutex<TriggerRegistry>`, `Arc<RwLock<...>>`, or
//! just stack-owned on wasm32). `set_*` take `&mut self`, `dispatch_*`
//! take `&self`.

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::capability::{CapabilityError, CapabilityRequest, CapabilityResponse};
use crate::ipc::{IpcError, IpcRequest, IpcResponse};

// ── TriggerRegistry ─────────────────────────────────────────────────────

/// Host→WASM trigger registry.
///
/// No interior mutability — the owner decides how to share access
/// (`static Mutex<TriggerRegistry>`, `Arc<RwLock<...>>`, or plain
/// stack ownership on wasm32).
pub struct TriggerRegistry {
    #[cfg(not(target_family = "wasm"))]
    capability: Option<Box<dyn Fn(CapabilityRequest<Vec<u8>>) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError> + Send + Sync + 'static>>,
    #[cfg(target_family = "wasm")]
    capability: Option<Box<dyn Fn(CapabilityRequest<Vec<u8>>) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError> + 'static>>,

    #[cfg(not(target_family = "wasm"))]
    ipc: Option<Box<dyn Fn(IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> + Send + Sync + 'static>>,
    #[cfg(target_family = "wasm")]
    ipc: Option<Box<dyn Fn(IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> + 'static>>,
}

impl TriggerRegistry {
    /// Create an empty registry. No handlers registered initially.
    #[must_use]
    pub const fn new() -> Self {
        Self { capability: None, ipc: None }
    }

    /// Register the capability trigger handler. Replaces any previous handler.
    #[cfg(not(target_family = "wasm"))]
    pub fn set_capability_handler(
        &mut self,
        handler: impl Fn(CapabilityRequest<Vec<u8>>) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError>
            + Send + Sync + 'static,
    ) {
        self.capability = Some(Box::new(handler));
    }

    #[cfg(target_family = "wasm")]
    pub fn set_capability_handler(
        &mut self,
        handler: impl Fn(CapabilityRequest<Vec<u8>>) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError>
            + 'static,
    ) {
        self.capability = Some(Box::new(handler));
    }

    /// Register the IPC trigger handler. Replaces any previous handler.
    #[cfg(not(target_family = "wasm"))]
    pub fn set_ipc_handler(
        &mut self,
        handler: impl Fn(IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError>
            + Send + Sync + 'static,
    ) {
        self.ipc = Some(Box::new(handler));
    }

    #[cfg(target_family = "wasm")]
    pub fn set_ipc_handler(
        &mut self,
        handler: impl Fn(IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError>
            + 'static,
    ) {
        self.ipc = Some(Box::new(handler));
    }

    /// Dispatch a capability trigger. Returns an error if no handler is registered.
    pub fn dispatch_capability(
        &self,
        request: CapabilityRequest<Vec<u8>>,
    ) -> Result<CapabilityResponse<Vec<u8>>, CapabilityError> {
        match self.capability.as_ref() {
            Some(handler) => handler(request),
            None => Err(CapabilityError::ExecutionFailed(
                "no capability trigger handler registered".into(),
            )),
        }
    }

    /// Dispatch an IPC trigger. Returns an error if no handler is registered.
    pub fn dispatch_ipc(
        &self,
        request: IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        match self.ipc.as_ref() {
            Some(handler) => handler(request),
            None => Err(IpcError::ExecutionFailed(
                "no IPC trigger handler registered".into(),
            )),
        }
    }
}

impl Default for TriggerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

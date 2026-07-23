//! Host→WASM trigger registry (F27, updated F41).
//!
//! WHY: F23 (capabilities) and F25 (IPC) implement the wasm→host direction.
//! The reverse — host delivering IPC requests INTO the WASM module — needs a
//! registered handler on the WASM side.
//!
//! WHAT: `TriggerRegistry` with `set_ipc_handler()`. Single-handler model —
//! the WASM app registers one handler. The JS runtime calls `dispatch_ipc()`
//! when the host delivers a trigger.
//!
//! HOW: Plain struct, no interior mutability. The owner wraps it however
//! they need (`static Mutex<TriggerRegistry>`, `Arc<RwLock<...>>`, or
//! just stack-owned on wasm32). `set_*` take `&mut self`, `dispatch_*`
//! take `&self`.

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::ipc::{IpcError, IpcRequest, IpcResponse};

// ── Internal handler types ─────────────────────────────────────────────

#[cfg(not(target_family = "wasm"))]
type IpcHandlerBox = Box<dyn Fn(IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> + Send + Sync + 'static>;
#[cfg(target_family = "wasm")]
type IpcHandlerBox = Box<dyn Fn(IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> + 'static>;

// ── TriggerRegistry ─────────────────────────────────────────────────────

/// Host→WASM trigger registry.
///
/// No interior mutability — the owner decides how to share access
/// (`static Mutex<TriggerRegistry>`, `Arc<RwLock<...>>`, or plain
/// stack ownership on wasm32).
pub struct TriggerRegistry {
    ipc: Option<IpcHandlerBox>,
}

impl TriggerRegistry {
    /// Create an empty registry. No handlers registered initially.
    #[must_use]
    pub const fn new() -> Self {
        Self { ipc: None }
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

    /// Dispatch an IPC trigger. Returns an error if no handler is registered.
    /// # Errors
    ///
    /// Returns [`IpcError::ExecutionFailed`] if no handler is registered.
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

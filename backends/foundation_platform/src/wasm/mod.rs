//! Typed WASM wrappers for platform IPC handlers (F41 Part D).
//!
//! Each module exposes a zero-sized struct with static methods that call
//! `dispatch_json()` — a thin serde wrapper around `ipc_dispatch()`.
//! WASM apps get typed APIs instead of raw FFI calls.
//!
//! The native side (`foundation_platform/src/ipc/`) implements the handlers
//! behind `Ipc` / `AndroidIpc` / `IosIpc`. On desktop/web where no native
//! handler exists, the host returns `ExecutionFailed("unsupported")`.

use foundation_wasm::ipc::{IpcContentType, IpcError, IpcRequest, IpcResponse};
use foundation_wasm::ipc_ffi::ipc_dispatch;

/// Serialize args to JSON → dispatch → deserialize result from JSON.
#[cfg(target_family = "wasm")]
pub fn dispatch_json<A: serde::Serialize, R: serde::de::DeserializeOwned>(
    name: &str,
    action: &str,
    args: A,
) -> Result<R, IpcError> {
    let payload = serde_json::to_vec(&args)
        .map_err(|e| IpcError::InvalidPayload(format!("serialize: {e}")))?;
    let wire_req = IpcRequest {
        ipc: name.into(), action: action.into(),
        payload, content_type: IpcContentType::Json, target: None,
    };
    let resp: IpcResponse<Vec<u8>> = ipc_dispatch(name, wire_req)?;
    let result = serde_json::from_slice(&resp.payload)
        .map_err(|e| IpcError::InvalidPayload(format!("deserialize: {e}")))?;
    Ok(result)
}

#[cfg(target_family = "wasm")]
pub mod camera;
#[cfg(target_family = "wasm")]
pub mod filesystem;
#[cfg(target_family = "wasm")]
pub mod biometric;
#[cfg(target_family = "wasm")]
pub mod chrome;

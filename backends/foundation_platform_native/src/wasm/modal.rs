//! Modal typed WASM wrapper — `Modal::present()` / `Modal::dismiss()`.
//!
//! Compiles only on wasm32. Passes concrete types through the IPC FFI —
//! serialization lives in `shared/modal_types.rs` (WirePayload impls).

use crate::shared::modal_types::{DismissArgs, PresentArgs, PresentResult};
use foundation_wasm::ipc::{IpcContentType, IpcError, IpcRequest, IpcResponse};
use foundation_wasm::ipc_ffi::ipc_dispatch;

pub struct Modal;

impl Modal {
    pub fn present(args: PresentArgs) -> Result<PresentResult, IpcError> {
        let IpcResponse { payload, .. } = ipc_dispatch("chrome", IpcRequest {
            ipc: "chrome".into(),
            action: "present_modal".into(),
            payload: args,
            content_type: IpcContentType::Json,
            target: None,
        })?;
        Ok(payload)
    }

    pub fn dismiss(args: DismissArgs) -> Result<(), IpcError> {
        ipc_dispatch::<DismissArgs, Vec<u8>>("chrome", IpcRequest {
            ipc: "chrome".into(),
            action: "dismiss_modal".into(),
            payload: args,
            content_type: IpcContentType::Json,
            target: None,
        })?;
        Ok(())
    }

    pub fn dismiss_all() -> Result<(), IpcError> {
        ipc_dispatch::<Vec<u8>, Vec<u8>>("chrome", IpcRequest {
            ipc: "chrome".into(),
            action: "dismiss_all_modals".into(),
            payload: vec![],
            content_type: IpcContentType::Json,
            target: None,
        })?;
        Ok(())
    }
}

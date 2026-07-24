//! Modal typed WASM wrapper — `Modal::present()` / `Modal::dismiss()`.
//!
//! Compiles on wasm32. Uses `ipc_invoke_typed` (callback-based).
//! WASM does not care whether the host resolves sync or async.

use crate::shared::modal_types::{DismissArgs, PresentArgs, PresentResult};
use foundation_wasm::ipc::{IpcContentType, IpcError, IpcRequest};
use foundation_wasm::ipc_ffi::ipc_invoke_typed;

pub struct Modal;

impl Modal {
    pub fn present(
        args: PresentArgs,
        callback: impl Fn(Result<PresentResult, IpcError>) + Send + Sync + 'static,
    ) -> Result<(), IpcError> {
        ipc_invoke_typed(
            "chrome",
            IpcRequest {
                ipc: "chrome".into(),
                action: "present_modal".into(),
                payload: args,
                content_type: IpcContentType::Json,
                target: None,
            },
            callback,
        )
    }

    pub fn dismiss(
        args: DismissArgs,
        callback: impl Fn(Result<(), IpcError>) + Send + Sync + 'static,
    ) -> Result<(), IpcError> {
        ipc_invoke_typed::<DismissArgs, alloc::vec::Vec<u8>>(
            "chrome",
            IpcRequest {
                ipc: "chrome".into(),
                action: "dismiss_modal".into(),
                payload: args,
                content_type: IpcContentType::Json,
                target: None,
            },
            move |r| callback(r.map(|_| ())),
        )
    }

    pub fn dismiss_all(
        callback: impl Fn(Result<(), IpcError>) + Send + Sync + 'static,
    ) -> Result<(), IpcError> {
        ipc_invoke_typed::<alloc::vec::Vec<u8>, alloc::vec::Vec<u8>>(
            "chrome",
            IpcRequest {
                ipc: "chrome".into(),
                action: "dismiss_all_modals".into(),
                payload: alloc::vec![],
                content_type: IpcContentType::Json,
                target: None,
            },
            move |r| callback(r.map(|_| ())),
        )
    }
}

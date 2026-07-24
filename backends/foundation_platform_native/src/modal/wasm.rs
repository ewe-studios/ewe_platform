//! Typed WASM wrapper for the modal IPC (F42).
//!
//! WASM apps import this module and call `Modal::present()` / `Modal::dismiss()`.

use foundation_wasm::ipc::{IpcContentType, IpcError, IpcRequest};
use foundation_wasm::ipc_ffi::ipc_dispatch;

/// A zero-sized typed wrapper for modal operations.
pub struct Modal;

/// Arguments for presenting a modal.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct PresentModalArgs {
    pub route: String,
    pub style: Option<String>, // "bottom_sheet" | "dialog" | "fullscreen"
    pub title: Option<String>,
}

/// Result of opening a modal.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct PresentModalResult {
    pub modal_id: String,
    pub webview_label: String,
}

/// Arguments for dismissing a modal.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DismissModalArgs {
    pub modal_id: String,
}

impl Modal {
    /// Open a native modal dialog with a WebView loading the given route.
    pub fn present(args: PresentModalArgs) -> Result<PresentModalResult, IpcError> {
        let payload = serde_json::to_vec(&args)
            .map_err(|e| IpcError::InvalidPayload(format!("serialize: {e}")))?;
        ipc_dispatch(
            "chrome",
            IpcRequest {
                ipc: "chrome".into(),
                action: "present_modal".into(),
                payload,
                content_type: IpcContentType::Json,
                target: None,
            },
        )
    }

    /// Dismiss a modal by ID.
    pub fn dismiss(args: DismissModalArgs) -> Result<(), IpcError> {
        let payload = serde_json::to_vec(&args)
            .map_err(|e| IpcError::InvalidPayload(format!("serialize: {e}")))?;
        let _: serde_json::Value = ipc_dispatch(
            "chrome",
            IpcRequest {
                ipc: "chrome".into(),
                action: "dismiss_modal".into(),
                payload,
                content_type: IpcContentType::Json,
                target: None,
            },
        )?;
        Ok(())
    }
}

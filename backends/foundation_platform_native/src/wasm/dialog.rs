//! Dialog typed WASM wrapper — `Dialog::show()`.
//!
//! Compiles only on wasm32.

use crate::shared::dialog_types::{ShowArgs, ShowResult};
use foundation_wasm::ipc::{IpcContentType, IpcError, IpcRequest, IpcResponse};
use foundation_wasm::ipc_ffi::ipc_dispatch;

pub struct Dialog;

impl Dialog {
    pub fn show(args: ShowArgs) -> Result<ShowResult, IpcError> {
        let IpcResponse { payload, .. } = ipc_dispatch("dialog", IpcRequest {
            ipc: "dialog".into(),
            action: "show".into(),
            payload: args,
            content_type: IpcContentType::Json,
            target: None,
        })?;
        Ok(payload)
    }
}

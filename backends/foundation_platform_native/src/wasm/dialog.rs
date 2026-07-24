//! Dialog typed WASM wrapper — `Dialog::show()`.
//!
//! Compiles on wasm32. Uses `ipc_invoke_typed` (callback-based).

extern crate alloc;

use crate::shared::dialog_types::{ShowArgs, ShowResult};
use foundation_wasm::ipc::{IpcContentType, IpcError, IpcRequest};
use foundation_wasm::ipc_ffi::ipc_invoke_typed;

pub struct Dialog;

impl Dialog {
    pub fn show(
        args: ShowArgs,
        callback: impl Fn(Result<ShowResult, IpcError>) + Send + Sync + 'static,
    ) -> Result<(), IpcError> {
        ipc_invoke_typed(
            "dialog",
            IpcRequest {
                ipc: "dialog".into(),
                action: "show".into(),
                payload: args,
                content_type: IpcContentType::Json,
                target: None,
            },
            callback,
        )
    }
}

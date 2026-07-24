//! Dialog IPC handler — native side (F42 dialog capability).
//!
//! F43: callback-based invoke — returns `Ok(())` on dispatch, fires callback.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::shared::dialog_types::{ShowArgs, ShowResult};
use foundation_platform::ipc::{IpcCallback, PlatformIpc};
use foundation_platform::PlatformSession;
use foundation_wasm::ipc::{Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse};

pub fn register(session: &Arc<PlatformSession>) {
    session.register_ipc(DialogIpc { session: Arc::clone(session) });
}

struct DialogIpc {
    session: Arc<PlatformSession>,
}

impl Ipc<Vec<u8>, Vec<u8>> for DialogIpc {
    fn name(&self) -> &str { "dialog" }
    fn kind(&self) -> IpcKind { IpcKind::Capability }
    fn invoke(&self, _req: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Err(IpcError::ExecutionFailed("use invoke_with_session".into()))
    }
}

impl PlatformIpc for DialogIpc {
    fn invoke_with_session(
        &self,
        req: &IpcRequest<Vec<u8>>,
        callback: IpcCallback,
    ) -> Result<(), IpcError> {
        let result = match req.action.as_str() {
            "show" => Self::show(req),
            "dismiss" => Self::dismiss(req),
            _ => Err(IpcError::ExecutionFailed(format!("dialog: unknown '{}'", req.action))),
        };
        callback(result);
        Ok(())
    }
}

impl DialogIpc {
    fn show(req: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let _args: ShowArgs = serde_json::from_slice(&req.payload)
            .map_err(|e| IpcError::InvalidPayload(format!("json: {e}")))?;

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let resp = ShowResult {
            dialog_id: format!("dialog_{}", COUNTER.fetch_add(1, Ordering::Relaxed)),
        };
        Ok(IpcResponse {
            payload: serde_json::to_vec(&resp).unwrap(),
            content_type: IpcContentType::Json,
        })
    }

    fn dismiss(req: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let _args: serde_json::Value = serde_json::from_slice(&req.payload)
            .map_err(|e| IpcError::InvalidPayload(format!("json: {e}")))?;
        Ok(IpcResponse {
            payload: br#"{"ok":true}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
    }
}

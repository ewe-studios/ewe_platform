//! Dialog IPC handler (F42 dialog capability).
//!
//! Feature-gated behind `#[cfg(feature = "dialog")]`.
//! Registers a "dialog" IPC handler for native AlertDialog/DialogFragment.

use std::sync::atomic::{AtomicU64, Ordering};
use foundation_platform::PlatformSession;
use foundation_wasm::ipc::{Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse};

/// Register the dialog IPC handler on the session.
pub fn register(session: &PlatformSession) {
    session.register_ipc(DialogIpc);
}

struct DialogIpc;

impl Ipc<Vec<u8>, Vec<u8>> for DialogIpc {
    fn name(&self) -> &str { "dialog" }
    fn kind(&self) -> IpcKind { IpcKind::Capability }
    fn invoke(&self, _req: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Err(IpcError::ExecutionFailed("use invoke_with_session".into()))
    }
}

impl foundation_platform::ipc::PlatformIpc for DialogIpc {
    fn invoke_with_session(
        &self,
        session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        match req.action.as_str() {
            "show" => Self::show(session, req),
            "dismiss" => Self::dismiss(req),
            _ => Err(IpcError::ExecutionFailed(format!("dialog: unknown '{}'", req.action))),
        }
    }
}

impl DialogIpc {
    fn show(
        _session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let args: serde_json::Value = serde_json::from_slice(&req.payload)
            .map_err(|e| IpcError::InvalidPayload(format!("json: {e}")))?;

        let _title = args["title"].as_str().unwrap_or("");
        let _message = args["message"].as_str().unwrap_or("");
        let _positive = args["positive_button"].as_str().unwrap_or("OK");
        let _negative = args["negative_button"].as_str().unwrap_or("");
        let _route = args["route"].as_str(); // optional WebView URL

        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dialog_id = format!("dialog_{}", COUNTER.fetch_add(1, Ordering::Relaxed));

        let resp = serde_json::json!({
            "dialog_id": dialog_id,
        });
        Ok(IpcResponse {
            payload: serde_json::to_vec(&resp).unwrap(),
            content_type: IpcContentType::Json,
        })
    }

    fn dismiss(
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let args: serde_json::Value = serde_json::from_slice(&req.payload)
            .map_err(|e| IpcError::InvalidPayload(format!("json: {e}")))?;
        let _dialog_id = args["dialog_id"].as_str().unwrap_or("");
        Ok(IpcResponse {
            payload: br#"{"ok":true}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
    }
}

// ── WASM wrapper (only on wasm32) ────────────────────────────────────────

#[cfg(target_family = "wasm")]
pub mod wasm {
    use foundation_wasm::ipc::{IpcContentType, IpcError, IpcRequest};
    use foundation_wasm::ipc_ffi::ipc_dispatch;

    pub struct Dialog;

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct ShowArgs {
        pub title: String,
        pub message: Option<String>,
        pub positive_button: Option<String>,
        pub negative_button: Option<String>,
        pub route: Option<String>,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct ShowResult { pub dialog_id: String }

    impl Dialog {
        pub fn show(args: ShowArgs) -> Result<ShowResult, IpcError> {
            let payload = serde_json::to_vec(&args)
                .map_err(|e| IpcError::InvalidPayload(format!("serialize: {e}")))?;
            ipc_dispatch("dialog", IpcRequest {
                ipc: "dialog".into(), action: "show".into(),
                payload, content_type: IpcContentType::Json, target: None,
            })
        }
    }
}

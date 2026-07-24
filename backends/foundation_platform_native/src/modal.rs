//! Modal IPC handler (F42 modal capability).
//!
//! Feature-gated behind `#[cfg(feature = "modal")]`.
//! Registers a "chrome" IPC handler that wraps WebViewStack + WindowManager
//! operations behind `present_modal` / `dismiss_modal`.

use foundation_platform::PlatformSession;
use foundation_wasm::ipc::{Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse};

/// Register the modal IPC handler on the session.
pub fn register(session: &PlatformSession) {
    session.register_ipc(ModalIpc);
}

struct ModalIpc;

impl Ipc<Vec<u8>, Vec<u8>> for ModalIpc {
    fn name(&self) -> &str { "chrome" }
    fn kind(&self) -> IpcKind { IpcKind::Capability }
    fn invoke(&self, _req: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Err(IpcError::ExecutionFailed("use invoke_with_session".into()))
    }
}

impl foundation_platform::ipc::PlatformIpc for ModalIpc {
    fn invoke_with_session(
        &self,
        session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        match req.action.as_str() {
            "present_modal" => Self::present(session, req),
            "dismiss_modal" => Self::dismiss(session, req),
            _ => Err(IpcError::ExecutionFailed(format!("chrome: unknown '{}'", req.action))),
        }
    }
}

impl ModalIpc {
    fn present(
        _session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let args: serde_json::Value = serde_json::from_slice(&req.payload)
            .map_err(|e| IpcError::InvalidPayload(format!("json: {e}")))?;
        let route = args["route"].as_str().unwrap_or("/");
        let style = args["style"].as_str().unwrap_or("dialog");
        let _title = args["title"].as_str().unwrap_or("");

        // TODO: create Tauri WebView window via WindowManager, push modal slot.
        // For now: stub that returns metadata.
        let modal_id = format!("modal_{route}", route = route.replace('/', "_"));
        let _ = style;

        let resp = serde_json::json!({
            "modal_id": modal_id,
            "webview_label": modal_id,
        });
        Ok(IpcResponse {
            payload: serde_json::to_vec(&resp).unwrap(),
            content_type: IpcContentType::Json,
        })
    }

    fn dismiss(
        _session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let args: serde_json::Value = serde_json::from_slice(&req.payload)
            .map_err(|e| IpcError::InvalidPayload(format!("json: {e}")))?;
        let _modal_id = args["modal_id"].as_str().unwrap_or("");
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

    pub struct Modal;

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct PresentArgs {
        pub route: String,
        pub style: Option<String>,
        pub title: Option<String>,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct PresentResult {
        pub modal_id: String,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct DismissArgs { pub modal_id: String }

    impl Modal {
        pub fn present(args: PresentArgs) -> Result<PresentResult, IpcError> {
            let payload = serde_json::to_vec(&args)
                .map_err(|e| IpcError::InvalidPayload(format!("serialize: {e}")))?;
            ipc_dispatch("chrome", IpcRequest {
                ipc: "chrome".into(), action: "present_modal".into(),
                payload, content_type: IpcContentType::Json, target: None,
            })
        }

        pub fn dismiss(args: DismissArgs) -> Result<(), IpcError> {
            let payload = serde_json::to_vec(&args)
                .map_err(|e| IpcError::InvalidPayload(format!("serialize: {e}")))?;
            let _: serde_json::Value = ipc_dispatch("chrome", IpcRequest {
                ipc: "chrome".into(), action: "dismiss_modal".into(),
                payload, content_type: IpcContentType::Json, target: None,
            })?;
            Ok(())
        }
    }
}

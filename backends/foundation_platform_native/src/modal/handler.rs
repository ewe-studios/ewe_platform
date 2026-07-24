//! Modal IPC handler (F42 modal capability).
//!
//! Registered on the PlatformSession as an IPC named "chrome".
//! Handles `present_modal`, `dismiss_modal`, `dismiss_all_modals`.

use foundation_platform::PlatformSession;
use foundation_wasm::ipc::{
    Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse,
};

/// The modal IPC handler.
pub struct ModalIpc;

impl Ipc<Vec<u8>, Vec<u8>> for ModalIpc {
    fn name(&self) -> &str {
        "chrome"
    }
    fn kind(&self) -> IpcKind {
        IpcKind::Capability
    }
    fn invoke(
        &self,
        _req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        // Delegate to session-aware invoke
        Err(IpcError::ExecutionFailed(
            "use invoke_with_session".into(),
        ))
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
            "dismiss_all_modals" => Self::dismiss_all(session),
            _ => Err(IpcError::ExecutionFailed(format!(
                "chrome: unknown action '{}'",
                req.action
            ))),
        }
    }
}

impl ModalIpc {
    fn present(
        _session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        // Parse args from req.payload (JSON: { route, style, title })
        let args: serde_json::Value = serde_json::from_slice(&req.payload)
            .map_err(|e| IpcError::InvalidPayload(format!("json: {e}")))?;

        let route = args["route"].as_str().unwrap_or("/");
        let style = args["style"].as_str().unwrap_or("dialog");
        let _title = args["title"].as_str().unwrap_or("");

        // TODO: create WebView window via WindowManager, push modal slot,
        // return modal_id. For now: stub that logs.
        let modal_id = "modal_1".to_string();
        let _ = route;
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

        // TODO: find modal by ID, close WebView, pop slot
        Ok(IpcResponse {
            payload: br#"{"ok":true}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
    }

    fn dismiss_all(_session: &PlatformSession) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        // TODO: iterate all modals, close each
        Ok(IpcResponse {
            payload: br#"{"ok":true}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
    }
}

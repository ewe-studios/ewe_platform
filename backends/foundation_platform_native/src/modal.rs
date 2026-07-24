//! Modal IPC handler (F42 modal capability).
//!
//! Registers a "chrome" IPC handler that wraps WebViewStack + WindowManager
//! operations behind `present_modal` / `dismiss_modal`.

use foundation_platform::{PlatformSession, SlotState, WebViewSlot, WebViewState};
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
            _ => Err(IpcError::ExecutionFailed(format!("unknown action '{}'", req.action))),
        }
    }
}

impl ModalIpc {
    fn present(
        session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let args: serde_json::Value = serde_json::from_slice(&req.payload)
            .map_err(|e| IpcError::InvalidPayload(format!("json: {e}")))?;
        let route = args["route"].as_str().unwrap_or("/");
        let _style = args["style"].as_str().unwrap_or("dialog");
        let _title = args["title"].as_str().unwrap_or("");

        let depth = session.webview_stack().depth();
        let modal_label = format!("modal_{depth}");

        // Push a modal slot and ensure the WebView window exists.
        // Same code path as record_presentation() in session.rs.
        let mut stack = session.webview_stack_mut();
        if stack.depth() == 0 {
            stack.init(route);
        }
        if let Some(mut pool) = stack.pool_mut() {
            let win_mgr = session.window_manager();
            win_mgr.ensure(&mut pool, &modal_label, route);
            pool.set_state(&modal_label, WebViewState::Active);
            pool.set_route(&modal_label, route);
        }
        let mut slot = WebViewSlot::new_modal(route);
        slot.state = SlotState::Active;
        stack.push_slot(slot);
        drop(stack);

        let resp = serde_json::json!({
            "modal_id": modal_label,
            "webview_label": modal_label,
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
                .map_err(|e| IpcError::InvalidPayload(alloc::format!("serialize: {e}")))?;
            ipc_dispatch("chrome", IpcRequest {
                ipc: "chrome".into(), action: "present_modal".into(),
                payload, content_type: IpcContentType::Json, target: None,
            })
        }

        pub fn dismiss(args: DismissArgs) -> Result<(), IpcError> {
            let payload = serde_json::to_vec(&args)
                .map_err(|e| IpcError::InvalidPayload(alloc::format!("serialize: {e}")))?;
            let _: serde_json::Value = ipc_dispatch("chrome", IpcRequest {
                ipc: "chrome".into(), action: "dismiss_modal".into(),
                payload, content_type: IpcContentType::Json, target: None,
            })?;
            Ok(())
        }
    }
}

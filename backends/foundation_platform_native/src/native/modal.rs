//! Modal IPC handler — native side (F42 modal capability).
//!
//! Drives WebViewStack + WindowManager for `present_modal` / `dismiss_modal`.
//! F43: callback-based `invoke_with_session` — returns `Ok(())` on dispatch,
//! callback fires with result (immediately for these sync ops).

use std::sync::Arc;

use crate::shared::modal_types::{DismissArgs, PresentArgs, PresentResult};
use foundation_platform::ipc::{IpcCallback, PlatformIpc};
use foundation_platform::{PlatformSession, SlotState, WebViewSlot, WebViewState};
use foundation_platform::stack::WebViewOps;
use foundation_wasm::ipc::{Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse};

/// Register the modal IPC handler. The handler stores its own
/// `Arc<PlatformSession>` — no borrow across the callback boundary.
pub fn register(session: Arc<PlatformSession>) {
    session.register_ipc(ModalIpc { session: Arc::clone(&session) });
}

struct ModalIpc {
    session: Arc<PlatformSession>,
}

impl Ipc<Vec<u8>, Vec<u8>> for ModalIpc {
    fn name(&self) -> &str { "chrome" }
    fn kind(&self) -> IpcKind { IpcKind::Capability }
    fn invoke(&self, _req: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Err(IpcError::ExecutionFailed("use invoke_with_session".into()))
    }
}

impl PlatformIpc for ModalIpc {
    fn invoke_with_session(
        &self,
        req: &IpcRequest<Vec<u8>>,
        callback: IpcCallback,
    ) -> Result<(), IpcError> {
        let result = match req.action.as_str() {
            "present_modal" => Self::handle(&self.session, req),
            "dismiss_modal" => Self::handle(&self.session, req),
            "dismiss_all_modals" => Self::handle(&self.session, req),
            _ => Err(IpcError::ExecutionFailed(format!(
                "chrome: unknown action '{}'", req.action
            ))),
        };
        callback(result);
        Ok(())
    }
}

struct NoopWebView;
impl WebViewOps for NoopWebView {
    fn navigate(&self, _url: &str) {}
    fn screenshot(&self) -> Vec<u8> { Vec::new() }
    fn eval(&self, _script: &str) {}
    fn reload(&self) {}
}

impl ModalIpc {
    fn handle(
        session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        match req.action.as_str() {
            "present_modal" => Self::present(session, req),
            "dismiss_modal" => Self::dismiss(session, req),
            "dismiss_all_modals" => Self::dismiss_all(session),
            other => Err(IpcError::ExecutionFailed(format!("chrome: unknown '{other}'"))),
        }
    }

    fn present(
        session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let typed: IpcRequest<PresentArgs> = req.clone().into_typed()
            .map_err(|e| IpcError::InvalidPayload(format!("{e:?}")))?;
        let args = typed.payload;

        let depth = session.webview_stack().depth();
        let modal_label = format!("modal_{depth}");

        let mut stack = session.webview_stack_mut();
        if stack.depth() == 0 {
            stack.init(&args.route);
        }
        if let Some(mut pool) = stack.pool_mut() {
            let win_mgr = session.window_manager();
            win_mgr.ensure(&mut pool, &modal_label, &args.route);
            pool.set_state(&modal_label, WebViewState::Active);
            pool.set_route(&modal_label, &args.route);
        }
        let mut slot = WebViewSlot::new_modal(&args.route);
        slot.state = SlotState::Active;
        stack.push_slot(slot);
        drop(stack);

        let resp = PresentResult {
            modal_id: modal_label.clone(),
            webview_label: Some(modal_label),
        };
        Ok(IpcResponse {
            payload: serde_json::to_vec(&resp).unwrap(),
            content_type: IpcContentType::Json,
        })
    }

    fn dismiss(
        session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let typed: IpcRequest<DismissArgs> = req.clone().into_typed()
            .map_err(|e| IpcError::InvalidPayload(format!("{e:?}")))?;
        let args = typed.payload;

        let mut stack = session.webview_stack_mut();
        if !args.modal_id.is_empty() {
            if let Some(mut pool) = stack.pool_mut() {
                session.window_manager().destroy(&mut pool, &args.modal_id);
            }
        }
        stack.pop(&NoopWebView);

        Ok(IpcResponse {
            payload: br#"{"ok":true}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
    }

    fn dismiss_all(
        session: &PlatformSession,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let mut stack = session.webview_stack_mut();
        while stack.depth() > 1 {
            stack.pop(&NoopWebView);
        }
        Ok(IpcResponse {
            payload: br#"{"ok":true}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
    }
}

//! Modal IPC handler — native side (F42 modal capability).
//!
//! Calls Kotlin `EwePlatformPlugin.presentModal()` via `PluginHandle`.
//! Kotlin finds the Tauri `RustWebView` in the Activity view hierarchy,
//! detaches it from the decor view, embeds it in a `BottomSheetDialog`,
//! and re-attaches on dismiss. This preserves `__TAURI_INTERNALS__`,
//! init scripts, and the Rust IPC bridge.
//!
//! F43: callback-based `invoke_with_session`.

use std::sync::Arc;

use foundation_platform::ipc::{IpcCallback, PlatformIpc};
use foundation_platform::PlatformSession;
use foundation_wasm::ipc::{Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse};

use crate::shared::modal_types::{DismissArgs, PresentArgs};

/// Register the modal IPC handler.
pub fn register(session: Arc<PlatformSession>) {
    let ipc = ModalIpc {
        session: Arc::clone(&session),
    };
    session.register_ipc(ipc);
}

struct ModalIpc {
    session: Arc<PlatformSession>,
}

impl Ipc<Vec<u8>, Vec<u8>> for ModalIpc {
    fn name(&self) -> &str { "chrome" }
    fn kind(&self) -> IpcKind { IpcKind::Capability }
    fn invoke(&self, _req: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Err(IpcError::ExecutionFailed)
    }
}

impl PlatformIpc for ModalIpc {
    fn invoke_with_session(
        &self,
        req: &IpcRequest<Vec<u8>>,
        callback: IpcCallback,
    ) -> Result<(), IpcError> {
        let result = match req.action.as_str() {
            "present_modal" => Self::present(&self.session, req),
            "dismiss_modal" => Self::dismiss(&self.session, req),
            "dismiss_all_modals" => Self::dismiss_all(&self.session),
            _ => Err(IpcError::ExecutionFailed),
        };
        callback(result);
        Ok(())
    }
}

impl ModalIpc {
    #[cfg(target_os = "android")]
    fn present(
        session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        use tauri::Manager;

        let typed: IpcRequest<PresentArgs> =
            req.clone().into_typed().map_err(|_| IpcError::InvalidPayload)?;
        let args = typed.payload;

        let depth = session.webview_stack().depth();
        let modal_label = format!("modal_{depth}");

        let app_handle = session
            .handles::<tauri::AppHandle<tauri::Wry>>()
            .ok_or(IpcError::ExecutionFailed)?;

        // F44: decorations(false) + inner_size on Android auto-routes to
        // SheetWryActivity (partial-height overlay via CustomWryActivity).
        // On desktop: frameless centered window.
        let builder = tauri::WebviewWindowBuilder::new(
            &app_handle,
            &modal_label,
            tauri::WebviewUrl::App(args.route.clone().into()),
        )
        .visible(false)
        .decorations(false)
        .title(args.title.unwrap_or_else(|| format!("ewe — {modal_label}")));

        builder.build().map_err(|e| {
            tracing::warn!("presentModal build failed: {e:?}");
            IpcError::ExecutionFailed
        })?;

        // Update the WebView pool so dismiss works
        let mut stack = session.webview_stack_mut();
        if let Some(mut pool) = stack.pool_mut() {
            pool.get_or_create(&modal_label);
            pool.set_route(&modal_label, &args.route);
        }

        // Push a modal slot so dismiss_all knows about it
        let slot = foundation_platform::WebViewSlot::new_modal(&args.route);
        stack.push_slot(slot);

        let resp = crate::shared::modal_types::PresentResult {
            modal_id: modal_label.clone(),
            webview_label: Some(modal_label),
        };
        let payload = serde_json::to_vec(&resp).unwrap_or_default();
        Ok(IpcResponse { payload, content_type: IpcContentType::Json })
    }

    #[cfg(target_os = "android")]
    fn dismiss(
        session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let typed: IpcRequest<DismissArgs> =
            req.clone().into_typed().map_err(|_| IpcError::InvalidPayload)?;

        // Close the sheet window via WindowManager
        let label = if typed.payload.modal_id.is_empty() {
            let stack = session.webview_stack();
            let depth = stack.depth();
            if depth <= 1 { return Ok(IpcResponse { payload: br#"{"ok":true}"#.to_vec(), content_type: IpcContentType::Json }); }
            format!("modal_{}", depth - 1)
        } else {
            typed.payload.modal_id.clone()
        };

        let mut stack = session.webview_stack_mut();
        if let Some(mut pool) = stack.pool_mut() {
            session.window_manager().destroy(&mut pool, &label);
        }
        stack.pop_with(|_, _| {});

        Ok(IpcResponse { payload: br#"{"ok":true}"#.to_vec(), content_type: IpcContentType::Json })
    }

    #[cfg(target_os = "android")]
    fn dismiss_all(session: &PlatformSession) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        use tauri::Manager;

        let app_handle = session
            .handles::<tauri::AppHandle<tauri::Wry>>()
            .ok_or(IpcError::ExecutionFailed)?;

        let handles = app_handle.state::<super::plugin::EweNativeHandles>();
        let _: serde_json::Value = handles
            .modal
            .run_mobile_plugin::<serde_json::Value>("dismissAllModals", ())
            .map_err(|e| {
                tracing::warn!("dismissAllModals failed: {e:?}");
                IpcError::ExecutionFailed
            })?;

        Ok(IpcResponse { payload: br#"{"ok":true}"#.to_vec(), content_type: IpcContentType::Json })
    }

    // Desktop stubs
    #[cfg(not(target_os = "android"))]
    fn present(
        _session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let _typed: IpcRequest<PresentArgs> =
            req.clone().into_typed().map_err(|_| IpcError::InvalidPayload)?;
        tracing::info!("presentModal: not on android — stub");
        Ok(IpcResponse {
            payload: br#"{"modal_id":"stub","webview_label":"stub"}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
    }

    #[cfg(not(target_os = "android"))]
    fn dismiss(
        _session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let _typed: IpcRequest<DismissArgs> =
            req.clone().into_typed().map_err(|_| IpcError::InvalidPayload)?;
        tracing::info!("dismissModal: not on android — stub");
        Ok(IpcResponse { payload: br#"{"ok":true}"#.to_vec(), content_type: IpcContentType::Json })
    }

    #[cfg(not(target_os = "android"))]
    fn dismiss_all(_session: &PlatformSession) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        tracing::info!("dismissAllModals: not on android — stub");
        Ok(IpcResponse { payload: br#"{"ok":true}"#.to_vec(), content_type: IpcContentType::Json })
    }
}

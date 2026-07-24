//! Modal IPC handler — native side (F42 modal capability).
//!
//! On Android: bridges WASM IPC → PluginHandle → Kotlin `EwePlatformPlugin`
//! (creates real `BottomSheetDialog` / `AlertDialog` with embedded `WebView`).
//!
//! On desktop: stub — no native dialog stack; returns synthetic success.

use std::sync::Arc;

use foundation_platform::ipc::{IpcCallback, PlatformIpc};
use foundation_platform::PlatformSession;
use foundation_wasm::ipc::{Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse};

use crate::shared::modal_types::{DismissArgs, PresentArgs};

/// Register the modal IPC handler.
pub fn register(session: Arc<PlatformSession>) {
    session.register_ipc(ModalIpc {
        session: Arc::clone(&session),
    });
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
            "present_modal" => Self::handle(&self.session, req),
            "dismiss_modal" => Self::handle(&self.session, req),
            "dismiss_all_modals" => Self::handle(&self.session, req),
            _ => Err(IpcError::ExecutionFailed),
        };
        callback(result);
        Ok(())
    }
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
            _ => Err(IpcError::ExecutionFailed),
        }
    }

    #[cfg(target_os = "android")]
    fn present(
        session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        use tauri::Manager;

        let typed: IpcRequest<PresentArgs> =
            req.clone().into_typed().map_err(|_| IpcError::InvalidPayload)?;

        let app_handle = session
            .handles::<tauri::AppHandle<tauri::Wry>>()
            .ok_or(IpcError::ExecutionFailed)?;

        let handles = app_handle.state::<super::plugin::EweNativeHandles>();
        let response: serde_json::Value = handles
            .modal
            .run_mobile_plugin("presentModal", &typed.payload)
            .map_err(|e| {
                tracing::warn!("presentModal failed: {e:?}");
                IpcError::ExecutionFailed
            })?;

        let payload = serde_json::to_vec(&response).unwrap_or_default();
        Ok(IpcResponse {
            payload,
            content_type: IpcContentType::Json,
        })
    }

    #[cfg(target_os = "android")]
    fn dismiss(
        session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        use tauri::Manager;

        let typed: IpcRequest<DismissArgs> =
            req.clone().into_typed().map_err(|_| IpcError::InvalidPayload)?;

        let app_handle = session
            .handles::<tauri::AppHandle<tauri::Wry>>()
            .ok_or(IpcError::ExecutionFailed)?;

        let handles = app_handle.state::<super::plugin::EweNativeHandles>();
        let _: serde_json::Value = handles
            .modal
            .run_mobile_plugin("dismissModal", &typed.payload)
            .map_err(|e| {
                tracing::warn!("dismissModal failed: {e:?}");
                IpcError::ExecutionFailed
            })?;

        Ok(IpcResponse {
            payload: br#"{"ok":true}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
    }

    #[cfg(target_os = "android")]
    fn dismiss_all(
        session: &PlatformSession,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        use tauri::Manager;

        let app_handle = session
            .handles::<tauri::AppHandle<tauri::Wry>>()
            .ok_or(IpcError::ExecutionFailed)?;

        let handles = app_handle.state::<super::plugin::EweNativeHandles>();
        let _: serde_json::Value = handles
            .modal
            .run_mobile_plugin("dismissAllModals", ())
            .map_err(|e| {
                tracing::warn!("dismissAllModals failed: {e:?}");
                IpcError::ExecutionFailed
            })?;

        Ok(IpcResponse {
            payload: br#"{"ok":true}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
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
        Ok(IpcResponse {
            payload: br#"{"ok":true}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
    }

    #[cfg(not(target_os = "android"))]
    fn dismiss_all(_session: &PlatformSession) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        tracing::info!("dismissAllModals: not on android — stub");
        Ok(IpcResponse {
            payload: br#"{"ok":true}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
    }
}

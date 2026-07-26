//! Dialog IPC handler — native side (F42 dialog capability).
//!
//! On Android: bridges WASM IPC → PluginHandle → Kotlin `EwePlatformPlugin`
//! (creates native `AlertDialog`). F43: callback-based invoke.

use std::sync::Arc;

use foundation_platform::ipc::{IpcCallback, PlatformIpc};
use foundation_platform::PlatformSession;
use foundation_wasm::ipc::{Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse};

use crate::shared::dialog_types::{ShowArgs, ShowResult};

/// Register the dialog IPC handler.
pub fn register(session: Arc<PlatformSession>) {
    session.register_ipc(DialogIpc { session: Arc::clone(&session) });
}

struct DialogIpc {
    session: Arc<PlatformSession>,
}

impl Ipc<Vec<u8>, Vec<u8>> for DialogIpc {
    fn name(&self) -> &str { "dialog" }
    fn kind(&self) -> IpcKind { IpcKind::Capability }
    fn invoke(&self, _req: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Err(IpcError::ExecutionFailed)
    }
}

impl PlatformIpc for DialogIpc {
    fn invoke_with_session(
        &self,
        req: &IpcRequest<Vec<u8>>,
        callback: IpcCallback,
    ) -> Result<(), IpcError> {
        let result = match req.action.as_str() {
            "show" => Self::show(&self.session, req),
            "dismiss" => Self::dismiss(&self.session, req),
            _ => Err(IpcError::ExecutionFailed),
        };
        callback(result);
        Ok(())
    }
}

impl DialogIpc {
    #[cfg(target_os = "android")]
    fn show(
        session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        use tauri::Manager;

        let typed: IpcRequest<ShowArgs> =
            req.clone().into_typed().map_err(|_| IpcError::InvalidPayload)?;

        // When a route is provided, create a Tauri WebView dialog (DialogWryActivity).
        // When no route, fall back to native text-only AlertDialog via PluginHandle.
        if let Some(ref route) = typed.payload.route {
            let depth = session.webview_stack().depth();
            let dialog_label = format!("dialog_{depth}");

            let app_handle = session
                .handles::<tauri::AppHandle<tauri::Wry>>()
                .ok_or(IpcError::ExecutionFailed)?
                .clone();

            let mut builder = tauri::WebviewWindowBuilder::new(
                &app_handle,
                &dialog_label,
                tauri::WebviewUrl::App(route.clone().into()),
            )
            .title(typed.payload.title.clone());

            #[cfg(target_os = "android")]
            {
                let dialog_class = String::from("DialogWryActivity");
                builder = builder.activity_name(dialog_class);
            }

            let _window = builder.build().map_err(|e| {
                tracing::warn!("showDialog build failed: {e:?}");
                IpcError::ExecutionFailed
            })?;

            let mut stack = session.webview_stack_mut();
            if let Some(mut pool) = stack.pool_mut() {
                pool.get_or_create(&dialog_label);
                pool.set_route(&dialog_label, route);
            }
            stack.push_slot(foundation_platform::WebViewSlot::new_modal(route));

            let resp = serde_json::json!({"dialog_id": dialog_label});
            Ok(IpcResponse {
                payload: serde_json::to_vec(&resp).unwrap_or_default(),
                content_type: IpcContentType::Json,
            })
        } else {
            // Text-only AlertDialog via Kotlin PluginHandle
            let app_handle = session
                .handles::<tauri::AppHandle<tauri::Wry>>()
                .ok_or(IpcError::ExecutionFailed)?;

            let handles = app_handle.state::<super::plugin::EweNativeHandles>();
            let response: serde_json::Value = handles
                .modal
                .run_mobile_plugin("showDialog", &typed.payload)
                .map_err(|e| {
                    tracing::warn!("showDialog failed: {e:?}");
                    IpcError::ExecutionFailed
                })?;

            let payload = serde_json::to_vec(&response).unwrap_or_default();
            Ok(IpcResponse {
                payload,
                content_type: IpcContentType::Json,
            })
        }
    }

    #[cfg(target_os = "android")]
    fn dismiss(
        session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        use tauri::Manager;

        let app_handle = session
            .handles::<tauri::AppHandle<tauri::Wry>>()
            .ok_or(IpcError::ExecutionFailed)?;

        let handles = app_handle.state::<super::plugin::EweNativeHandles>();
        let args: serde_json::Value =
            serde_json::from_slice(&req.payload).map_err(|_| IpcError::InvalidPayload)?;
        let _: serde_json::Value = handles
            .modal
            .run_mobile_plugin("dismissDialog", &args)
            .map_err(|e| {
                tracing::warn!("dismissDialog failed: {e:?}");
                IpcError::ExecutionFailed
            })?;

        Ok(IpcResponse {
            payload: br#"{"ok":true}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
    }

    // Desktop stubs
    #[cfg(not(target_os = "android"))]
    fn show(
        _session: &PlatformSession,
        req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let _typed: IpcRequest<ShowArgs> =
            req.clone().into_typed().map_err(|_| IpcError::InvalidPayload)?;
        tracing::info!("showDialog: not on android — stub");
        Ok(IpcResponse {
            payload: br#"{"dialog_id":"stub"}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
    }

    #[cfg(not(target_os = "android"))]
    fn dismiss(
        _session: &PlatformSession,
        _req: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        tracing::info!("dismissDialog: not on android — stub");
        Ok(IpcResponse {
            payload: br#"{"ok":true}"#.to_vec(),
            content_type: IpcContentType::Json,
        })
    }
}

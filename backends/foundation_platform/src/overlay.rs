//! Platform overlay system (F29 Stage 4).
//!
//! WHY: Tauri has no built-in overlay/toolbar system. The platform needs a
//! way to render navigation bars, slide panels, and modal overlays on top
//! of WebView content — without native bridge overhead.
//!
//! WHAT: `OverlayCapability` is a `PlatformIpc` that injects the
//! `floating-nav.js` toolbar into the WebView. `OverlayConfig` lets route
//! handlers customize position, visibility, and toolbar buttons.
//!
//! HOW: The overlay is pure CSS + JS injected by the `ScriptInjector`.
//! `OverlayCapability` provides a programmatic API for route handlers to
//! show/hide/configure the toolbar without calling `window.eval()` directly.

use foundation_wasm::ipc::{IpcError, IpcRequest, IpcResponse};
use foundation_ui_traits::{CapabilityId, Profile};

use crate::capability::PlatformIpc;

// ── Overlay config ───────────────────────────────────────────────────────

/// Configuration for the floating navigation toolbar.
#[derive(Debug, Clone)]
pub struct OverlayConfig {
    /// Toolbar position: `"top"` or `"bottom"`.
    pub position: OverlayPosition,
    /// Whether the toolbar is visible on page load.
    pub visible: bool,
    /// Title text shown in the center of the toolbar.
    pub title: Option<String>,
    /// Show the back button.
    pub show_back: bool,
    /// Show the home button.
    pub show_home: bool,
    /// Show the refresh button.
    pub show_refresh: bool,
    /// Show the app-switcher button.
    pub show_app_switcher: bool,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            position: OverlayPosition::Bottom,
            visible: true,
            title: None,
            show_back: true,
            show_home: true,
            show_refresh: true,
            show_app_switcher: true,
        }
    }
}

/// Toolbar position on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayPosition {
    /// Fixed to top of viewport.
    Top,
    /// Fixed to bottom of viewport.
    Bottom,
}

impl OverlayConfig {
    /// Generate the JS to configure the floating nav.
    #[must_use]
    pub fn to_js(&self) -> String {
        let pos = match self.position {
            OverlayPosition::Top => "top",
            OverlayPosition::Bottom => "bottom",
        };
        let visibility = if self.visible { "show()" } else { "hide()" };
        let title_js = self
            .title
            .as_ref()
            .map(|t| format!("__eweNav.setTitle({t:?});"))
            .unwrap_or_default();

        format!(
            "if(window.__eweNav){{__eweNav.setPosition('{pos}');{title_js}__eweNav.{visibility}}}"
        )
    }
}

// ── Overlay capability ───────────────────────────────────────────────────

/// A `PlatformIpc` that lets route handlers control the floating
/// navigation toolbar overlay.
///
/// Registered on the session like any other capability. Route handlers
/// call it to show/hide/configure the toolbar based on the current page.
pub struct OverlayCapability;

impl OverlayCapability {
    /// Capability name as registered in the registry.
    pub const NAME: &str = "overlay";

    /// Build JS to inject that shows the toolbar with the given config.
    #[must_use]
    pub fn inject_js(config: &OverlayConfig) -> String {
        config.to_js()
    }

    /// Build JS to hide the toolbar.
    #[must_use]
    pub fn hide_js() -> String {
        "if(window.__eweNav)__eweNav.hide()".to_string()
    }

    /// Build JS to show the toolbar.
    #[must_use]
    pub fn show_js() -> String {
        "if(window.__eweNav)__eweNav.show()".to_string()
    }

    /// Build JS to set the title.
    #[must_use]
    pub fn title_js(title: &str) -> String {
        format!("if(window.__eweNav)__eweNav.setTitle({title:?})")
    }
}

impl foundation_wasm::ipc::Ipc<Vec<u8>, Vec<u8>> for OverlayCapability {
    fn name(&self) -> &str {
        Self::NAME
    }

    fn kind(&self) -> foundation_wasm::ipc::IpcKind {
        foundation_wasm::ipc::IpcKind::Capability
    }

    fn invoke(
        &self,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        let action = request.action.as_str();
        let payload_str = String::from_utf8_lossy(&request.payload);

        let js = match action {
            "show" => Self::show_js(),
            "hide" => Self::hide_js(),
            "set_title" => Self::title_js(&payload_str),
            "configure" => {
                if let Ok(config) = serde_json::from_str::<serde_json::Value>(&payload_str) {
                    let oc = OverlayConfig {
                        position: if config
                            .get("position")
                            .and_then(|v| v.as_str())
                            .unwrap_or("bottom")
                            == "top"
                        {
                            OverlayPosition::Top
                        } else {
                            OverlayPosition::Bottom
                        },
                        visible: config
                            .get("visible")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true),
                        title: config
                            .get("title")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        show_back: config
                            .get("show_back")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true),
                        show_home: config
                            .get("show_home")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true),
                        show_refresh: config
                            .get("show_refresh")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true),
                        show_app_switcher: config
                            .get("show_app_switcher")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true),
                    };
                    oc.to_js()
                } else {
                    return Err(IpcError::InvalidPayload(
                        "overlay configure requires JSON payload".into(),
                    ));
                }
            }
            _ => {
                return Err(IpcError::ExecutionFailed(format!(
                    "overlay: unknown action '{action}'"
                )))
            }
        };

        Ok(IpcResponse {
            payload: js.into_bytes(),
            content_type: request.content_type,
        })
    }
}

impl PlatformIpc for OverlayCapability {
    fn capability_id(&self) -> &CapabilityId {
        // Leak is intentional — static lifetime for capability IDs.
        Box::leak(Box::new(CapabilityId("overlay".into())))
    }

    fn min_profile(&self) -> Profile {
        // Overlay is available to TrustedRemote and above.
        // UntrustedRemote cannot manipulate the toolbar.
        Profile::TrustedRemote
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use foundation_wasm::ipc::{Ipc, IpcContentType};

    #[test]
    fn overlay_config_default_to_js() {
        let config = OverlayConfig::default();
        let js = config.to_js();
        assert!(js.contains("bottom"));
        assert!(js.contains("show()"));
    }

    #[test]
    fn overlay_config_hidden() {
        let config = OverlayConfig {
            visible: false,
            ..Default::default()
        };
        let js = config.to_js();
        assert!(js.contains("hide()"));
    }

    #[test]
    fn overlay_config_with_title() {
        let config = OverlayConfig {
            title: Some("Settings".into()),
            ..Default::default()
        };
        let js = config.to_js();
        assert!(js.contains("Settings"));
    }

    #[test]
    fn overlay_capability_show_action() {
        let cap = OverlayCapability;
        let req = IpcRequest {
            ipc: "overlay".into(),
            action: "show".into(),
            payload: vec![],
            content_type: IpcContentType::Json,
            target: None,
        };
        let resp = cap.invoke(&req).unwrap();
        let js = String::from_utf8(resp.payload).unwrap();
        assert!(js.contains("__eweNav.show()"));
    }

    #[test]
    fn overlay_capability_hide_action() {
        let cap = OverlayCapability;
        let req = IpcRequest {
            ipc: "overlay".into(),
            action: "hide".into(),
            payload: vec![],
            content_type: IpcContentType::Json,
            target: None,
        };
        let resp = cap.invoke(&req).unwrap();
        let js = String::from_utf8(resp.payload).unwrap();
        assert!(js.contains("__eweNav.hide()"));
    }

    #[test]
    fn overlay_capability_set_title() {
        let cap = OverlayCapability;
        let req = IpcRequest {
            ipc: "overlay".into(),
            action: "set_title".into(),
            payload: b"Home".to_vec(),
            content_type: IpcContentType::Json,
            target: None,
        };
        let resp = cap.invoke(&req).unwrap();
        let js = String::from_utf8(resp.payload).unwrap();
        assert!(js.contains("Home"));
    }
}

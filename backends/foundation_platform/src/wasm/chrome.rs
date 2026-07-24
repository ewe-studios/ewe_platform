//! Typed WASM wrapper for the `chrome` IPC — native UI chrome.

use foundation_wasm::ipc::IpcError;
use super::dispatch_json;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ToolbarButton {
    pub id: String,
    pub label: String,
    pub position: Option<String>,  // "left" | "right"
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Tab {
    pub id: String,
    pub label: String,
    pub route: String,
    pub icon: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct OverlayConfig {
    pub title: Option<String>,
    pub position: Option<String>,    // "top" | "bottom"
    pub visible: Option<bool>,
    pub show_back: Option<bool>,
}

pub struct Chrome;

impl Chrome {
    /// Set the native toolbar title.
    pub fn set_title(title: &str) -> Result<(), IpcError> {
        dispatch_json("chrome", "set_title", serde_json::json!({ "title": title }))
    }

    /// Add a button to the native toolbar.
    /// When tapped, the host fires an `ipc_handle_event` with event = "chrome:toolbar_tap".
    pub fn add_toolbar_button(btn: ToolbarButton) -> Result<(), IpcError> {
        dispatch_json("chrome", "add_toolbar_button", btn)
    }

    /// Remove a toolbar button by ID.
    pub fn remove_toolbar_button(id: &str) -> Result<(), IpcError> {
        dispatch_json("chrome", "remove_toolbar_button", serde_json::json!({ "id": id }))
    }

    /// Show / hide / configure the floating overlay toolbar.
    pub fn configure_overlay(cfg: OverlayConfig) -> Result<(), IpcError> {
        dispatch_json("chrome", "configure_overlay", cfg)
    }
}

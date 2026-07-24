//! Wire-format types for the modal IPC — shared between native handler
//! and WASM wrapper. Implements `WirePayload` so both sides pass concrete
//! types through the FFI — serialization lives here, not in the callers.

use foundation_wasm::ipc::{IpcContentType, WireError, WirePayload};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct PresentArgs {
    #[serde(rename = "url", alias = "route")]
    pub route: String,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

impl WirePayload for PresentArgs {
    fn into_wire_bytes(self) -> (Vec<u8>, IpcContentType) {
        let bytes = serde_json::to_vec(&self).unwrap_or_default();
        (bytes, IpcContentType::Json)
    }
    fn from_wire_bytes(data: &[u8], _ct: IpcContentType) -> Result<Self, WireError> {
        serde_json::from_slice(data).map_err(|_| WireError::DecodeFailed)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct PresentResult {
    pub modal_id: String,
    #[serde(default)]
    pub webview_label: Option<String>,
}

impl WirePayload for PresentResult {
    fn into_wire_bytes(self) -> (Vec<u8>, IpcContentType) {
        let bytes = serde_json::to_vec(&self).unwrap_or_default();
        (bytes, IpcContentType::Json)
    }
    fn from_wire_bytes(data: &[u8], _ct: IpcContentType) -> Result<Self, WireError> {
        serde_json::from_slice(data).map_err(|_| WireError::DecodeFailed)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct DismissArgs {
    pub modal_id: String,
}

impl WirePayload for DismissArgs {
    fn into_wire_bytes(self) -> (Vec<u8>, IpcContentType) {
        let bytes = serde_json::to_vec(&self).unwrap_or_default();
        (bytes, IpcContentType::Json)
    }
    fn from_wire_bytes(data: &[u8], _ct: IpcContentType) -> Result<Self, WireError> {
        serde_json::from_slice(data).map_err(|_| WireError::DecodeFailed)
    }
}

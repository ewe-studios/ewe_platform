//! Wire-format types for the dialog IPC — shared between native handler
//! and WASM wrapper.

use foundation_wasm::ipc::{IpcContentType, WireError, WirePayload};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ShowArgs {
    pub title: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub positive_button: Option<String>,
    #[serde(default)]
    pub negative_button: Option<String>,
    #[serde(default)]
    pub route: Option<String>,
}

impl WirePayload for ShowArgs {
    fn into_wire_bytes(self) -> (Vec<u8>, IpcContentType) {
        let bytes = serde_json::to_vec(&self).unwrap_or_default();
        (bytes, IpcContentType::Json)
    }
    fn from_wire_bytes(data: &[u8], _ct: IpcContentType) -> Result<Self, WireError> {
        serde_json::from_slice(data).map_err(|_| WireError::DecodeFailed)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ShowResult {
    pub dialog_id: String,
}

impl WirePayload for ShowResult {
    fn into_wire_bytes(self) -> (Vec<u8>, IpcContentType) {
        let bytes = serde_json::to_vec(&self).unwrap_or_default();
        (bytes, IpcContentType::Json)
    }
    fn from_wire_bytes(data: &[u8], _ct: IpcContentType) -> Result<Self, WireError> {
        serde_json::from_slice(data).map_err(|_| WireError::DecodeFailed)
    }
}

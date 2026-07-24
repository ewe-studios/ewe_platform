//! Typed WASM wrapper for the `filesystem` IPC — file picker and read.

use foundation_wasm::ipc::IpcError;
use super::dispatch_json;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PickArgs {
    pub accept: Option<Vec<String>>,  // MIME types, e.g. ["image/*"]
    pub multiple: Option<bool>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PickedFile {
    pub uri: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ReadArgs { pub uri: String }

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FileBytes { pub data: Vec<u8>, pub mime: String }

pub struct FilePicker;

impl FilePicker {
    /// Open the native file picker. Returns metadata + URIs.
    pub fn pick(args: PickArgs) -> Result<Vec<PickedFile>, IpcError> {
        dispatch_json("filesystem", "pick", args)
    }
    /// Read a file by URI into memory.
    pub fn read(args: ReadArgs) -> Result<FileBytes, IpcError> {
        dispatch_json("filesystem", "read", args)
    }
}

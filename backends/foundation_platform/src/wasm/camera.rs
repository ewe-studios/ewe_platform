//! Typed WASM wrapper for the `camera` IPC.

use foundation_wasm::ipc::IpcError;
use super::dispatch_json;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct OpenArgs { pub facing: Option<String>, pub mode: Option<String> }

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CameraHandle { pub handle: u64 }

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CaptureArgs { pub handle: u64 }

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PhotoResult { pub uri: String, pub mime: String }

pub struct Camera;

impl Camera {
    pub fn open(args: OpenArgs) -> Result<CameraHandle, IpcError> {
        dispatch_json("camera", "open", args)
    }
    pub fn capture(args: CaptureArgs) -> Result<PhotoResult, IpcError> {
        dispatch_json("camera", "capture", args)
    }
    pub fn close(handle: u64) -> Result<(), IpcError> {
        dispatch_json("camera", "close", serde_json::json!({ "handle": handle }))
    }
}

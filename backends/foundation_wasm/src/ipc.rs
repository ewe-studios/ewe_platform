//! Platform-agnostic IPC types (F25).
//!
//! WHY: The `IpcRequest`/`IpcResponse` contract must be available to WASM apps
//! running on any host (Tauri, browser, Deno). These types live in
//! `foundation_wasm` so they compile for wasm32 and native without platform
//! dependencies.
//!
//! WHAT: `IpcRequest`, `IpcResponse`, `IpcError`, `IpcContentType` —
//! the wire types for all IPC communication.
//!
//! HOW: no_std compatible. The `Ipc` trait + registry live in
//! `foundation_platform` (they need PlatformSession).

use alloc::string::String;
use alloc::vec::Vec;

/// Content type for IPC payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcContentType {
    /// JSON-encoded payload.
    Json,
    /// Arrow columnar payload.
    Arrow,
    /// Raw binary payload.
    Binary,
}

/// A request from the frontend to an IPC handler.
#[derive(Debug, Clone)]
pub struct IpcRequest {
    /// The IPC name (matches a registered `Ipc::name()`).
    pub ipc: String,
    /// The action or sub-command within this IPC.
    pub action: String,
    /// Serialized payload. Encoding per `content_type`.
    pub payload: Vec<u8>,
    pub content_type: IpcContentType,
    /// For Emit IPCs: the target webview label, or None for broadcast.
    pub target: Option<String>,
}

/// The response from an IPC handler.
#[derive(Debug, Clone)]
pub struct IpcResponse {
    /// Serialized result. Encoding per `content_type`.
    pub payload: Vec<u8>,
    pub content_type: IpcContentType,
}

/// Errors from IPC invocation.
#[derive(Debug, Clone)]
pub enum IpcError {
    /// No IPC registered under this name.
    UnknownIpc(String),
    /// The payload could not be decoded.
    InvalidPayload(String),
    /// The IPC handler failed.
    ExecutionFailed(String),
    /// The caller does not have permission.
    PermissionDenied(String),
    /// Domain-level error with a machine-readable code.
    DomainError { code: String, message: String },
}

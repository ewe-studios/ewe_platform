//! Platform-agnostic IPC types (F25).
//!
//! WHY: The `IpcRequest<T>` / `IpcResponse<T>` contract must be available
//! to WASM apps running on any host (Tauri, browser, Deno). These types live
//! in `foundation_wasm` so they compile for wasm32 and native without
//! platform dependencies.
//!
//! WHAT: Generic `IpcRequest<T = Vec<u8>>`, `IpcResponse<T = Vec<u8>>`,
//! `IpcError`, `IpcContentType` — the wire types for all IPC communication.
//! Default `T = Vec<u8>` is the on-wire form. Any `T: WirePayload` can
//! round-trip through `into_wire()` / `into_typed()`.
//!
//! HOW: `no_std` compatible. The `Ipc` trait + registry live in
//! `foundation_platform` (they need `PlatformSession`).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::capability::{CapabilityContentType, WireError, WirePayload};

// ── Content type ────────────────────────────────────────────────────────

/// Re-use capability's content type for IPC. The enum variants (Json, Arrow)
/// are the same — only the context differs.
pub type IpcContentType = CapabilityContentType;

// ── Wire types (generic over payload T) ─────────────────────────────────

/// A request from the frontend to an IPC handler.
///
/// Default `T = Vec<u8>` is the on-wire form. Any `T: WirePayload` can
/// go onto the wire via `self.into_wire()`.
#[derive(Debug, Clone)]
pub struct IpcRequest<T = Vec<u8>> {
    /// The IPC name (matches a registered `Ipc::name()`).
    pub ipc: String,
    /// The action or sub-command within this IPC.
    pub action: String,
    /// The typed payload.
    pub payload: T,
    /// Payload encoding.
    pub content_type: IpcContentType,
    /// For Emit IPCs: the target webview label, or None for broadcast.
    pub target: Option<String>,
}

/// The response from an IPC handler.
#[derive(Debug, Clone)]
pub struct IpcResponse<T = Vec<u8>> {
    /// The typed result.
    pub payload: T,
    /// Result encoding.
    pub content_type: IpcContentType,
}

// ── into_wire / into_typed on requests ─────────────────────────────────

impl<T: WirePayload> IpcRequest<T> {
    /// Convert a typed request into the wire form.
    pub fn into_wire(self) -> IpcRequest<Vec<u8>> {
        let (payload, content_type) = self.payload.into_wire_bytes();
        IpcRequest {
            ipc: self.ipc,
            action: self.action,
            payload,
            content_type,
            target: self.target,
        }
    }

    /// Create a wire request from typed parts.
    pub fn wire(ipc: impl Into<String>, action: impl Into<String>, payload: T) -> IpcRequest<Vec<u8>> {
        IpcRequest {
            ipc: ipc.into(),
            action: action.into(),
            payload,
            content_type: IpcContentType::Json,
            target: None,
        }.into_wire()
    }
}

impl IpcRequest<Vec<u8>> {
    /// Convert a wire request back into a typed request.
    pub fn into_typed<T: WirePayload>(self) -> Result<IpcRequest<T>, WireError> {
        let payload = T::from_wire_bytes(&self.payload, self.content_type)?;
        Ok(IpcRequest {
            ipc: self.ipc,
            action: self.action,
            payload,
            content_type: self.content_type,
            target: self.target,
        })
    }
}

// ── into_wire / into_typed on responses ────────────────────────────────

impl<T: WirePayload> IpcResponse<T> {
    /// Convert a typed response into the wire form.
    pub fn into_wire(self) -> IpcResponse<Vec<u8>> {
        let (payload, content_type) = self.payload.into_wire_bytes();
        IpcResponse { payload, content_type }
    }

    /// Create a wire response from typed parts.
    pub fn wire(payload: T) -> IpcResponse<Vec<u8>> {
        IpcResponse {
            payload,
            content_type: IpcContentType::Json,
        }.into_wire()
    }
}

impl IpcResponse<Vec<u8>> {
    /// Convert a wire response back into a typed response.
    pub fn into_typed<T: WirePayload>(self) -> Result<IpcResponse<T>, WireError> {
        let payload = T::from_wire_bytes(&self.payload, self.content_type)?;
        Ok(IpcResponse { payload, content_type: self.content_type })
    }
}

// ── Errors ─────────────────────────────────────────────────────────────

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

impl From<WireError> for IpcError {
    fn from(e: WireError) -> Self {
        match e {
            WireError::DecodeFailed(msg) => IpcError::InvalidPayload(msg),
            WireError::EncodeFailed(msg) => IpcError::ExecutionFailed(msg),
            WireError::UnsupportedContentType(ct) => {
                IpcError::InvalidPayload(format!("unsupported content type: {ct:?}"))
            }
        }
    }
}

// ── IpcKind ─────────────────────────────────────────────────────────────

/// The transport kind for an IPC handler.
///
/// Streaming is NOT a variant — streaming needs a different contract
/// (bidirectional chunks, backpressure). See `StreamingIpc` (F26).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcKind {
    /// Request → Response. Single reply (like Tauri invoke).
    Query,
    /// Fire-and-forget. No response expected (like Tauri events).
    Emit,
    /// Request → Response, rendered as a page. Can also serve as an
    /// HTTP route handler when registered with a platform that supports it.
    Page,
}

// ── Ipc trait ───────────────────────────────────────────────────────────

/// A portable IPC handler. Works on wasm32 and native.
///
/// Platform crates (foundation_platform) extend this with additional methods
/// (`as_route_handler()`, security gates, etc.) via the `PlatformIpc` trait.
///
/// A portable IPC handler. Works on wasm32 and native.
///
/// On native targets, implementors should also be `Send + Sync` if stored
/// in a shared registry — but that bound lives on the registry's storage
/// type, not here.
#[cfg(target_family = "wasm")]
pub trait Ipc {
    fn name(&self) -> &str;
    fn kind(&self) -> IpcKind;
    fn invoke(
        &self,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError>;
}

#[cfg(not(target_family = "wasm"))]
pub trait Ipc: Send + Sync {
    fn name(&self) -> &str;
    fn kind(&self) -> IpcKind;
    fn invoke(
        &self,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError>;
}

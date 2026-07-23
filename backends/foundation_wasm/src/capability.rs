//! Backward-compatible re-exports — F41 merged this crate into `ipc`.
//!
//! All capability types now live in [`crate::ipc`]. This module re-exports
//! them under their old names so existing code compiles without changes.
//!
//! New code should use the `ipc` module directly:
//!   - `IpcRequest<T>` instead of `CapabilityRequest<T>`
//!   - `IpcResponse<T>` instead of `CapabilityResponse<T>`
//!   - `IpcError` instead of `CapabilityError`
//!   - `IpcContentType` instead of `CapabilityContentType`
//!   - `Ipc<Input, Output>` instead of `WasmCapability`
//!   - `IpcRegistry` instead of `CapabilityRegistry`

pub use crate::ipc::{
    CapabilityContentType, CapabilityError, CapabilityRegistry, CapabilityRequest,
    CapabilityResponse, Ipc, IpcContentType, IpcError, IpcKind, IpcRegistry, IpcRequest,
    IpcResponse, WasmCapability, WireError, WirePayload,
    is_valid_capability_name, is_valid_ipc_name,
};

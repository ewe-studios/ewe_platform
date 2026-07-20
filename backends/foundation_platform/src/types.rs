//! F05 native capability request/response types — carry `serde_json::Value`
//! payloads which don't belong in `foundation_wasm` (`no_std`).
//!
//! F23 moved portable bytes-based types to `foundation_wasm::capability`.
//! These are renamed with the `Native` prefix to avoid collision.

use foundation_ui_traits::PageIdentity;

/// A request from the web side for a native capability (F05).
/// Routed through the session backbone to the registered handler.
#[derive(Debug, Clone)]
pub struct NativeCapabilityRequest {
    /// Unique request ID for matching response to caller.
    pub id: String,

    /// Which page made the request (for stale-page guard).
    pub page_identity: PageIdentity,

    /// The capability being invoked (e.g. "camera", "`biometric_auth`").
    pub capability: String,

    /// What action to perform (e.g. "capture", "authenticate").
    pub action: String,

    /// Action-specific parameters.
    pub payload: serde_json::Value,
}

/// The result of a capability invocation (F05).
/// Delivered scoped to the requesting page via the session backbone.
#[derive(Debug, Clone)]
pub struct NativeCapabilityResponse {
    /// Matches the request's `id` for correlation.
    pub id: String,

    /// Which page the response is for (must match the request's
    /// `page_identity`).
    pub page_identity: PageIdentity,

    /// `Ok(value)` on success, `Err(message)` on failure.
    pub status: Result<serde_json::Value, String>,
}

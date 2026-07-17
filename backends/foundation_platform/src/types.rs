//! Capability contract types — live in `foundation_platform` (not
//! `foundation_ui_traits`) because they carry `serde_json::Value` payloads
//! which don't belong in a `no_std` crate.
//!
//! Serialize/Deserialize derives are deferred to F05 (capability registry).
//! For F00, these are plain structs that compile without serde on PageIdentity.

use foundation_ui_traits::PageIdentity;

/// A request from the web side for a native capability.
/// Routed through the session backbone to the registered handler.
#[derive(Debug, Clone)]
pub struct CapabilityRequest {
    /// Unique request ID for matching response to caller.
    pub id: String,

    /// Which page made the request (for stale-page guard).
    pub page_identity: PageIdentity,

    /// The capability being invoked (e.g. "camera", "biometric_auth").
    pub capability: String,

    /// What action to perform (e.g. "capture", "authenticate").
    pub action: String,

    /// Action-specific parameters.
    pub payload: serde_json::Value,
}

/// The result of a capability invocation.
/// Delivered scoped to the requesting page via the session backbone.
#[derive(Debug, Clone)]
pub struct CapabilityResponse {
    /// Matches the request's `id` for correlation.
    pub id: String,

    /// Which page the response is for (must match the request's
    /// `page_identity`).
    pub page_identity: PageIdentity,

    /// `Ok(value)` on success, `Err(message)` on failure.
    pub status: Result<serde_json::Value, String>,
}

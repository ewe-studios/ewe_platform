//! What a browser client accepts in a bootstrap payload (spec 83 feature 05,
//! section 6.4).
//!
//! WHY: a browser cannot change the certificate store, cannot pin a server
//! certificate through `fetch`, and a page has no client certificate. A payload
//! that names one of those cases stops the page with a clear configuration
//! error before any call, not with a Transport Layer Security (TLS) error.
//!
//! WHAT: [`check_browser_support`] and [`check_browser_payload`].
//!
//! HOW: the rules are a pure check over the checked payload. `decode` has
//! already limited a plaintext URL to a loopback host, so the check of the
//! trust mode only refuses a pinned certificate. The host must accept a signed
//! access secret; mutual TLS (mTLS) alone is not reachable from a page.

use crate::core::bootstrap::{Bootstrap, TlsTrust};
use crate::error::{AgentRpcError, AgentRpcResult};

/// The auth mode string of the signed-secret provider.
pub const SIGNED_SECRET_MODE: &str = "signed-secret";

/// Whether a page can use this trust mode and these auth modes.
///
/// # Errors
/// [`AgentRpcError::Config`] for a pinned certificate (a browser cannot pin
/// one) or a host that does not accept a signed access secret.
pub fn check_browser_support(trust: TlsTrust, auth_modes: &[String]) -> AgentRpcResult<()> {
    if matches!(trust, TlsTrust::PinnedSha256(_)) {
        return Err(AgentRpcError::Config(
            "this host has a certificate that only a pin trusts; a browser cannot open TLS to it; \
             serve the host with a public certificate or behind the proxy"
                .to_string(),
        )
        .into());
    }
    if !auth_modes.iter().any(|mode| mode == SIGNED_SECRET_MODE) {
        return Err(AgentRpcError::Config(
            "the host does not accept access secrets; a browser needs the signed-secret mode"
                .to_string(),
        )
        .into());
    }
    Ok(())
}

/// [`check_browser_support`] over a checked payload.
///
/// # Errors
/// As [`check_browser_support`].
pub fn check_browser_payload(payload: &Bootstrap) -> AgentRpcResult<()> {
    check_browser_support(payload.trust, &payload.capabilities.auth_modes)
}

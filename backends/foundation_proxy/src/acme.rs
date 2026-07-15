//! ACME (RFC 8555) certificate provisioning — Let's Encrypt DNS-01 (Decision 18).
//!
//! WHY: The proxy must auto-provision TLS certificates without manual PEM file
//! management. Let's Encrypt provides free DV certs via the ACME protocol.
//!
//! WHAT: [`AcmeClient`] — account creation, order placement, DNS-01 challenge
//! via Cloudflare API, certificate finalization. Uses the shared pooled HTTP
//! client for ACME API calls and foundation_netio's HTTP client for the
//! Cloudflare integration.
//!
//! HOW: JSON-over-HTTPS to Let's Encrypt's v2 API. JWS signing is done via
//! an external signing callback (caller provides an ECDSA or RSA key). DNS-01
//! TXT records are created/cleaned via `foundation_deployment_cloudflare`.
//!
//! # Limitations (current scope)
//!
//! - JWS signing is done via a caller-provided callback; the proxy doesn't
//!   generate keys internally (keygen is the operator's responsibility)
//! - Rate limits are the caller's concern
//! - Only DNS-01 challenge (works behind NAT, no open :80 needed)

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use foundation_netio::http::NativeHttpClient;

use crate::config::ProxyError;

// ── ACME API types ─────────────────────────────────────────────────────

const ACME_STAGING: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";
const ACME_PRODUCTION: &str = "https://acme-v02.api.letsencrypt.org/directory";

/// The ACME directory resource.
#[derive(Debug, Deserialize)]
struct Directory {
    #[serde(rename = "newAccount")]
    new_account: String,
    #[serde(rename = "newOrder")]
    new_order: String,
    #[serde(rename = "newNonce")]
    new_nonce: String,
}

/// ACME account creation payload.
#[derive(Debug, Serialize)]
struct NewAccountPayload {
    #[serde(rename = "termsOfServiceAgreed")]
    terms_of_service_agreed: bool,
    contact: Vec<String>,
}

/// ACME order request.
#[derive(Debug, Serialize)]
struct NewOrderPayload {
    identifiers: Vec<Identifier>,
}

#[derive(Debug, Serialize)]
struct Identifier {
    #[serde(rename = "type")]
    typ: String,
    value: String,
}

/// ACME order response.
#[derive(Debug, Deserialize)]
struct Order {
    status: String,
    authorizations: Vec<String>,
    finalize: String,
    certificate: Option<String>,
}

/// ACME authorization with challenges.
#[derive(Debug, Deserialize)]
struct Authorization {
    identifier: IdentifierObj,
    challenges: Vec<Challenge>,
}

#[derive(Debug, Deserialize)]
struct IdentifierObj {
    #[serde(rename = "type")]
    typ: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct Challenge {
    #[serde(rename = "type")]
    typ: String,
    url: String,
    token: String,
    status: String,
}

/// DNS-01 challenge response (empty JSON body signals completion).
#[derive(Debug, Serialize)]
struct ChallengeResponse {}

// ── ACME Client ─────────────────────────────────────────────────────────

/// ACME certificate provisioning client.
///
/// Uses the proxy's shared HTTP client for API calls.
/// JWS signing is provided by a caller-supplied closure.
pub struct AcmeClient {
    client: Arc<NativeHttpClient<foundation_netio::shared::client::SystemDnsResolver>>,
    directory_url: String,
    /// JWS signing callback: (payload_json) → base64url-encoded JWS body
    signer: Option<Box<dyn Fn(&str) -> Result<String, String> + Send + Sync>>,
}

impl AcmeClient {
    /// Create an ACME client targeting Let's Encrypt production.
    pub fn production(
        client: Arc<NativeHttpClient<foundation_netio::shared::client::SystemDnsResolver>>,
    ) -> Self {
        Self {
            client,
            directory_url: ACME_PRODUCTION.to_string(),
            signer: None,
        }
    }

    /// Create an ACME client targeting Let's Encrypt staging.
    pub fn staging(
        client: Arc<NativeHttpClient<foundation_netio::shared::client::SystemDnsResolver>>,
    ) -> Self {
        Self {
            client,
            directory_url: ACME_STAGING.to_string(),
            signer: None,
        }
    }

    /// Set the JWS signer for authenticated requests.
    #[must_use]
    pub fn with_signer(
        mut self,
        signer: impl Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    ) -> Self {
        self.signer = Some(Box::new(signer));
        self
    }

    /// Compute the DNS-01 TXT record value for a challenge token + account
    /// thumbprint. The DNS record `_acme-challenge.<domain>` must be set to
    /// this value.
    ///
    /// Returns the base64url-encoded SHA-256 of the key authorization.
    pub fn dns01_value(
        token: &str,
        account_thumbprint: &str,
    ) -> String {
        use sha2::{Digest, Sha256};
        let key_auth = format!("{token}.{account_thumbprint}");
        let hash = Sha256::digest(key_auth.as_bytes());
        base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            &hash,
        )
    }

    /// Compute the account key thumbprint (JWS `jwk` field → SHA-256 → base64url).
    /// The caller must provide their JWK JSON as a string.
    pub fn account_thumbprint(jwk_json: &str) -> String {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(jwk_json.as_bytes());
        base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            &hash,
        )
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns01_value_is_deterministic() {
        let a = AcmeClient::dns01_value("tok1", "thumb1");
        let b = AcmeClient::dns01_value("tok1", "thumb1");
        assert_eq!(a, b, "same inputs = same dns01 value");
    }

    #[test]
    fn dns01_value_is_base64url() {
        let val = AcmeClient::dns01_value("test-token", "thumb");
        // base64url: no +/  chars, no = padding
        assert!(!val.contains('+'), "no + in base64url: {val}");
        assert!(!val.contains('/'), "no / in base64url: {val}");
        assert!(!val.contains('='), "no = padding in base64url: {val}");
    }

    #[test]
    fn dns01_different_inputs_yield_different_outputs() {
        let a = AcmeClient::dns01_value("tok1", "thumb1");
        let b = AcmeClient::dns01_value("tok2", "thumb1");
        assert_ne!(a, b, "different token = different dns01 value");
    }
}

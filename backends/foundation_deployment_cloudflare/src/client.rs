//! CloudflareClient — wraps the auto-generated async `*_request` functions
//! with auth injection and typed domain structs.

use foundation_netio::{DynNetClient, HttpClientBuilder};
use crate::types::*;

/// Cloudflare's API base URL, from the spec's `servers[]`.
///
/// The generated `*_request` functions take this per call rather than hardcoding
/// it, so a test can point the client at a mock (spec-54's configurable base_url).
pub const DEFAULT_BASE_URL: &str = "https://api.cloudflare.com/client/v4";

/// Central client for Cloudflare API operations.
pub struct CloudflareClient {
    http: DynNetClient,
    token: String,
    zone_id: String,
    domain: String,
    base_url: String,
}

impl CloudflareClient {
    /// Create from env: CLOUDFLARE_API_TOKEN + CLOUDFLARE_ZONE_ID.
    /// Optional: CLOUDFLARE_DOMAIN for bootstrap_domain().
    pub fn from_env() -> Result<Self, CloudflareError> {
        let token = std::env::var("CLOUDFLARE_API_TOKEN")
            .map_err(|_| CloudflareError::Auth("CLOUDFLARE_API_TOKEN not set".into()))?;
        let zone_id = std::env::var("CLOUDFLARE_ZONE_ID")
            .map_err(|_| CloudflareError::Auth("CLOUDFLARE_ZONE_ID not set".into()))?;
        let domain = std::env::var("CLOUDFLARE_DOMAIN").unwrap_or_default();
        Ok(Self {
            http: HttpClientBuilder::new().build(),
            token,
            zone_id,
            domain,
            base_url: DEFAULT_BASE_URL.to_string(),
        })
    }

    /// Create with explicit credentials.
    pub fn new(token: String, zone_id: String) -> Self {
        Self {
            http: HttpClientBuilder::new().build(),
            token,
            zone_id,
            domain: String::new(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    /// Create with an externally-provided client (for testing or custom config).
    pub fn with_client(http: DynNetClient, token: String, zone_id: String) -> Self {
        Self {
            http,
            token,
            zone_id,
            domain: String::new(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    /// Set the domain for bootstrap operations.
    #[must_use]
    pub fn with_domain(mut self, domain: String) -> Self {
        self.domain = domain;
        self
    }

    #[must_use] pub fn zone_id(&self) -> &str { &self.zone_id }
    /// The cross-platform HTTP client handle (cheap `Arc` clone).
    #[must_use] pub fn http(&self) -> DynNetClient { self.http.clone() }
    #[must_use] pub fn token(&self) -> String { self.token.clone() }
    #[must_use] pub fn domain(&self) -> &str { &self.domain }
    /// The API base URL the generated calls are pointed at.
    #[must_use] pub fn base_url(&self) -> &str { &self.base_url }

    /// Point the client at a different base URL — a mock server, in practice.
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }
}

//! CloudflareClient — wraps the auto-generated valtron TaskIterator functions
//! with auth injection and typed domain structs.

use foundation_netio::http::SimpleHttpClient;
use crate::types::*;

/// Central client for Cloudflare API operations.
pub struct CloudflareClient {
    http: SimpleHttpClient,
    token: String,
    zone_id: String,
    domain: String,
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
        Ok(Self { http: SimpleHttpClient::from_system(), token, zone_id, domain })
    }

    /// Create with explicit credentials.
    pub fn new(token: String, zone_id: String) -> Self {
        Self { http: SimpleHttpClient::from_system(), token, zone_id, domain: String::new() }
    }

    /// Set the domain for bootstrap operations.
    #[must_use]
    pub fn with_domain(mut self, domain: String) -> Self {
        self.domain = domain;
        self
    }

    #[must_use] pub fn zone_id(&self) -> &str { &self.zone_id }
    #[must_use] pub fn http(&self) -> &SimpleHttpClient { &self.http }
    #[must_use] pub fn token(&self) -> String { self.token.clone() }
    #[must_use] pub fn domain(&self) -> &str { &self.domain }
}

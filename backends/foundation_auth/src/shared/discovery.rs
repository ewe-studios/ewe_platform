//! OIDC Discovery Client.
//!
//! Fetches and caches the OIDC discovery document from
//! `/.well-known/openid-configuration`. Auto-configures `OAuthConfig` from the
//! discovery response.
//!
//! ## Caching Strategy
//! The discovery document is static configuration metadata. It is cached
//! in-memory for the lifetime of the `DiscoveryClient`. No TTL, no disk
//! persistence, no foundation_db involvement. Use `refresh()` to force
//! re-fetch or `clear_cache()` to drop the cached entry.

use serde::{Deserialize, Serialize};

use super::oauth::OAuthConfig;

/// OIDC discovery document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcDiscovery {
    /// Issuer identifier.
    pub issuer: String,
    /// Authorization endpoint URL.
    pub authorization_endpoint: String,
    /// Token endpoint URL.
    pub token_endpoint: String,
    /// UserInfo endpoint URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userinfo_endpoint: Option<String>,
    /// JWKS URI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jwks_uri: Option<String>,
    /// Dynamic client registration endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration_endpoint: Option<String>,
    /// Supported scopes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scopes_supported: Option<Vec<String>>,
    /// Supported response types.
    pub response_types_supported: Vec<String>,
    /// Supported response modes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_modes_supported: Option<Vec<String>>,
    /// Supported grant types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant_types_supported: Option<Vec<String>>,
    /// Supported ACR values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acr_values_supported: Option<Vec<String>>,
    /// Supported subject types.
    pub subject_types_supported: Vec<String>,
    /// Supported ID token signing algorithms.
    pub id_token_signing_alg_values_supported: Vec<String>,
    /// Supported ID token encryption algorithms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token_encryption_alg_values_supported: Option<Vec<String>>,
    /// Supported UserInfo signing algorithms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userinfo_signing_alg_values_supported: Option<Vec<String>>,
    /// Supported token endpoint auth methods.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_endpoint_auth_methods_supported: Option<Vec<String>>,
    /// Supported token endpoint auth signing algorithms.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_endpoint_auth_signing_alg_values_supported: Option<Vec<String>>,
    /// Supported display values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_values_supported: Option<Vec<String>>,
    /// Supported claim types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_types_supported: Option<Vec<String>>,
    /// Supported claims.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claims_supported: Option<Vec<String>>,
    /// Service documentation URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_documentation: Option<String>,
    /// Supported claim locales.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claims_locales_supported: Option<Vec<String>>,
    /// Supported UI locales.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_locales_supported: Option<Vec<String>>,
    /// Introspection endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub introspection_endpoint: Option<String>,
    /// Revocation endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revocation_endpoint: Option<String>,
    /// Device authorization endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_authorization_endpoint: Option<String>,
    /// Supported code challenge methods (PKCE).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_challenge_methods_supported: Option<Vec<String>>,
}

impl OidcDiscovery {
    /// Build an OAuthConfig from this discovery document.
    #[must_use]
    pub fn to_oauth_config(&self, client_id: &str, redirect_uri: &str) -> OAuthConfig {
        let mut builder = OAuthConfig::builder()
            .client_id(client_id)
            .authorization_url(&self.authorization_endpoint)
            .token_url(&self.token_endpoint)
            .redirect_uri(redirect_uri)
            .response_type(
                self.response_types_supported
                    .first()
                    .cloned()
                    .unwrap_or_else(|| String::from("code")),
            );

        // Scopes — prefer openid, profile, email if available
        if let Some(ref supported) = self.scopes_supported {
            let defaults = ["openid", "profile", "email"];
            let scopes: Vec<String> = defaults
                .iter()
                .filter(|s| supported.iter().any(|x| x == *s))
                .map(|s| s.to_string())
                .collect();
            if !scopes.is_empty() {
                builder = builder.scopes(scopes);
            } else if !supported.is_empty() {
                builder = builder.scopes(supported.clone());
            }
        }

        // PKCE enabled if S256 is supported
        if let Some(ref methods) = self.code_challenge_methods_supported {
            builder = builder.pkce_enabled(methods.iter().any(|m| m == "S256"));
        } else {
            builder = builder.pkce_enabled(false);
        }

        builder.build()
    }

    /// Get the JWKS URL, preferring jwks_uri over issuer + "/jwks".
    #[must_use]
    pub fn jwks_url(&self) -> String {
        self.jwks_uri
            .clone()
            .unwrap_or_else(|| format!("{}/jwks", self.issuer.trim_end_matches('/')))
    }
}

/// Discovery client — fetches and caches OIDC discovery documents.
///
/// The discovery document is configuration metadata that rarely changes.
/// It is cached in memory for the lifetime of the client.
pub struct DiscoveryClient {
    cached: Option<(String, OidcDiscovery)>, // (issuer_url, discovery)
}

impl Default for DiscoveryClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoveryClient {
    /// Create a new discovery client with an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self { cached: None }
    }

    /// Get the cached discovery document, fetching if not cached.
    ///
    /// # Errors
    ///
    /// Returns `DiscoveryError` if the fetch or parse fails.
    pub async fn get(&mut self, issuer_url: &str) -> Result<&OidcDiscovery, DiscoveryError> {
        if let Some((cached_url, _)) = &self.cached {
            if cached_url == issuer_url {
                return Ok(&self.cached.as_ref().unwrap().1);
            }
        }
        self.fetch(issuer_url).await
    }

    /// Fetch discovery document from the issuer URL, replacing cache.
    ///
    /// # Errors
    ///
    /// Returns `DiscoveryError` if the fetch or parse fails.
    pub async fn fetch(&mut self, issuer_url: &str) -> Result<&OidcDiscovery, DiscoveryError> {
        let url = build_well_known_url(issuer_url);
        let discovery = do_fetch(&url).await?;
        self.cached = Some((issuer_url.to_string(), discovery));
        Ok(&self.cached.as_ref().unwrap().1)
    }

    /// Fetch discovery document from an explicit URL (bypasses cache, no caching).
    ///
    /// # Errors
    ///
    /// Returns `DiscoveryError` if the fetch or parse fails.
    pub async fn fetch_from_url(&self, url: &str) -> Result<OidcDiscovery, DiscoveryError> {
        do_fetch(url).await
    }

    /// Clear the cached discovery document.
    pub fn clear_cache(&mut self) {
        self.cached = None;
    }

    /// Returns true if a discovery document is cached for this issuer.
    #[must_use]
    pub fn is_cached(&self, issuer_url: &str) -> bool {
        self.cached
            .as_ref()
            .is_some_and(|(url, _)| url == issuer_url)
    }
}

/// Build the well-known URL from an issuer URL.
fn build_well_known_url(issuer: &str) -> String {
    let base = issuer.trim_end_matches('/');
    if base.ends_with("/.well-known/openid-configuration") {
        return base.to_string();
    }
    format!("{base}/.well-known/openid-configuration")
}

/// Fetch and parse a discovery document from a URL.
async fn do_fetch(url: &str) -> Result<OidcDiscovery, DiscoveryError> {
    let json = fetch_discovery(url).await?;
    serde_json::from_str(&json)
        .map_err(|e| DiscoveryError::ParseError(e.to_string()))
}

/// Validate required fields.
#[allow(dead_code)]
fn validate_discovery(d: &OidcDiscovery) -> Result<(), DiscoveryError> {
    if d.issuer.is_empty() {
        return Err(DiscoveryError::MissingRequiredField(
            "issuer is required".into(),
        ));
    }
    if d.authorization_endpoint.is_empty() {
        return Err(DiscoveryError::MissingRequiredField(
            "authorization_endpoint is required".into(),
        ));
    }
    if d.token_endpoint.is_empty() {
        return Err(DiscoveryError::MissingRequiredField(
            "token_endpoint is required".into(),
        ));
    }
    Ok(())
}

/// Discovery-related errors.
#[derive(Debug)]
pub enum DiscoveryError {
    /// Issuer URL is invalid.
    InvalidIssuerUrl(String),
    /// HTTP fetch failed.
    FetchFailed(String),
    /// JSON parse failed.
    ParseError(String),
    /// A required field is missing.
    MissingRequiredField(String),
}

impl core::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DiscoveryError::InvalidIssuerUrl(s) => write!(f, "Invalid issuer URL: {s}"),
            DiscoveryError::FetchFailed(s) => write!(f, "Discovery fetch failed: {s}"),
            DiscoveryError::ParseError(s) => write!(f, "Discovery parse error: {s}"),
            DiscoveryError::MissingRequiredField(s) => {
                write!(f, "Discovery missing required field: {s}")
            }
        }
    }
}

impl std::error::Error for DiscoveryError {}

// ===========================================================================
// HTTP fetch — uses the cross-platform HttpClient trait (native + wasm).
// ===========================================================================

async fn fetch_discovery(url: &str) -> Result<String, DiscoveryError> {
    use foundation_core::url::Uri;
    use foundation_netio::simple_http::client::default_http_client;
    use foundation_netio::simple_http::client::shared::request::PreparedRequest;
    use foundation_netio::simple_http::shared::{
        SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod,
    };

    let client = default_http_client();
    let uri = Uri::parse(url)
        .map_err(|e| DiscoveryError::FetchFailed(format!("invalid URL: {e}")))?;
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::ACCEPT, vec!["application/json".into()]);

    let req = PreparedRequest {
        method: SimpleMethod::GET,
        url: uri,
        headers,
        body: SendSafeBody::None,
        extensions: Default::default(),
    };

    let resp = client
        .send_async(req)
        .await
        .map_err(|e| DiscoveryError::FetchFailed(e.to_string()))?;

    let status: usize = resp.get_status().into();
    if !(200..300).contains(&status) {
        let body = match resp.get_body_ref() {
            SendSafeBody::Text(t) => t.clone(),
            SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
            _ => String::new(),
        };
        return Err(DiscoveryError::FetchFailed(format!(
            "HTTP {status}: {body}"
        )));
    }

    match resp.get_body_ref() {
        SendSafeBody::Text(t) => Ok(t.clone()),
        SendSafeBody::Bytes(b) => String::from_utf8(b.clone())
            .map_err(|e| DiscoveryError::FetchFailed(e.to_string())),
        _ => Ok(String::new()),
    }
}

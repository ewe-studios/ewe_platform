//! Platform-agnostic upstream OIDC/OAuth2 client (spec-57, F003).
//!
//! WHY: Social login needs a single client that can authenticate with any
//! OIDC/OAuth2 upstream provider — discover endpoints from an issuer URL OR
//! use provider-configured explicit URLs, build authorize URLs with PKCE,
//! exchange codes for tokens, and fetch the user profile from userinfo.
//!
//! WHAT: [`UpstreamOidcClient`] wraps [`super::discovery::DiscoveryClient`]
//! for OIDC providers and [`super::oauth::OAuthManager`] for authorization URL
//! generation. It is fully cross-platform: discovery, token exchange, and
//! userinfo all run over the platform network client, [`DynNetClient`]
//! (built with [`HttpClientBuilder`], or given to
//! [`UpstreamOidcClient::with_client`]), which is the native HTTP stack on
//! native targets and browser/Worker `fetch` on wasm32 — so the whole broker
//! flow executes wherever that client exists (including a Cloudflare Worker),
//! with no platform-gated branching in this module.
//!
//! HOW:
//! 1. `discover_and_configure()` — fetches discovery doc, builds `OAuthConfig`
//! 2. `authorize_url(state) -> (url, pkce)` — ready for redirect
//! 3. `exchange_code()` — exchanges the auth code for tokens at the token endpoint
//! 4. `fetch_userinfo()` — fetches the standardized profile from userinfo

use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::{SendSafeBody, SimpleHeader, SimpleResponse};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};

use super::discovery::{DiscoveryClient, DiscoveryError, OidcDiscovery};
use super::oauth::{OAuthConfig, OAuthError, OAuthManager, PkceChallenge, TokenResponse};
use super::oauth_token::OAuthToken;
use super::provider::{ProviderType, UpstreamProvider};

/// Errors from upstream authentication flows.
#[derive(Debug)]
pub enum UpstreamClientError {
    /// Discovery phase failed.
    Discovery(DiscoveryError),
    /// OAuth configuration or authorization failed.
    OAuth(OAuthError),
    /// Token exchange failed (platform-specific).
    TokenExchange(String),
    /// Userinfo fetch failed.
    Userinfo(String),
    /// Provider is missing endpoints and has no discovery URL.
    NoEndpoints,
    /// Invalid provider configuration.
    InvalidConfig(String),
}

impl core::fmt::Display for UpstreamClientError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Discovery(e) => write!(f, "discovery error: {e}"),
            Self::OAuth(e) => write!(f, "oauth error: {e}"),
            Self::TokenExchange(s) => write!(f, "token exchange error: {s}"),
            Self::Userinfo(s) => write!(f, "userinfo error: {s}"),
            Self::NoEndpoints => write!(f, "provider has no discovery URL and no explicit endpoints"),
            Self::InvalidConfig(s) => write!(f, "invalid provider config: {s}"),
        }
    }
}

impl std::error::Error for UpstreamClientError {}

impl From<DiscoveryError> for UpstreamClientError {
    fn from(e: DiscoveryError) -> Self {
        Self::Discovery(e)
    }
}

impl From<OAuthError> for UpstreamClientError {
    fn from(e: OAuthError) -> Self {
        Self::OAuth(e)
    }
}

/// OIDC / OAuth2 upstream authentication client.
///
/// Works for any provider that supports the standard OIDC/OAuth2 protocol.
/// The client is configured from an [`UpstreamProvider`] — it either discovers
/// endpoints from a `.well-known` URL or uses the explicit authorize/token/userinfo
/// URLs configured on the provider.
pub struct UpstreamOidcClient {
    /// HTTP client for the token and userinfo requests.
    http: DynNetClient,
    /// The provider configuration.
    provider: UpstreamProvider,
    /// OAuth manager for authorization URL generation.
    oauth: OAuthManager,
    /// Cached discovery document (None for non-OIDC providers).
    discovery: Option<OidcDiscovery>,
    /// Redirect URI for the authorization code flow.
    redirect_uri: String,
}

impl UpstreamOidcClient {
    /// Create a new client for the given provider.
    ///
    /// Use [`Self::configure`] or [`Self::discover_and_configure`] to set up
    /// the OAuth configuration before generating URLs or exchanging codes.
    /// The token and userinfo requests use the platform's default HTTP client
    /// ([`HttpClientBuilder`]); [`Self::with_client`] takes another one.
    #[must_use]
    pub fn new(provider: UpstreamProvider, redirect_uri: String) -> Self {
        Self::with_client(HttpClientBuilder::new().build(), provider, redirect_uri)
    }

    /// Create a new client for the given provider that sends its token and
    /// userinfo requests through `http` (a shared client, or a test client).
    /// Discovery runs through the [`DiscoveryClient`] given to
    /// [`Self::discover_and_configure`].
    #[must_use]
    pub fn with_client(
        http: DynNetClient,
        provider: UpstreamProvider,
        redirect_uri: String,
    ) -> Self {
        Self {
            http,
            provider,
            oauth: OAuthManager::new(OAuthConfig::default()),
            discovery: None,
            redirect_uri,
        }
    }

    /// Build from a provider using explicit endpoints (no discovery).
    ///
    /// # Errors
    ///
    /// Returns `UpstreamClientError::NoEndpoints` if the provider lacks both
    /// `authorization_url` and `token_url`.
    pub fn configure(&mut self) -> Result<(), UpstreamClientError> {
        let provider = &self.provider;
        let auth_url = provider
            .authorization_url
            .as_deref()
            .or_else(|| {
                self.discovery
                    .as_ref()
                    .map(|d| d.authorization_endpoint.as_str())
            })
            .ok_or(UpstreamClientError::NoEndpoints)?;

        let token_url = provider
            .token_url
            .as_deref()
            .or_else(|| {
                self.discovery
                    .as_ref()
                    .map(|d| d.token_endpoint.as_str())
            })
            .ok_or(UpstreamClientError::NoEndpoints)?;

        let mut builder = OAuthConfig::builder()
            .client_id(&provider.client_id)
            .authorization_url(auth_url)
            .token_url(token_url)
            .redirect_uri(&self.redirect_uri)
            .scopes(provider.scopes.clone());

        // PKCE is enabled by default for public clients.
        builder = builder.pkce_enabled(true);

        // For OIDC providers, add nonce support
        if provider.provider_type == ProviderType::Oidc {
            builder = builder.nonce(OAuthManager::generate_nonce());
        }

        self.oauth = OAuthManager::new(builder.build());
        Ok(())
    }

    /// Fetch OIDC discovery document and configure from it.
    ///
    /// Uses the provider's `discovery_url`. After discovery, the endpoints
    /// are taken from the discovery document, with explicit provider URLs
    /// acting as overrides.
    ///
    /// # Errors
    ///
    /// Returns `UpstreamClientError::Discovery` on fetch/parse failure.
    pub async fn discover_and_configure(
        &mut self,
        client: &mut DiscoveryClient,
    ) -> Result<(), UpstreamClientError> {
        let discovery_url = self
            .provider
            .discovery_url
            .as_deref()
            .ok_or(UpstreamClientError::NoEndpoints)?;

        let discovery = client.fetch(discovery_url).await?;
        self.discovery = Some(discovery.clone());
        self.configure()
    }

    /// Build the authorization URL with state and optional PKCE challenge.
    ///
    /// Returns the URL to redirect the user to, plus the PKCE challenge
    /// (code verifier must be stored and sent to `exchange_code`).
    ///
    /// # Errors
    ///
    /// Returns `UpstreamClientError::OAuth` if the config is invalid.
    pub fn authorize_url(
        &self,
        state: &str,
    ) -> Result<(String, PkceChallenge), UpstreamClientError> {
        // Ensure we have a valid config
        self.oauth
            .config()
            .validate()
            .map_err(UpstreamClientError::OAuth)?;

        let (url, pkce) = self.oauth.get_authorization_url(state)?;
        let challenge = pkce.ok_or(UpstreamClientError::OAuth(OAuthError::PkceFailed))?;
        Ok((url, challenge))
    }

    /// Build an authorization URL without PKCE (for providers that don't support it).
    ///
    /// # Errors
    ///
    /// Returns `UpstreamClientError::OAuth` if the config is invalid.
    pub fn authorize_url_no_pkce(&self, state: &str) -> Result<String, UpstreamClientError> {
        self.oauth
            .config()
            .validate()
            .map_err(UpstreamClientError::OAuth)?;

        // Temporarily disable PKCE for this call
        let mut cfg = self.oauth.config().clone();
        cfg.pkce_enabled = false;
        let tmp_oauth = OAuthManager::new(cfg);
        let (url, _) = tmp_oauth.get_authorization_url(state)?;
        Ok(url)
    }

    /// Get a reference to the OAuth manager.
    #[must_use]
    pub fn oauth_manager(&self) -> &OAuthManager {
        &self.oauth
    }

    /// Get the provider configuration.
    #[must_use]
    pub fn provider(&self) -> &UpstreamProvider {
        &self.provider
    }

    /// Get the redirect URI.
    #[must_use]
    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    /// Get the cached discovery document.
    #[must_use]
    pub fn discovery(&self) -> Option<&OidcDiscovery> {
        self.discovery.as_ref()
    }

    /// Get the userinfo endpoint, preferring discovery document value.
    #[must_use]
    pub fn userinfo_endpoint(&self) -> Option<String> {
        self.provider.userinfo_url.clone().or_else(|| {
            self.discovery
                .as_ref()
                .and_then(|d| d.userinfo_endpoint.clone())
        })
    }

    /// Exchange an authorization code for tokens at the provider's token endpoint.
    ///
    /// Runs the OAuth2 `authorization_code` grant over the shared
    /// client ([`DynNetClient`]), so it works identically on native and wasm.
    /// The `client_secret` (decrypted by the caller from
    /// [`UpstreamProvider::client_secret_ciphertext`]) is sent only when
    /// provided — public PKCE clients pass `None`. Any secret already present
    /// on the configured [`OAuthConfig`] is used as a fallback.
    ///
    /// # Errors
    ///
    /// Returns [`UpstreamClientError::OAuth`] if the client is not configured,
    /// or [`UpstreamClientError::TokenExchange`] if the request fails or the
    /// token response cannot be parsed.
    pub async fn exchange_code(
        &self,
        code: &str,
        code_verifier: Option<&str>,
        client_secret: Option<&str>,
    ) -> Result<OAuthToken, UpstreamClientError> {
        let config = self.oauth.config();
        config.validate().map_err(UpstreamClientError::OAuth)?;

        let mut form = vec![
            ("grant_type".to_string(), "authorization_code".to_string()),
            ("code".to_string(), code.to_string()),
            ("redirect_uri".to_string(), config.redirect_uri.clone()),
            ("client_id".to_string(), config.client_id.clone()),
        ];

        if let Some(secret) = client_secret.or(config.client_secret.as_deref()) {
            form.push(("client_secret".to_string(), secret.to_string()));
        }
        if let Some(verifier) = code_verifier {
            form.push(("code_verifier".to_string(), verifier.to_string()));
        }

        let body = encode_form(&form);
        let response = http_post_form(&self.http, &config.token_url, &body).await?;
        let token: TokenResponse = serde_json::from_str(&response)
            .map_err(|e| UpstreamClientError::TokenExchange(format!("parse token response: {e}")))?;

        Ok(OAuthToken {
            access_token: token.access_token,
            token_type: token.token_type,
            expires_in: token.expires_in,
            refresh_token: token.refresh_token,
            scope: token.scope,
            id_token: token.id_token,
        })
    }

    /// Fetch the standardized user profile from the provider's userinfo endpoint.
    ///
    /// GETs the userinfo endpoint with a Bearer access token over the shared
    /// client ([`DynNetClient`]), then maps the raw claims to an
    /// [`UpstreamProfile`] using the provider's `mapping_config`.
    ///
    /// # Errors
    ///
    /// Returns [`UpstreamClientError::NoEndpoints`] if the provider exposes no
    /// userinfo endpoint, or [`UpstreamClientError::Userinfo`] if the request
    /// fails, the body is not valid JSON, or required claims are missing.
    pub async fn fetch_userinfo(
        &self,
        access_token: &str,
    ) -> Result<UpstreamProfile, UpstreamClientError> {
        let url = self
            .userinfo_endpoint()
            .ok_or(UpstreamClientError::NoEndpoints)?;

        let response = http_get_with_bearer(&self.http, &url, access_token).await?;
        let json: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| UpstreamClientError::Userinfo(format!("parse userinfo response: {e}")))?;

        UpstreamProfile::from_userinfo(&json, &self.provider.mapping_config).ok_or_else(|| {
            UpstreamClientError::Userinfo(
                "userinfo response missing the mapped subject claim".to_string(),
            )
        })
    }
}

/// A standardized upstream user profile returned from userinfo.
///
/// All providers map their claim names to these normalized fields via
/// the provider's `mapping_config`.
#[derive(Debug, Clone)]
pub struct UpstreamProfile {
    /// The stable subject identifier from the upstream provider.
    pub sub: String,
    /// Email address.
    pub email: Option<String>,
    /// Whether the email has been verified.
    pub email_verified: bool,
    /// Display name.
    pub name: Option<String>,
    /// Preferred username.
    pub preferred_username: Option<String>,
    /// Raw claims from the provider (for debugging / custom claims).
    pub raw: serde_json::Value,
}

impl UpstreamProfile {
    /// Extract a profile from a userinfo JSON response using the provider's mapping.
    pub fn from_userinfo(
        json: &serde_json::Value,
        mapping: &super::provider::ProviderMapping,
    ) -> Option<Self> {
        Some(Self {
            sub: json.get(&mapping.subject_field)?.as_str()?.to_string(),
            email: json
                .get(&mapping.email_field)
                .and_then(|v| v.as_str())
                .map(String::from),
            email_verified: json
                .get(&mapping.email_verified_field)
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            name: json
                .get(&mapping.name_field)
                .and_then(|v| v.as_str())
                .map(String::from),
            preferred_username: mapping
                .username_field
                .as_ref()
                .and_then(|f| json.get(f))
                .and_then(|v| v.as_str())
                .map(String::from),
            raw: json.clone(),
        })
    }

    /// Extract a profile from an OIDC ID token claims set.
    pub fn from_id_token(
        claims: &serde_json::Value,
        mapping: &super::provider::ProviderMapping,
    ) -> Option<Self> {
        Self::from_userinfo(claims, mapping)
    }
}

// ===========================================================================
// HTTP — cross-platform via the platform network client (`DynNetClient`):
// native HTTP stack on native, browser/Worker fetch
// on wasm32. Mirrors `discovery.rs` / `userinfo.rs`.
// ===========================================================================

/// URL-encode a set of form fields into an `application/x-www-form-urlencoded` body.
fn encode_form(fields: &[(String, String)]) -> String {
    fields
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// POST a form-urlencoded body and return the response text.
async fn http_post_form(
    client: &DynNetClient,
    url: &str,
    body: &str,
) -> Result<String, UpstreamClientError> {
    let req = PreparedRequestBuilder::post(url)
        .map_err(|e| UpstreamClientError::TokenExchange(format!("invalid token URL: {e}")))?
        .header(SimpleHeader::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(SimpleHeader::ACCEPT, "application/json")
        .body_text(body)
        .build();

    let resp = client
        .send_async(req)
        .await
        .map_err(|e| UpstreamClientError::TokenExchange(e.to_string()))?;

    let status: usize = resp.get_status().into();
    let text = collect_body_text(resp)
        .map_err(|e| UpstreamClientError::TokenExchange(format!("read token response: {e}")))?;
    if !(200..300).contains(&status) {
        return Err(UpstreamClientError::TokenExchange(format!(
            "HTTP {status}: {text}"
        )));
    }
    Ok(text)
}

/// GET a URL with a Bearer access token and return the response text.
async fn http_get_with_bearer(
    client: &DynNetClient,
    url: &str,
    access_token: &str,
) -> Result<String, UpstreamClientError> {
    let req = PreparedRequestBuilder::get(url)
        .map_err(|e| UpstreamClientError::Userinfo(format!("invalid userinfo URL: {e}")))?
        .header(SimpleHeader::ACCEPT, "application/json")
        .bearer_token(access_token)
        .build();

    let resp = client
        .send_async(req)
        .await
        .map_err(|e| UpstreamClientError::Userinfo(e.to_string()))?;

    let status: usize = resp.get_status().into();
    let text = collect_body_text(resp)
        .map_err(|e| UpstreamClientError::Userinfo(format!("read userinfo response: {e}")))?;
    if status == 401 {
        return Err(UpstreamClientError::Userinfo(format!("unauthorized: {text}")));
    }
    if !(200..300).contains(&status) {
        return Err(UpstreamClientError::Userinfo(format!("HTTP {status}: {text}")));
    }
    Ok(text)
}

/// Drain a response body to a UTF-8 string (lossy for raw bytes).
///
/// The client returns bodies as a lazy [`SendSafeBody::Stream`], so
/// `get_body_ref()` would observe an empty body — the stream must be collected
/// via [`try_collect_bytes`].
fn collect_body_text(resp: SimpleResponse<SendSafeBody>) -> Result<String, String> {
    let bytes = try_collect_bytes(resp.take_body()).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

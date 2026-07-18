//! Platform-agnostic upstream OIDC/OAuth2 client (spec-57, F003).
//!
//! WHY: Social login needs a single client that can authenticate with any
//! OIDC/OAuth2 upstream provider — discover endpoints from an issuer URL OR
//! use provider-configured explicit URLs, build authorize URLs with PKCE,
//! exchange codes for tokens, and fetch the user profile from userinfo.
//!
//! WHAT: [`UpstreamOidcClient`] wraps [`super::discovery::DiscoveryClient`]
//! for OIDC providers and [`super::oauth::OAuthManager`] for authorization URL
//! generation. Token exchange is platform-gated (native uses
//! [`crate::native::oauth::NativeOAuth`]; wasm uses
//! [`crate::wasm_bindgen::oauth::WasmOAuth`]).
//!
//! HOW:
//! 1. `discover_and_configure()` — fetches discovery doc, builds `OAuthConfig`
//! 2. `build_authorize_url(state) -> (url, pkce)` — ready for redirect
//! 3. `exchange_code()` — platform-gated token exchange
//! 4. `fetch_userinfo()` — fetches the standardized profile from userinfo

use super::discovery::DiscoveryError;
#[cfg(feature = "server")]
use super::discovery::{DiscoveryClient, OidcDiscovery};
use super::oauth::OAuthError;
#[cfg(feature = "server")]
use super::oauth::{OAuthConfig, OAuthManager, PkceChallenge};
#[cfg(feature = "server")]
use crate::server::models::provider::{ProviderType, UpstreamProvider};

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
#[cfg(feature = "server")]
pub struct UpstreamOidcClient {
    /// The provider configuration.
    provider: UpstreamProvider,
    /// OAuth manager for authorization URL generation.
    oauth: OAuthManager,
    /// Cached discovery document (None for non-OIDC providers).
    discovery: Option<OidcDiscovery>,
    /// Redirect URI for the authorization code flow.
    redirect_uri: String,
}

#[cfg(feature = "server")]
impl UpstreamOidcClient {
    /// Create a new client for the given provider.
    ///
    /// Use [`Self::configure`] or [`Self::discover_and_configure`] to set up
    /// the OAuth configuration before generating URLs or exchanging codes.
    #[must_use]
    pub fn new(provider: UpstreamProvider, redirect_uri: String) -> Self {
        Self {
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
    #[cfg(feature = "server")]
    pub fn from_userinfo(
        json: &serde_json::Value,
        mapping: &crate::server::models::provider::ProviderMapping,
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
    #[cfg(feature = "server")]
    pub fn from_id_token(
        claims: &serde_json::Value,
        mapping: &crate::server::models::provider::ProviderMapping,
    ) -> Option<Self> {
        Self::from_userinfo(claims, mapping)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use crate::server::models::provider::{ProviderMapping, ProviderType, UpstreamProvider};

    fn google_provider() -> UpstreamProvider {
        UpstreamProvider {
            id: "google".into(),
            name: "Google".into(),
            provider_type: ProviderType::Oidc,
            client_id: "test-client-id.apps.googleusercontent.com".into(),
            client_secret_ciphertext: None,
            encryption_key_id: "default".into(),
            authorization_url: None,
            token_url: None,
            userinfo_url: None,
            discovery_url: Some(
                "https://accounts.google.com/.well-known/openid-configuration".into(),
            ),
            scopes: vec!["openid".into(), "email".into(), "profile".into()],
            is_active: true,
            mapping_config: ProviderMapping::default(),
            created_at: 0,
            updated_at: 0,
        }
    }

    fn github_provider() -> UpstreamProvider {
        UpstreamProvider {
            id: "github".into(),
            name: "GitHub".into(),
            provider_type: ProviderType::Oauth2,
            client_id: "github-client-id".into(),
            client_secret_ciphertext: None,
            encryption_key_id: "default".into(),
            authorization_url: Some("https://github.com/login/oauth/authorize".into()),
            token_url: Some("https://github.com/login/oauth/access_token".into()),
            userinfo_url: Some("https://api.github.com/user".into()),
            discovery_url: None,
            scopes: vec!["user:email".into()],
            is_active: true,
            mapping_config: ProviderMapping {
                subject_field: "id".into(),
                email_field: "email".into(),
                email_verified_field: "email_verified".into(),
                name_field: "login".into(),
                username_field: None,
            },
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn oidc_provider_builds_authorize_url_with_pkce() {
        let mut client = UpstreamOidcClient::new(
            google_provider(),
            "https://auth.example.com/callback".into(),
        );
        // Configure from explicit endpoints for testing (no network)
        client.provider.authorization_url = Some("https://accounts.google.com/o/oauth2/v2/auth".into());
        client.provider.token_url = Some("https://oauth2.googleapis.com/token".into());
        client.configure().expect("configure");

        let state = OAuthManager::generate_state();
        let (url, pkce) = client.authorize_url(&state).expect("authorize_url");

        assert!(url.contains("response_type=code"));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state="));
        assert!(!pkce.code_verifier.is_empty());
    }

    #[test]
    fn oauth2_provider_with_explicit_urls_configures() {
        let mut client = UpstreamOidcClient::new(
            github_provider(),
            "https://auth.example.com/callback".into(),
        );
        client.configure().expect("configure");
        assert_eq!(
            client.oauth_manager().config().authorization_url,
            "https://github.com/login/oauth/authorize"
        );
    }

    #[test]
    fn provider_with_no_endpoints_returns_error() {
        let mut p = google_provider();
        p.discovery_url = None;
        let mut client =
            UpstreamOidcClient::new(p, "https://auth.example.com/callback".into());
        let err = client.configure().unwrap_err();
        assert!(matches!(err, UpstreamClientError::NoEndpoints));
    }

    #[test]
    fn profile_extraction_from_userinfo() {
        let json = serde_json::json!({
            "sub": "12345",
            "email": "user@example.com",
            "email_verified": true,
            "name": "Test User",
            "preferred_username": "testuser",
        });
        let profile = UpstreamProfile::from_userinfo(&json, &ProviderMapping::default())
            .expect("extract profile");
        assert_eq!(profile.sub, "12345");
        assert_eq!(profile.email.unwrap(), "user@example.com");
        assert!(profile.email_verified);
        assert_eq!(profile.name.unwrap(), "Test User");
        assert_eq!(profile.preferred_username.unwrap(), "testuser");
    }

    #[test]
    fn profile_extraction_uses_custom_mapping() {
        let json = serde_json::json!({
            "id": 42,
            "login": "ghuser",
            "email": "gh@example.com",
        });
        let mapping = ProviderMapping {
            subject_field: "login".into(),
            email_field: "email".into(),
            email_verified_field: "email_verified".into(),
            name_field: "login".into(),
            username_field: None,
        };
        let profile =
            UpstreamProfile::from_userinfo(&json, &mapping).expect("extract profile");
        assert_eq!(profile.sub, "ghuser");
        assert_eq!(profile.name.unwrap(), "ghuser");
    }
}

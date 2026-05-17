//! OAuth 2.0 flows module with PKCE support.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// OAuth 2.0 configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    /// Client ID.
    pub client_id: String,
    /// Client secret (optional for public clients).
    pub client_secret: Option<String>,
    /// Authorization endpoint URL.
    pub authorization_url: String,
    /// Token endpoint URL.
    pub token_url: String,
    /// Redirect URI.
    pub redirect_uri: String,
    /// Requested scopes (space-separated).
    pub scopes: Vec<String>,
    /// Enable PKCE (recommended for public clients).
    pub pkce_enabled: bool,
    /// Response type (default: "code").
    pub response_type: String,
    /// Grant type.
    pub grant_type: String,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: None,
            authorization_url: String::new(),
            token_url: String::new(),
            redirect_uri: String::new(),
            scopes: Vec::new(),
            pkce_enabled: true,
            response_type: "code".to_string(),
            grant_type: "authorization_code".to_string(),
        }
    }
}

impl OAuthConfig {
    /// Create a new OAuth config builder.
    #[must_use]
    pub fn builder() -> OAuthConfigBuilder {
        OAuthConfigBuilder::new()
    }

    /// Validate the configuration.
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if any required field is missing.
    pub fn validate(&self) -> Result<(), OAuthError> {
        if self.client_id.is_empty() {
            return Err(OAuthError::MissingClientId);
        }
        if self.authorization_url.is_empty() {
            return Err(OAuthError::MissingAuthorizationUrl);
        }
        if self.token_url.is_empty() {
            return Err(OAuthError::MissingTokenUrl);
        }
        if self.redirect_uri.is_empty() {
            return Err(OAuthError::MissingRedirectUri);
        }
        Ok(())
    }
}

/// Builder for [`OAuthConfig`].
pub struct OAuthConfigBuilder {
    config: OAuthConfig,
}

impl OAuthConfigBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: OAuthConfig::default(),
        }
    }

    #[must_use]
    pub fn client_id(mut self, client_id: impl Into<String>) -> Self {
        self.config.client_id = client_id.into();
        self
    }

    #[must_use]
    pub fn client_secret(mut self, client_secret: impl Into<String>) -> Self {
        self.config.client_secret = Some(client_secret.into());
        self
    }

    #[must_use]
    pub fn authorization_url(mut self, url: impl Into<String>) -> Self {
        self.config.authorization_url = url.into();
        self
    }

    #[must_use]
    pub fn token_url(mut self, url: impl Into<String>) -> Self {
        self.config.token_url = url.into();
        self
    }

    #[must_use]
    pub fn redirect_uri(mut self, uri: impl Into<String>) -> Self {
        self.config.redirect_uri = uri.into();
        self
    }

    #[must_use]
    pub fn scopes(mut self, scopes: Vec<String>) -> Self {
        self.config.scopes = scopes;
        self
    }

    #[must_use]
    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        self.config.scopes.push(scope.into());
        self
    }

    #[must_use]
    pub fn pkce_enabled(mut self, enabled: bool) -> Self {
        self.config.pkce_enabled = enabled;
        self
    }

    #[must_use]
    pub fn build(self) -> OAuthConfig {
        self.config
    }
}

impl Default for OAuthConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// PKCE challenge pair.
#[derive(Debug, Clone)]
pub struct PkceChallenge {
    /// The code verifier (random string).
    pub code_verifier: String,
    /// The code challenge (SHA256 hash of verifier, base64 encoded).
    pub code_challenge: String,
    /// Challenge method (always "S256").
    pub challenge_method: String,
}

impl PkceChallenge {
    /// Generate a new PKCE challenge pair.
    #[must_use]
    pub fn generate() -> Self {
        // Generate a random code verifier (43-128 characters)
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let code_verifier = URL_SAFE_NO_PAD.encode(bytes);

        // Generate code challenge (SHA256 hash of verifier)
        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let hash = hasher.finalize();
        let code_challenge = URL_SAFE_NO_PAD.encode(hash);

        Self {
            code_verifier,
            code_challenge,
            challenge_method: "S256".to_string(),
        }
    }
}

/// OAuth-related errors.
#[derive(derive_more::From, Debug)]
pub enum OAuthError {
    /// Missing client ID.
    MissingClientId,
    /// Missing client secret.
    MissingClientSecret,
    /// Missing authorization URL.
    MissingAuthorizationUrl,
    /// Missing token URL.
    MissingTokenUrl,
    /// Missing redirect URI.
    MissingRedirectUri,
    /// Invalid URL.
    #[from(ignore)]
    InvalidUrl(String),
    /// Invalid state parameter.
    InvalidState,
    /// Token request failed.
    #[from(ignore)]
    TokenRequestFailed(String),
    /// Token endpoint error.
    TokenEndpointError { status: u16, message: String },
    /// Token parse error.
    #[from(ignore)]
    TokenParseError(String),
    /// PKCE generation failed.
    PkceFailed,
}

impl core::fmt::Display for OAuthError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            OAuthError::MissingClientId => write!(f, "Missing client ID"),
            OAuthError::MissingClientSecret => write!(f, "Missing client secret"),
            OAuthError::MissingAuthorizationUrl => write!(f, "Missing authorization URL"),
            OAuthError::MissingTokenUrl => write!(f, "Missing token URL"),
            OAuthError::MissingRedirectUri => write!(f, "Missing redirect URI"),
            OAuthError::InvalidUrl(s) => write!(f, "Invalid URL: {s}"),
            OAuthError::InvalidState => write!(f, "Invalid state parameter"),
            OAuthError::TokenRequestFailed(s) => write!(f, "Token request failed: {s}"),
            OAuthError::TokenEndpointError { status, message } => {
                write!(f, "Token endpoint error ({status}): {message}")
            }
            OAuthError::TokenParseError(s) => write!(f, "Failed to parse token response: {s}"),
            OAuthError::PkceFailed => write!(f, "PKCE generation failed"),
        }
    }
}

impl std::error::Error for OAuthError {}

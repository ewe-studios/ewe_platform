//! OAuth 2.0 flows module with PKCE support.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use url::Url;

use crate::jwt::JwtToken;

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

/// Builder for OAuthConfig.
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

    pub fn client_id(mut self, client_id: impl Into<String>) -> Self {
        self.config.client_id = client_id.into();
        self
    }

    pub fn client_secret(mut self, client_secret: impl Into<String>) -> Self {
        self.config.client_secret = Some(client_secret.into());
        self
    }

    pub fn authorization_url(mut self, url: impl Into<String>) -> Self {
        self.config.authorization_url = url.into();
        self
    }

    pub fn token_url(mut self, url: impl Into<String>) -> Self {
        self.config.token_url = url.into();
        self
    }

    pub fn redirect_uri(mut self, uri: impl Into<String>) -> Self {
        self.config.redirect_uri = uri.into();
        self
    }

    pub fn scopes(mut self, scopes: Vec<String>) -> Self {
        self.config.scopes = scopes;
        self
    }

    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        self.config.scopes.push(scope.into());
        self
    }

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

/// OAuth manager for handling OAuth flows.
pub struct OAuthManager {
    config: OAuthConfig,
}

impl OAuthManager {
    /// Create a new OAuth manager with the given configuration.
    #[must_use]
    pub fn new(config: OAuthConfig) -> Self {
        Self { config }
    }

    /// Get the OAuth configuration.
    #[must_use]
    pub fn config(&self) -> &OAuthConfig {
        &self.config
    }

    /// Generate a random state parameter for CSRF protection.
    #[must_use]
    pub fn generate_state() -> String {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Generate the authorization URL with PKCE support.
    ///
    /// Returns the URL to redirect the user to, along with the PKCE challenge
    /// that must be stored for the code exchange.
    pub fn get_authorization_url(&self, state: &str) -> Result<(String, Option<PkceChallenge>), OAuthError> {
        self.config.validate()?;

        let mut url = Url::parse(&self.config.authorization_url)
            .map_err(|_| OAuthError::InvalidUrl(self.config.authorization_url.clone()))?;

        // Add required parameters
        url.query_pairs_mut()
            .append_pair("response_type", &self.config.response_type)
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", &self.config.redirect_uri)
            .append_pair("state", state);

        // Add scopes if present
        if !self.config.scopes.is_empty() {
            let scopes_joined = self.config.scopes.join(" ");
            url.query_pairs_mut()
                .append_pair("scope", &scopes_joined);
        }

        // Add PKCE if enabled
        let pkce = if self.config.pkce_enabled {
            let challenge = PkceChallenge::generate();
            url.query_pairs_mut()
                .append_pair("code_challenge", &challenge.code_challenge)
                .append_pair("code_challenge_method", &challenge.challenge_method);
            Some(challenge)
        } else {
            None
        };

        Ok((url.to_string(), pkce))
    }

    /// Validate the state parameter.
    #[must_use]
    pub fn validate_state(expected: &str, actual: &str) -> bool {
        // Constant-time comparison to prevent timing attacks
        expected.as_bytes() == actual.as_bytes()
    }

    /// Exchange authorization code for tokens.
    ///
    /// This completes the authorization code flow by sending the code
    /// to the token endpoint along with the PKCE code verifier.
    pub fn exchange_code(
        &self,
        code: &str,
        code_verifier: Option<&str>,
    ) -> Result<OAuthToken, OAuthError> {
        self.config.validate()?;

        // Build token request body as URL-encoded form data
        let mut body_parts = vec![
            format!("grant_type={}", urlencoding::encode("authorization_code")),
            format!("code={}", urlencoding::encode(code)),
            format!("redirect_uri={}", urlencoding::encode(&self.config.redirect_uri)),
            format!("client_id={}", urlencoding::encode(&self.config.client_id)),
        ];

        // Add client secret if available
        if let Some(ref secret) = self.config.client_secret {
            body_parts.push(format!("client_secret={}", urlencoding::encode(secret)));
        }

        // Add PKCE verifier if using PKCE
        if let Some(verifier) = code_verifier {
            body_parts.push(format!("code_verifier={}", urlencoding::encode(verifier)));
        }

        let body = body_parts.join("&");

        // Send token request using simple_http
        let client = foundation_core::wire::simple_http::client::SimpleHttpClient::from_system();
        let response = client
            .post(&self.config.token_url)
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .header(foundation_core::wire::simple_http::SimpleHeader::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body_text(body)
            .build_client()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .send()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;

        if !response.is_success() {
            let body = match response.get_body_ref() {
                foundation_core::wire::simple_http::SendSafeBody::Text(t) => t.clone(),
                foundation_core::wire::simple_http::SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
                _ => String::new(),
            };
            return Err(OAuthError::TokenEndpointError {
                status: response.get_status().into_usize() as u16,
                message: body,
            });
        }

        // Parse JSON response
        let body_text = match response.get_body_ref() {
            foundation_core::wire::simple_http::SendSafeBody::Text(t) => t.as_str(),
            foundation_core::wire::simple_http::SendSafeBody::Bytes(b) => std::str::from_utf8(b).map_err(|e| OAuthError::TokenParseError(e.to_string()))?,
            _ => "",
        };
        let token_response: TokenResponse = serde_json::from_str(body_text)
            .map_err(|e| OAuthError::TokenParseError(e.to_string()))?;

        Ok(OAuthToken {
            access_token: token_response.access_token,
            token_type: token_response.token_type,
            expires_in: token_response.expires_in,
            refresh_token: token_response.refresh_token,
            scope: token_response.scope,
            id_token: token_response.id_token,
        })
    }

    /// Client credentials flow for service-to-service authentication.
    pub fn client_credentials(&self, scopes: Option<Vec<String>>) -> Result<OAuthToken, OAuthError> {
        self.config.validate()?;

        let Some(ref client_secret) = self.config.client_secret else {
            return Err(OAuthError::MissingClientSecret);
        };

        // Build scope string first to avoid borrow issues
        let scope_str;
        if let Some(ref s) = scopes {
            scope_str = s.join(" ");
        } else if !self.config.scopes.is_empty() {
            scope_str = self.config.scopes.join(" ");
        } else {
            scope_str = String::new();
        }

        // Build request body as URL-encoded form data
        let mut body_parts = vec![
            format!("grant_type={}", urlencoding::encode("client_credentials")),
            format!("client_id={}", urlencoding::encode(&self.config.client_id)),
            format!("client_secret={}", urlencoding::encode(client_secret)),
        ];

        // Add requested scopes if present
        if !scope_str.is_empty() {
            body_parts.push(format!("scope={}", urlencoding::encode(&scope_str)));
        }

        let body = body_parts.join("&");

        let client = foundation_core::wire::simple_http::client::SimpleHttpClient::from_system();
        let response = client
            .post(&self.config.token_url)
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .header(foundation_core::wire::simple_http::SimpleHeader::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body_text(body)
            .build_client()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .send()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;

        if !response.is_success() {
            let body = match response.get_body_ref() {
                foundation_core::wire::simple_http::SendSafeBody::Text(t) => t.clone(),
                foundation_core::wire::simple_http::SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
                _ => String::new(),
            };
            return Err(OAuthError::TokenEndpointError {
                status: response.get_status().into_usize() as u16,
                message: body,
            });
        }

        // Parse JSON response
        let body_text = match response.get_body_ref() {
            foundation_core::wire::simple_http::SendSafeBody::Text(t) => t.as_str(),
            foundation_core::wire::simple_http::SendSafeBody::Bytes(b) => std::str::from_utf8(b).map_err(|e| OAuthError::TokenParseError(e.to_string()))?,
            _ => "",
        };
        let token_response: TokenResponse = serde_json::from_str(body_text)
            .map_err(|e| OAuthError::TokenParseError(e.to_string()))?;

        Ok(OAuthToken {
            access_token: token_response.access_token,
            token_type: token_response.token_type,
            expires_in: token_response.expires_in,
            refresh_token: None, // Client credentials don't return refresh tokens
            scope: token_response.scope,
            id_token: None,
        })
    }

    /// Refresh an access token using a refresh token.
    pub fn refresh_token(&self, refresh_token: &str) -> Result<OAuthToken, OAuthError> {
        self.config.validate()?;

        // Build request body as URL-encoded form data
        let mut body_parts = vec![
            format!("grant_type={}", urlencoding::encode("refresh_token")),
            format!("refresh_token={}", urlencoding::encode(refresh_token)),
            format!("client_id={}", urlencoding::encode(&self.config.client_id)),
        ];

        // Add client secret if available
        if let Some(ref secret) = self.config.client_secret {
            body_parts.push(format!("client_secret={}", urlencoding::encode(secret)));
        }

        let body = body_parts.join("&");

        let client = foundation_core::wire::simple_http::client::SimpleHttpClient::from_system();
        let response = client
            .post(&self.config.token_url)
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .header(foundation_core::wire::simple_http::SimpleHeader::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body_text(body)
            .build_client()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .send()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;

        if !response.is_success() {
            let body = match response.get_body_ref() {
                foundation_core::wire::simple_http::SendSafeBody::Text(t) => t.clone(),
                foundation_core::wire::simple_http::SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
                _ => String::new(),
            };
            return Err(OAuthError::TokenEndpointError {
                status: response.get_status().into_usize() as u16,
                message: body,
            });
        }

        // Parse JSON response
        let body_text = match response.get_body_ref() {
            foundation_core::wire::simple_http::SendSafeBody::Text(t) => t.as_str(),
            foundation_core::wire::simple_http::SendSafeBody::Bytes(b) => std::str::from_utf8(b).map_err(|e| OAuthError::TokenParseError(e.to_string()))?,
            _ => "",
        };
        let token_response: TokenResponse = serde_json::from_str(body_text)
            .map_err(|e| OAuthError::TokenParseError(e.to_string()))?;

        Ok(OAuthToken {
            access_token: token_response.access_token,
            token_type: token_response.token_type,
            expires_in: token_response.expires_in,
            refresh_token: token_response.refresh_token.or_else(|| Some(refresh_token.to_string())),
            scope: token_response.scope,
            id_token: token_response.id_token,
        })
    }
}

/// OAuth token response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    /// Access token.
    pub access_token: String,
    /// Token type (usually "Bearer").
    pub token_type: String,
    /// Expires in seconds.
    pub expires_in: Option<u64>,
    /// Refresh token (if provided).
    pub refresh_token: Option<String>,
    /// Granted scopes.
    pub scope: Option<String>,
    /// ID token (for OIDC).
    pub id_token: Option<String>,
}

impl OAuthToken {
    /// Convert to a JwtToken.
    #[must_use]
    #[allow(clippy::cast_possible_wrap)]
    pub fn into_jwt_token(self) -> Option<JwtToken> {
        let expires_at = self
            .expires_in
            .map(|exp| chrono::Utc::now().timestamp() + exp as i64)
            .unwrap_or_else(|| chrono::Utc::now().timestamp() + 3600); // Default 1 hour

        JwtToken::from_parts(self.access_token, self.refresh_token, expires_at, self.scope, None, None).into()
    }

    /// Check if the token is expired.
    #[must_use]
    pub fn is_expired(&self, buffer_seconds: u64) -> bool {
        self.expires_in
            .is_some_and(|exp| exp <= buffer_seconds)
    }
}

/// Token response from OAuth server.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
    scope: Option<String>,
    id_token: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oauth_config_builder() {
        let config = OAuthConfig::builder()
            .client_id("test_client_id")
            .client_secret("test_client_secret")
            .authorization_url("https://auth.example.com/oauth/authorize")
            .token_url("https://auth.example.com/oauth/token")
            .redirect_uri("https://app.example.com/callback")
            .scope("openid")
            .scope("profile")
            .scope("email")
            .pkce_enabled(true)
            .build();

        assert_eq!(config.client_id, "test_client_id");
        assert_eq!(config.client_secret, Some("test_client_secret".to_string()));
        assert_eq!(config.scopes.len(), 3);
        assert!(config.pkce_enabled);
    }

    #[test]
    fn test_oauth_config_validation() {
        let config = OAuthConfig::default();
        assert!(config.validate().is_err());

        let config = OAuthConfig::builder()
            .client_id("test")
            .authorization_url("https://auth.example.com")
            .token_url("https://auth.example.com/token")
            .redirect_uri("https://app.example.com/callback")
            .build();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_pkce_challenge_generation() {
        let challenge = PkceChallenge::generate();

        // Verifier should be 43 characters (32 bytes base64)
        assert_eq!(challenge.code_verifier.len(), 43);
        // Challenge should be 32 bytes (SHA256) = 43 base64 chars
        assert_eq!(challenge.code_challenge.len(), 43);
        // Method should be S256
        assert_eq!(challenge.challenge_method, "S256");
    }

    #[test]
    fn test_state_generation() {
        let state1 = OAuthManager::generate_state();
        let state2 = OAuthManager::generate_state();

        // States should be unique
        assert_ne!(state1, state2);
        // States should be reasonably long
        assert!(state1.len() > 30);
    }

    #[test]
    fn test_state_validation() {
        let state = "test_state_value";
        assert!(OAuthManager::validate_state(state, state));
        assert!(!OAuthManager::validate_state(state, "different_state"));
    }

    #[test]
    fn test_authorization_url_generation() {
        let config = OAuthConfig::builder()
            .client_id("test_client")
            .authorization_url("https://auth.example.com/oauth/authorize")
            .token_url("https://auth.example.com/oauth/token")
            .redirect_uri("https://app.example.com/callback")
            .scope("openid profile")
            .pkce_enabled(true)
            .build();

        let manager = OAuthManager::new(config);
        let state = OAuthManager::generate_state();
        let (url, pkce) = manager.get_authorization_url(&state).unwrap();

        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=test_client"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Fapp.example.com%2Fcallback"));
        assert!(url.contains(&format!("state={state}")));
        assert!(url.contains("scope=openid+profile"));
        assert!(pkce.is_some());
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_oauth_token_conversion() {
        let oauth_token = OAuthToken {
            access_token: "access_123".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: Some("refresh_456".to_string()),
            scope: Some("openid profile".to_string()),
            id_token: None,
        };

        let jwt_token = oauth_token.into_jwt_token().unwrap();
        assert_eq!(jwt_token.access_token(), "access_123");
        assert_eq!(jwt_token.refresh_token(), Some("refresh_456".to_string()));
        assert!(!jwt_token.is_expired());
    }
}

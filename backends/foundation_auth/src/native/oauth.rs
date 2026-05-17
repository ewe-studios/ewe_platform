//! OAuth 2.0 flows requiring native HTTP client access.

use serde::Deserialize;
use url::Url;

use crate::shared::oauth::{OAuthConfig, OAuthError, PkceChallenge};
use crate::shared::oauth_token::OAuthToken;

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
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        use rand::RngCore;
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Generate the authorization URL with PKCE support.
    ///
    /// Returns the URL to redirect the user to, along with the PKCE challenge
    /// that must be stored for the code exchange.
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the configuration is invalid or the URL cannot be parsed.
    pub fn get_authorization_url(
        &self,
        state: &str,
    ) -> Result<(String, Option<PkceChallenge>), OAuthError> {
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
            url.query_pairs_mut().append_pair("scope", &scopes_joined);
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
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the token request fails or the response cannot be parsed.
    #[allow(clippy::cast_possible_truncation)]
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
            format!(
                "redirect_uri={}",
                urlencoding::encode(&self.config.redirect_uri)
            ),
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
            .header(
                foundation_core::wire::simple_http::SimpleHeader::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body_text(body)
            .build_client()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .send()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;

        if !response.is_success() {
            let body = match response.get_body_ref() {
                foundation_core::wire::simple_http::SendSafeBody::Text(t) => t.clone(),
                foundation_core::wire::simple_http::SendSafeBody::Bytes(b) => {
                    String::from_utf8_lossy(b).to_string()
                }
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
            foundation_core::wire::simple_http::SendSafeBody::Bytes(b) => {
                std::str::from_utf8(b).map_err(|e| OAuthError::TokenParseError(e.to_string()))?
            }
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
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the token request fails or the response cannot be parsed.
    #[allow(clippy::cast_possible_truncation, clippy::needless_pass_by_value)]
    pub fn client_credentials(
        &self,
        scopes: Option<Vec<String>>,
    ) -> Result<OAuthToken, OAuthError> {
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
            .header(
                foundation_core::wire::simple_http::SimpleHeader::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body_text(body)
            .build_client()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .send()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;

        if !response.is_success() {
            let body = match response.get_body_ref() {
                foundation_core::wire::simple_http::SendSafeBody::Text(t) => t.clone(),
                foundation_core::wire::simple_http::SendSafeBody::Bytes(b) => {
                    String::from_utf8_lossy(b).to_string()
                }
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
            foundation_core::wire::simple_http::SendSafeBody::Bytes(b) => {
                std::str::from_utf8(b).map_err(|e| OAuthError::TokenParseError(e.to_string()))?
            }
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
    ///
    /// # Errors
    ///
    /// Returns an `OAuthError` if the refresh request fails or the response cannot be parsed.
    #[allow(clippy::cast_possible_truncation)]
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
            .header(
                foundation_core::wire::simple_http::SimpleHeader::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body_text(body)
            .build_client()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .send()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;

        if !response.is_success() {
            let body = match response.get_body_ref() {
                foundation_core::wire::simple_http::SendSafeBody::Text(t) => t.clone(),
                foundation_core::wire::simple_http::SendSafeBody::Bytes(b) => {
                    String::from_utf8_lossy(b).to_string()
                }
                _ => String::new(),
            };
            #[allow(clippy::cast_possible_truncation)]
            return Err(OAuthError::TokenEndpointError {
                status: response.get_status().into_usize() as u16,
                message: body,
            });
        }

        // Parse JSON response
        let body_text = match response.get_body_ref() {
            foundation_core::wire::simple_http::SendSafeBody::Text(t) => t.as_str(),
            foundation_core::wire::simple_http::SendSafeBody::Bytes(b) => {
                std::str::from_utf8(b).map_err(|e| OAuthError::TokenParseError(e.to_string()))?
            }
            _ => "",
        };
        let token_response: TokenResponse = serde_json::from_str(body_text)
            .map_err(|e| OAuthError::TokenParseError(e.to_string()))?;

        Ok(OAuthToken {
            access_token: token_response.access_token,
            token_type: token_response.token_type,
            expires_in: token_response.expires_in,
            refresh_token: token_response
                .refresh_token
                .or_else(|| Some(refresh_token.to_string())),
            scope: token_response.scope,
            id_token: token_response.id_token,
        })
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

#[cfg(test)]
mod tests {
    use crate::shared::oauth::{OAuthConfig, OAuthError, PkceChallenge};
    use crate::shared::oauth_token::OAuthToken;

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

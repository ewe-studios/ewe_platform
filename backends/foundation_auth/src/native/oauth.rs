//! OAuth 2.0 token exchange using native HTTP client.

use crate::shared::oauth::{OAuthConfig, OAuthError, OAuthManager, TokenResponse};
use crate::shared::oauth_token::OAuthToken;

/// Native OAuth client wrapping shared OAuth configuration with sync token exchange.
pub struct NativeOAuth {
    inner: OAuthManager,
}

impl NativeOAuth {
    /// Create a new native OAuth client.
    #[must_use]
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            inner: OAuthManager::new(config),
        }
    }

    /// Get the shared OAuth manager for URL generation.
    #[must_use]
    pub fn manager(&self) -> &OAuthManager {
        &self.inner
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
        self.inner.config.validate()?;

        let mut body_parts = vec![
            format!("grant_type={}", urlencoding::encode("authorization_code")),
            format!("code={}", urlencoding::encode(code)),
            format!(
                "redirect_uri={}",
                urlencoding::encode(&self.inner.config.redirect_uri)
            ),
            format!("client_id={}", urlencoding::encode(&self.inner.config.client_id)),
        ];

        if let Some(ref secret) = self.inner.config.client_secret {
            body_parts.push(format!("client_secret={}", urlencoding::encode(secret)));
        }

        if let Some(verifier) = code_verifier {
            body_parts.push(format!("code_verifier={}", urlencoding::encode(verifier)));
        }

        let body = body_parts.join("&");

        let client = foundation_netio::simple_http::client::SimpleHttpClient::from_system();
        let response = client
            .post(&self.inner.config.token_url)
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .header(
                foundation_netio::simple_http::shared::SimpleHeader::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body_text(body)
            .build_client()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .send()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;

        if !response.is_success() {
            let body = match response.get_body_ref() {
                foundation_netio::simple_http::shared::SendSafeBody::Text(t) => t.clone(),
                foundation_netio::simple_http::shared::SendSafeBody::Bytes(b) => {
                    String::from_utf8_lossy(b).to_string()
                }
                _ => String::new(),
            };
            return Err(OAuthError::TokenEndpointError {
                status: response.get_status().into_usize() as u16,
                message: body,
            });
        }

        let body_text = match response.get_body_ref() {
            foundation_netio::simple_http::shared::SendSafeBody::Text(t) => t.as_str(),
            foundation_netio::simple_http::shared::SendSafeBody::Bytes(b) => {
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
        self.inner.config.validate()?;

        let Some(ref client_secret) = self.inner.config.client_secret else {
            return Err(OAuthError::MissingClientSecret);
        };

        let scope_str;
        if let Some(ref s) = scopes {
            scope_str = s.join(" ");
        } else if !self.inner.config.scopes.is_empty() {
            scope_str = self.inner.config.scopes.join(" ");
        } else {
            scope_str = String::new();
        }

        let mut body_parts = vec![
            format!("grant_type={}", urlencoding::encode("client_credentials")),
            format!("client_id={}", urlencoding::encode(&self.inner.config.client_id)),
            format!("client_secret={}", urlencoding::encode(client_secret)),
        ];

        if !scope_str.is_empty() {
            body_parts.push(format!("scope={}", urlencoding::encode(&scope_str)));
        }

        let body = body_parts.join("&");

        let client = foundation_netio::simple_http::client::SimpleHttpClient::from_system();
        let response = client
            .post(&self.inner.config.token_url)
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .header(
                foundation_netio::simple_http::shared::SimpleHeader::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body_text(body)
            .build_client()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .send()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;

        if !response.is_success() {
            let body = match response.get_body_ref() {
                foundation_netio::simple_http::shared::SendSafeBody::Text(t) => t.clone(),
                foundation_netio::simple_http::shared::SendSafeBody::Bytes(b) => {
                    String::from_utf8_lossy(b).to_string()
                }
                _ => String::new(),
            };
            return Err(OAuthError::TokenEndpointError {
                status: response.get_status().into_usize() as u16,
                message: body,
            });
        }

        let body_text = match response.get_body_ref() {
            foundation_netio::simple_http::shared::SendSafeBody::Text(t) => t.as_str(),
            foundation_netio::simple_http::shared::SendSafeBody::Bytes(b) => {
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
            refresh_token: None,
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
        self.inner.config.validate()?;

        let mut body_parts = vec![
            format!("grant_type={}", urlencoding::encode("refresh_token")),
            format!("refresh_token={}", urlencoding::encode(refresh_token)),
            format!("client_id={}", urlencoding::encode(&self.inner.config.client_id)),
        ];

        if let Some(ref secret) = self.inner.config.client_secret {
            body_parts.push(format!("client_secret={}", urlencoding::encode(secret)));
        }

        let body = body_parts.join("&");

        let client = foundation_netio::simple_http::client::SimpleHttpClient::from_system();
        let response = client
            .post(&self.inner.config.token_url)
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .header(
                foundation_netio::simple_http::shared::SimpleHeader::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body_text(body)
            .build_client()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
            .send()
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;

        if !response.is_success() {
            let body = match response.get_body_ref() {
                foundation_netio::simple_http::shared::SendSafeBody::Text(t) => t.clone(),
                foundation_netio::simple_http::shared::SendSafeBody::Bytes(b) => {
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

        let body_text = match response.get_body_ref() {
            foundation_netio::simple_http::shared::SendSafeBody::Text(t) => t.as_str(),
            foundation_netio::simple_http::shared::SendSafeBody::Bytes(b) => {
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

#[cfg(test)]
mod tests {
    use crate::shared::oauth::{OAuthConfig, OAuthManager, PkceChallenge};
    use crate::shared::oauth_token::OAuthToken;
    use crate::native::oauth::NativeOAuth;

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
        assert_eq!(challenge.code_verifier.len(), 43);
        assert_eq!(challenge.code_challenge.len(), 43);
        assert_eq!(challenge.challenge_method, "S256");
    }

    #[test]
    fn test_state_generation() {
        let state1 = OAuthManager::generate_state();
        let state2 = OAuthManager::generate_state();
        assert_ne!(state1, state2);
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

        let manager = NativeOAuth::new(config);
        let state = OAuthManager::generate_state();
        let (url, pkce) = manager.manager().get_authorization_url(&state).unwrap();

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

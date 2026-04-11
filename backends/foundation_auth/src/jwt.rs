//! JWT management module for token lifecycle handling.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ConfidentialText;

/// A JWT token with metadata for lifecycle management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtToken {
    /// The raw JWT token string.
    pub token: ConfidentialText,
    /// Refresh token (if available).
    pub refresh_token: Option<ConfidentialText>,
    /// Token expiration timestamp (Unix epoch seconds).
    pub expires_at: i64,
    /// Token scope.
    pub scope: Option<String>,
    /// Token audience.
    pub audience: Option<String>,
    /// Token issuer.
    pub issuer: Option<String>,
    /// When the token was created.
    pub created_at: i64,
}

impl JwtToken {
    /// Create a new JWT token from parts.
    #[must_use]
    pub fn from_parts(
        token: String,
        refresh_token: Option<String>,
        expires_at: i64,
        scope: Option<String>,
        audience: Option<String>,
        issuer: Option<String>,
    ) -> Self {
        let now = Utc::now().timestamp();
        Self {
            token: ConfidentialText::new(token),
            refresh_token: refresh_token.map(ConfidentialText::new),
            expires_at,
            scope,
            audience,
            issuer,
            created_at: now,
        }
    }

    /// Create from a raw token string by parsing the JWT payload.
    ///
    /// # Errors
    ///
    /// Returns a `JwtError` if the token cannot be parsed or is missing required claims.
    pub fn from_token(token: String) -> Result<Self, JwtError> {
        // Parse the JWT payload to extract claims
        let claims = decode_claims(&token)?;

        let expires_at = claims
            .exp
            .ok_or(JwtError::MissingExpiration)?;

        Ok(Self {
            token: ConfidentialText::new(token),
            refresh_token: None, // Refresh token not embedded in JWT
            expires_at,
            scope: claims.custom.get("scope").and_then(|v| v.as_str().map(String::from)),
            audience: claims.custom.get("aud").and_then(|v| v.as_str().map(String::from)),
            issuer: claims.custom.get("iss").and_then(|v| v.as_str().map(String::from)),
            created_at: Utc::now().timestamp(),
        })
    }

    /// Create with a refresh token.
    #[must_use]
    pub fn with_refresh_token(mut self, refresh_token: String) -> Self {
        self.refresh_token = Some(ConfidentialText::new(refresh_token));
        self
    }

    /// Check if the token is expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp();
        now >= self.expires_at
    }

    /// Check if the token expires within the given buffer (in seconds).
    #[must_use]
    pub fn expires_within(&self, buffer_seconds: i64) -> bool {
        let now = Utc::now().timestamp();
        now + buffer_seconds >= self.expires_at
    }

    /// Get seconds until expiration.
    #[must_use]
    pub fn expires_in(&self) -> i64 {
        let now = Utc::now().timestamp();
        (self.expires_at - now).max(0)
    }

    /// Get the refresh token if available.
    #[must_use]
    pub fn refresh_token(&self) -> Option<String> {
        self.refresh_token.as_ref().map(ConfidentialText::get)
    }

    /// Get the access token.
    #[must_use]
    pub fn access_token(&self) -> String {
        self.token.get()
    }
}

/// JWT Manager for handling token lifecycle.
pub struct JwtManager {
    /// Current token.
    token: Option<JwtToken>,
    /// Refresh buffer in seconds (default 5 minutes).
    refresh_buffer: i64,
    /// Token storage key.
    storage_key: String,
}

impl Default for JwtManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JwtManager {
    /// Create a new JWT manager with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            token: None,
            refresh_buffer: 300, // 5 minutes
            storage_key: String::from("jwt:token"),
        }
    }

    /// Create with a custom refresh buffer.
    #[must_use]
    pub fn with_refresh_buffer(mut self, buffer_seconds: i64) -> Self {
        self.refresh_buffer = buffer_seconds;
        self
    }

    /// Create with a custom storage key.
    #[must_use]
    pub fn with_storage_key(mut self, key: impl Into<String>) -> Self {
        self.storage_key = key.into();
        self
    }

    /// Set the JWT token.
    pub fn set_token(&mut self, token: JwtToken) {
        self.token = Some(token);
    }

    /// Get the current token without checking expiration.
    #[must_use]
    pub fn get_token(&self) -> Option<&JwtToken> {
        self.token.as_ref()
    }

    /// Clear the stored token.
    pub fn clear_token(&mut self) {
        self.token = None;
    }

    /// Get a valid token, refreshing if necessary.
    ///
    /// Returns the access token string if available and valid.
    ///
    /// # Errors
    ///
    /// Returns a `JwtError` if there is no token, no refresh token, or if the refresh function fails.
    pub fn get_valid_token<F>(&mut self, refresh_fn: F) -> Result<String, JwtError>
    where
        F: FnOnce(String) -> Result<JwtToken, JwtError>,
    {
        // Check if we have a token
        let Some(token) = &self.token else {
            return Err(JwtError::NoToken);
        };

        // Check if token needs refresh (expired or within buffer)
        if token.is_expired() || token.expires_within(self.refresh_buffer) {
            // Need to refresh
            let Some(refresh_token) = token.refresh_token() else {
                return Err(JwtError::NoRefreshToken);
            };

            // Call the refresh function
            let new_token = refresh_fn(refresh_token)?;
            self.token = Some(new_token);
        }

        // Return the current access token
        self.token
            .as_ref()
            .map(JwtToken::access_token)
            .ok_or(JwtError::NoToken)
    }

    /// Refresh the token if needed (within buffer or expired).
    ///
    /// # Errors
    ///
    /// Returns a `JwtError` if there is no refresh token or if the refresh function fails.
    pub fn refresh_if_needed<F>(&mut self, refresh_fn: F) -> Result<bool, JwtError>
    where
        F: FnOnce(String) -> Result<JwtToken, JwtError>,
    {
        let Some(token) = &self.token else {
            return Ok(false);
        };

        if !token.is_expired() && !token.expires_within(self.refresh_buffer) {
            return Ok(false); // No refresh needed
        }

        let Some(refresh_token) = token.refresh_token() else {
            return Err(JwtError::NoRefreshToken);
        };

        let new_token = refresh_fn(refresh_token)?;
        self.token = Some(new_token);
        Ok(true)
    }

    /// Check if the manager has a valid (non-expired) token.
    #[must_use]
    pub fn has_valid_token(&self) -> bool {
        self.token
            .as_ref()
            .is_some_and(|t| !t.is_expired() && !t.expires_within(self.refresh_buffer))
    }

    /// Get the storage key for persistence.
    #[must_use]
    pub fn storage_key(&self) -> &str {
        &self.storage_key
    }
}

/// Parse claims from a JWT token without full validation.
fn decode_claims(token: &str) -> Result<Claims, JwtError> {
    // Split the token into parts
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(JwtError::InvalidTokenFormat);
    }

    // Decode the payload (second part)
    let payload = base64_decode(parts[1])?;
    let claims: Claims = serde_json::from_slice(&payload)?;

    Ok(claims)
}

/// Base64 decode with URL-safe alphabet.
fn base64_decode(input: &str) -> Result<Vec<u8>, JwtError> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.decode(input).map_err(|_| JwtError::DecodeError)
}

/// JWT claims structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Issuer.
    #[serde(rename = "iss", skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
    /// Subject.
    #[serde(rename = "sub", skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    /// Audience.
    #[serde(rename = "aud", skip_serializing_if = "Option::is_none")]
    pub aud: Option<String>,
    /// Expiration time (Unix timestamp).
    #[serde(rename = "exp", skip_serializing_if = "Option::is_none")]
    pub exp: Option<i64>,
    /// Not before (Unix timestamp).
    #[serde(rename = "nbf", skip_serializing_if = "Option::is_none")]
    pub nbf: Option<i64>,
    /// Issued at (Unix timestamp).
    #[serde(rename = "iat", skip_serializing_if = "Option::is_none")]
    pub iat: Option<i64>,
    /// JWT ID.
    #[serde(rename = "jti", skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
    /// Custom claims.
    #[serde(flatten)]
    pub custom: serde_json::Map<String, serde_json::Value>,
}

impl Claims {
    /// Get the expiration time as [`DateTime`].
    #[must_use]
    pub fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.exp.and_then(|ts| DateTime::from_timestamp(ts, 0))
    }
}

/// JWT-related errors.
#[derive(derive_more::From, Debug)]
pub enum JwtError {
    /// No token available.
    NoToken,
    /// No refresh token available.
    NoRefreshToken,
    /// Token refresh failed.
    #[from(ignore)]
    RefreshFailed(String),
    /// Invalid token format.
    InvalidTokenFormat,
    /// Token is expired.
    TokenExpired,
    /// Missing expiration claim.
    MissingExpiration,
    /// Base64 decode error.
    #[from(ignore)]
    DecodeError,
    /// JSON parse error.
    Json(serde_json::Error),
    /// Token generation error.
    #[from(ignore)]
    GenerationError(String),
}

impl core::fmt::Display for JwtError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            JwtError::NoToken => write!(f, "No JWT token available"),
            JwtError::NoRefreshToken => write!(f, "No refresh token available"),
            JwtError::RefreshFailed(s) => write!(f, "Token refresh failed: {s}"),
            JwtError::InvalidTokenFormat => write!(f, "Invalid JWT token format"),
            JwtError::TokenExpired => write!(f, "JWT token is expired"),
            JwtError::MissingExpiration => write!(f, "JWT token missing expiration claim"),
            JwtError::DecodeError => write!(f, "Base64 decode error"),
            JwtError::Json(e) => write!(f, "JSON parse error: {e}"),
            JwtError::GenerationError(s) => write!(f, "Token generation error: {s}"),
        }
    }
}

impl std::error::Error for JwtError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_token_creation() {
        let future_time = Utc::now().timestamp() + 3600; // 1 hour from now
        let token = JwtToken::from_parts(
            "test_token".to_string(),
            Some("test_refresh".to_string()),
            future_time,
            Some("read:write".to_string()),
            Some("api".to_string()),
            Some("auth.example.com".to_string()),
        );

        assert!(!token.is_expired());
        assert_eq!(token.access_token(), "test_token");
        assert_eq!(token.refresh_token(), Some("test_refresh".to_string()));
        assert_eq!(token.scope, Some("read:write".to_string()));
    }

    #[test]
    fn test_jwt_token_expiration() {
        let past_time = Utc::now().timestamp() - 3600; // 1 hour ago
        let token = JwtToken::from_parts(
            "expired_token".to_string(),
            None,
            past_time,
            None,
            None,
            None,
        );

        assert!(token.is_expired());
        assert!(token.expires_in() == 0);
    }

    #[test]
    fn test_jwt_token_expires_within() {
        let soon = Utc::now().timestamp() + 120; // 2 minutes from now
        let token = JwtToken::from_parts(
            "expiring_token".to_string(),
            None,
            soon,
            None,
            None,
            None,
        );

        // Should expire within 5 minutes
        assert!(token.expires_within(300));
        // Should NOT expire within 1 minute
        assert!(!token.expires_within(60));
    }

    #[test]
    fn test_jwt_manager_refresh() {
        let mut manager = JwtManager::new().with_refresh_buffer(300);

        // Initially no token
        assert!(!manager.has_valid_token());

        // Set a token that's about to expire
        let soon = Utc::now().timestamp() + 60; // 1 minute from now
        manager.set_token(JwtToken::from_parts(
            "test_token".to_string(),
            Some("refresh_token".to_string()),
            soon,
            None,
            None,
            None,
        ));

        // Should need refresh (within buffer)
        assert!(manager.token.as_ref().unwrap().expires_within(300));

        // Mock refresh function (synchronous)
        let refresh_fn = |_refresh_token: String| {
            let future = Utc::now().timestamp() + 3600;
            Ok(JwtToken::from_parts(
                "new_token".to_string(),
                Some("new_refresh".to_string()),
                future,
                None,
                None,
                None,
            ))
        };

        // Refresh should succeed
        let refreshed = manager.refresh_if_needed(refresh_fn).unwrap();
        assert!(refreshed);
        assert_eq!(manager.get_token().unwrap().access_token(), "new_token");
    }
}

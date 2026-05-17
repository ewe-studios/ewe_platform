//! OAuth token data type (pure data, no native deps).

use serde::{Deserialize, Serialize};

use crate::shared::jwt::JwtToken;

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
    /// Convert to a [`JwtToken`].
    #[must_use]
    #[allow(clippy::cast_possible_wrap)]
    pub fn into_jwt_token(self) -> Option<JwtToken> {
        let expires_at = self.expires_in.map_or_else(
            || chrono::Utc::now().timestamp() + 3600,
            |exp| chrono::Utc::now().timestamp() + exp as i64,
        );

        JwtToken::from_parts(
            self.access_token,
            self.refresh_token,
            expires_at,
            self.scope,
            None,
            None,
        )
        .into()
    }

    /// Check if the token is expired.
    #[must_use]
    pub fn is_expired(&self, buffer_seconds: u64) -> bool {
        self.expires_in.is_some_and(|exp| exp <= buffer_seconds)
    }
}

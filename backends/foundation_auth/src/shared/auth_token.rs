//! Unified authentication token type across auth methods.
//!
//! WHY: Different auth methods (OAuth, JWT, API key, session) need a single
//! type to represent the active credential for downstream use.
//!
//! WHAT: `AuthToken` enum with variants for each auth method.
//! HOW: Wraps existing credential types; provides unified `is_expired()` interface.

use chrono::Utc;

use crate::{AuthCredential, ConfidentialText, JwtCredential, OAuthCredential, SessionCredential};
use foundation_netio::shared::client::Cookie;

/// Unified authentication token representation.
#[derive(Debug, Clone)]
pub enum AuthToken {
    /// OAuth access token with optional refresh.
    OAuth {
        access_token: ConfidentialText,
        refresh_token: Option<ConfidentialText>,
        token_type: String,
        expires_at: f64,
        scope: Option<String>,
    },
    /// JWT bearer token.
    Jwt {
        token: ConfidentialText,
        expires_at: f64,
        issuer: Option<String>,
        audience: Option<String>,
    },
    /// API key / bearer token (no expiration).
    ApiKey { key: ConfidentialText },
    /// Session-based auth with cookie.
    Session {
        session_id: String,
        token: ConfidentialText,
        expires_at: f64,
        cookie: Option<Cookie>,
    },
}

impl AuthToken {
    /// Whether the token is currently valid.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn is_expired(&self) -> bool {
        match self {
            Self::OAuth { expires_at, .. }
            | Self::Jwt { expires_at, .. }
            | Self::Session { expires_at, .. } => Utc::now().timestamp() as f64 >= *expires_at,
            Self::ApiKey { .. } => false, // API keys don't expire
        }
    }

    /// Get the bearer token value (for API calls).
    ///
    /// Returns `None` for session-based tokens (which use cookies instead).
    #[must_use]
    pub fn bearer_token(&self) -> Option<String> {
        match self {
            Self::OAuth {
                access_token,
                token_type,
                ..
            } => Some(format!("{token_type} {}", access_token.get())),
            Self::Jwt { token, .. } => Some(format!("Bearer {}", token.get())),
            Self::ApiKey { key } => Some(format!("Bearer {}", key.get())),
            Self::Session { .. } => None,
        }
    }
}

impl From<OAuthCredential> for AuthToken {
    fn from(cred: OAuthCredential) -> Self {
        Self::OAuth {
            access_token: cred.access_token,
            refresh_token: cred.refresh_token,
            token_type: "Bearer".to_string(),
            expires_at: cred.expires,
            scope: None,
        }
    }
}

impl From<JwtCredential> for AuthToken {
    fn from(cred: JwtCredential) -> Self {
        Self::Jwt {
            token: cred.token,
            expires_at: cred.expires,
            issuer: None,
            audience: None,
        }
    }
}

impl From<SessionCredential> for AuthToken {
    fn from(cred: SessionCredential) -> Self {
        Self::Session {
            session_id: cred.session_id,
            token: cred.token,
            expires_at: cred.expires,
            cookie: cred.cookie,
        }
    }
}

impl From<AuthCredential> for AuthToken {
    fn from(cred: AuthCredential) -> Self {
        match cred {
            AuthCredential::OAuth(oauth) => Self::from(oauth),
            AuthCredential::ClientSecret {
                client_id,
                client_secret,
            } => Self::ApiKey {
                key: ConfidentialText::new(format!("{}:{}", client_id.get(), client_secret.get())),
            },
            AuthCredential::UsernameAndPassword { username, password } => {
                use base64::{engine::general_purpose::STANDARD, Engine};
                let encoded = STANDARD.encode(format!("{username}:{}", password.get()));
                Self::ApiKey {
                    key: ConfidentialText::new(format!("Basic {encoded}")),
                }
            }
            AuthCredential::EmailAuth { .. } | AuthCredential::SecretOnly(_) => Self::ApiKey {
                key: ConfidentialText::new(String::new()),
            },
        }
    }
}


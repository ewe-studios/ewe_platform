//! Foundation Auth - Comprehensive authentication infrastructure.
//!
//! This crate provides authentication flows, credential management, and token handling
//! for use with AI inference providers and other services requiring authentication.

pub mod auth_state;
pub mod auth_token;
pub mod credential_store;
pub mod jwt;
pub mod middleware;
pub mod oauth;
pub mod session;
pub mod two_factor;

use std::error::Error;

use derive_more::From;
use foundation_core::{valtron::StreamIterator, wire::simple_http::client::Cookie};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroizing;

pub use auth_state::{AuthEvent, AuthState, AuthStateError, AuthStateMachine, QueuedRequest};
pub use auth_token::AuthToken;
pub use credential_store::{
    CredentialStorage, CredentialStore, CredentialStoreError, OAuthTokenStore, StoredCredential,
};
pub use jwt::{JwtError, JwtManager, JwtToken};
pub use middleware::{
    extract_bearer_token, extract_session_token, has_scope, optional_auth, require_auth,
    AuthContext, GuardResult,
};
pub use oauth::{OAuthConfig, OAuthError, OAuthManager, OAuthToken, PkceChallenge};
pub use session::{Session, SessionConfig, SessionError, SessionManager};
pub use two_factor::{BackupCodeSet, TOTPSecret, TwoFactorChallenge, TwoFactorError};

#[derive(From, Clone)]
pub struct ConfidentialText(Zeroizing<String>);

impl ConfidentialText {
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    #[must_use]
    pub fn get(&self) -> String {
        (*self.0).clone()
    }
}

impl Serialize for ConfidentialText {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.get().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ConfidentialText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(Self::new(value))
    }
}

impl core::fmt::Debug for ConfidentialText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:}")
    }
}

impl core::fmt::Display for ConfidentialText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = &(*self.0);
        let first_char = value.chars().next().unwrap_or('*');
        let remaining = value.len();
        let stars = "*".repeat(remaining.saturating_sub(1));
        write!(f, "Confidential({first_char}{stars})")
    }
}

#[derive(From, Debug, Clone)]
pub struct OAuthCredential {
    pub expires: f64,
    pub access_token: ConfidentialText,
    pub refresh_token: Option<ConfidentialText>,
}

#[derive(From, Debug, Clone)]
pub struct JwtCredential {
    pub expires: f64,
    pub token: ConfidentialText,
}

#[derive(From, Debug, Clone)]
pub struct SessionCredential {
    pub expires: f64,
    pub session_id: String,
    pub token: ConfidentialText,
    pub cookie: Option<Cookie>,
}

#[derive(From, Debug)]
pub enum Authenticated {
    OAuth(OAuthCredential),
    JWt(JwtCredential),
    Session(SessionCredential),
}

#[derive(From, Debug)]
pub enum AuthCredential {
    OAuth(OAuthCredential),
    SecretOnly(ConfidentialText),

    EmailAuth {
        email: String,
    },

    UsernameAndPassword {
        username: String,
        password: ConfidentialText,
    },

    ClientSecret {
        client_id: ConfidentialText,
        client_secret: ConfidentialText,
    },
}

#[derive(From)]
pub enum AuthenticationErrors {
    InvalidCredentials,
    RequestErrors,
    FailedToConnect,
    InvalidEndpoint,
    /// Token has expired.
    TokenExpired,
    /// Token refresh failed.
    RefreshFailed,
    /// OAuth-specific error details.
    OAuthError {
        error: String,
        description: Option<String>,
    },
    /// Invalid OAuth state parameter (CSRF mismatch).
    InvalidState,
    /// PKCE generation or validation failed.
    PKCEFailed,
    /// Session not found.
    SessionNotFound,
    /// Account is locked due to too many failed attempts.
    AccountLocked,
    /// Two-factor authentication error.
    TwoFactor(TwoFactorError),
    /// Session management error.
    Session(SessionError),
    /// Auth state machine error.
    AuthState(AuthStateError),
    /// OAuth error.
    OAuth(#[from] OAuthError),
    /// JWT error.
    Jwt(#[from] JwtError),
    /// Credential store error.
    CredentialStore(#[from] CredentialStoreError),
}

impl core::fmt::Display for AuthenticationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthenticationErrors::InvalidCredentials => write!(f, "Invalid credentials"),
            AuthenticationErrors::RequestErrors => write!(f, "Request errors"),
            AuthenticationErrors::FailedToConnect => write!(f, "Failed to connect"),
            AuthenticationErrors::InvalidEndpoint => write!(f, "Invalid endpoint"),
            AuthenticationErrors::TokenExpired => write!(f, "Token expired"),
            AuthenticationErrors::RefreshFailed => write!(f, "Token refresh failed"),
            AuthenticationErrors::OAuthError { error, description } => {
                write!(f, "OAuth error: {error}")?;
                if let Some(desc) = description {
                    write!(f, " ({desc})")?;
                }
                Ok(())
            }
            AuthenticationErrors::InvalidState => write!(f, "Invalid OAuth state"),
            AuthenticationErrors::PKCEFailed => write!(f, "PKCE generation failed"),
            AuthenticationErrors::SessionNotFound => write!(f, "Session not found"),
            AuthenticationErrors::AccountLocked => write!(f, "Account locked"),
            AuthenticationErrors::TwoFactor(e) => write!(f, "Two-factor auth error: {e}"),
            AuthenticationErrors::Session(e) => write!(f, "Session error: {e}"),
            AuthenticationErrors::AuthState(e) => write!(f, "Auth state error: {e}"),
            AuthenticationErrors::OAuth(e) => write!(f, "OAuth error: {e}"),
            AuthenticationErrors::Jwt(e) => write!(f, "JWT error: {e}"),
            AuthenticationErrors::CredentialStore(e) => write!(f, "Credential store error: {e}"),
        }
    }
}

impl core::fmt::Debug for AuthenticationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        core::fmt::Display::fmt(self, f)
    }
}

impl Error for AuthenticationErrors {}

pub type AuthenticationResult<T> = std::result::Result<T, AuthenticationErrors>;

#[derive(From, Debug, Clone)]
pub enum OnAuthData {
    OAuth {
        url: ConfidentialText,
        instructions: Option<String>,
    },
    OAuthAuthorizationRequired {
        url: ConfidentialText,
        state: String,
    },
    OnTwoFactor {
        // location to send two factor token into.
        url: ConfidentialText,
    },
}

#[derive(From, Debug, Clone)]
pub enum AuthenticationStates {
    Connecting,
    Prompting(Option<OnAuthData>),
    Progressing(Option<String>),
    Done,
    Aborted,
}

pub trait AuthProviderEndpoint {
    /// Attempts to log in using the provided credentials.
    ///
    /// # Errors
    ///
    /// Returns an [`AuthenticationErrors`] if authentication fails or an error occurs during the process.
    fn login<T>(&self, credential: AuthCredential) -> AuthenticationResult<T>
    where
        T: StreamIterator<D = Authenticated, P = AuthenticationStates>;
}

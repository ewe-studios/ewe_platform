use std::error::Error;

use derive_more::From;
use foundation_core::{valtron::StreamIterator, wire::simple_http::client::Cookie};
use zeroize::Zeroizing;

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

#[derive(From, Debug, Clone)]
pub enum Authenticated {
    OAuth(OAuthCredential),
    JWt(JwtCredential),
    Session(SessionCredential),
}

#[derive(From, Debug, Clone)]
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
}

impl core::fmt::Display for AuthenticationErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthenticationErrors::InvalidCredentials => write!(f, "Invalid credentials"),
            AuthenticationErrors::RequestErrors => write!(f, "Request errors"),
            AuthenticationErrors::FailedToConnect => write!(f, "Failed to connect"),
            AuthenticationErrors::InvalidEndpoint => write!(f, "Invalid endpoint"),
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

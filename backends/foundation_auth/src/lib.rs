use derive_more::From;
use zeroize::Zeroizing;

#[derive(From, Clone)]
pub struct Confidential(Zeroizing<String>);

impl Confidential {
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    #[must_use]
    pub fn get(&self) -> String {
        (*self.0).clone()
    }
}

impl core::fmt::Debug for Confidential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:}")
    }
}

impl core::fmt::Display for Confidential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = &(*self.0);
        let first_char = value.chars().next().unwrap_or('*');
        let remaining = value.len();
        let stars = "*".repeat(remaining.saturating_sub(1));
        write!(f, "Confidential({first_char}{stars})")
    }
}

#[derive(From, Debug, Clone)]
pub struct AuthenticatedCredentials {
    pub expires: f64,
    pub access_token: Confidential,
    pub refresh_token: Option<Confidential>,
}

#[derive(From, Debug, Clone)]
pub enum AuthCredential {
    SecretOnly(Confidential),

    EmailAuth {
        email: String,
    },

    UsernameAndPassword {
        username: String,
        password: Confidential,
    },

    OAuthAuth(AuthenticatedCredentials),

    ClientSecret {
        client_id: Confidential,
        client_secret: Confidential,
    },
}

pub trait OAuthProviderEndpoint {
    fn login(&self, credential: AuthCredential);
}

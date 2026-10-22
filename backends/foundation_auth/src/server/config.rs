//! IdP server configuration.

use std::time::Duration;

use crate::shared::jwt::JwtSigningKey;

#[derive(Debug, Clone)]
pub struct PasswordPolicy {
    pub min_length: usize,
    pub require_uppercase: bool,
    pub require_lowercase: bool,
    pub require_number: bool,
    pub require_special: bool,
    pub max_failed_attempts: u32,
    pub lockout_duration: Duration,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 12,
            require_uppercase: true,
            require_lowercase: true,
            require_number: true,
            require_special: true,
            max_failed_attempts: 5,
            lockout_duration: Duration::from_secs(900),
        }
    }
}

pub struct IdpConfig {
    pub issuer_url: String,
    pub signing_key: JwtSigningKey,
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration,
    pub session_ttl: Duration,
    pub auth_code_ttl: Duration,
    pub device_code_ttl: Duration,
    pub device_code_interval: u32,
    pub require_pkce: bool,
    pub password_policy: PasswordPolicy,
    /// Largest request body the IdP endpoints read, in bytes. IdP requests
    /// are small forms and JSON documents that must be parsed whole; anything
    /// larger is rejected with 413 before more of it is read. Default 1 MiB,
    /// nginx's `client_max_body_size` default.
    pub max_request_body_bytes: usize,
}

/// Default for [`IdpConfig::max_request_body_bytes`]: 1 MiB (nginx's
/// `client_max_body_size` default).
pub const DEFAULT_MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;

impl IdpConfig {
    #[must_use]
    pub fn new(issuer_url: String) -> Self {
        Self {
            issuer_url,
            signing_key: JwtSigningKey::generate_ed25519(),
            access_token_ttl: Duration::from_secs(900),
            refresh_token_ttl: Duration::from_secs(604_800),
            session_ttl: Duration::from_secs(28_800),
            auth_code_ttl: Duration::from_secs(600),
            device_code_ttl: Duration::from_secs(600),
            device_code_interval: 5,
            require_pkce: true,
            password_policy: PasswordPolicy::default(),
            max_request_body_bytes: DEFAULT_MAX_REQUEST_BODY_BYTES,
        }
    }

    #[must_use]
    pub fn with_signing_key(mut self, key: JwtSigningKey) -> Self {
        self.signing_key = key;
        self
    }

    #[must_use]
    pub fn with_access_token_ttl(mut self, ttl: Duration) -> Self {
        self.access_token_ttl = ttl;
        self
    }

    #[must_use]
    pub fn with_refresh_token_ttl(mut self, ttl: Duration) -> Self {
        self.refresh_token_ttl = ttl;
        self
    }

    #[must_use]
    pub fn with_session_ttl(mut self, ttl: Duration) -> Self {
        self.session_ttl = ttl;
        self
    }

    #[must_use]
    pub fn with_require_pkce(mut self, require: bool) -> Self {
        self.require_pkce = require;
        self
    }

    #[must_use]
    pub fn with_password_policy(mut self, policy: PasswordPolicy) -> Self {
        self.password_policy = policy;
        self
    }

    /// Set the largest request body the IdP endpoints read (see
    /// [`max_request_body_bytes`](Self::max_request_body_bytes)).
    #[must_use]
    pub fn with_max_request_body_bytes(mut self, bytes: usize) -> Self {
        self.max_request_body_bytes = bytes;
        self
    }
}

impl core::fmt::Debug for IdpConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IdpConfig")
            .field("issuer_url", &self.issuer_url)
            .field("access_token_ttl", &self.access_token_ttl)
            .field("refresh_token_ttl", &self.refresh_token_ttl)
            .field("session_ttl", &self.session_ttl)
            .field("require_pkce", &self.require_pkce)
            .field("max_request_body_bytes", &self.max_request_body_bytes)
            .finish_non_exhaustive()
    }
}

//! `IdP` server configuration.

use std::time::Duration;

use crate::shared::jwt::JwtSigningKey;

// One public flag per password rule; grouping them would change the config API.
#[allow(clippy::struct_excessive_bools)]
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

/// Keys that encrypt TOTP secrets at rest (ChaCha20-Poly1305, 256-bit keys).
///
/// The keys come from the `IdP` configuration (for example from an environment
/// variable or a secret store read by the application); they are never in the
/// code. Each stored ciphertext carries the id of the key that made it, so you
/// can rotate: make the new key the active one with
/// [`TotpKeyRing::with_active`] and keep the old one with
/// [`TotpKeyRing::with_retired`] until every secret is re-enrolled.
///
/// With no active key, TOTP enrollment is off and fails closed.
#[derive(Clone, Default)]
pub struct TotpKeyRing {
    active: Option<String>,
    keys: std::collections::HashMap<String, zeroize::Zeroizing<[u8; 32]>>,
}

impl TotpKeyRing {
    /// A ring with one active key.
    #[must_use]
    pub fn new(key_id: impl Into<String>, key: [u8; 32]) -> Self {
        Self::default().with_active(key_id, key)
    }

    /// Add a key and make it the one new secrets are encrypted with.
    #[must_use]
    pub fn with_active(mut self, key_id: impl Into<String>, key: [u8; 32]) -> Self {
        let id = key_id.into();
        self.keys.insert(id.clone(), zeroize::Zeroizing::new(key));
        self.active = Some(id);
        self
    }

    /// Add a key that can only decrypt (an earlier active key).
    #[must_use]
    pub fn with_retired(mut self, key_id: impl Into<String>, key: [u8; 32]) -> Self {
        self.keys
            .insert(key_id.into(), zeroize::Zeroizing::new(key));
        self
    }

    /// The id and bytes of the key new secrets are encrypted with, if any.
    #[must_use]
    pub fn active(&self) -> Option<(&str, &[u8; 32])> {
        let id = self.active.as_deref()?;
        self.keys.get(id).map(|k| (id, &**k))
    }

    /// The key with this id (active or retired), if the ring holds it.
    #[must_use]
    pub fn get(&self, key_id: &str) -> Option<&[u8; 32]> {
        self.keys.get(key_id).map(|k| &**k)
    }
}

impl core::fmt::Debug for TotpKeyRing {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut ids: Vec<&String> = self.keys.keys().collect();
        ids.sort();
        f.debug_struct("TotpKeyRing")
            .field("active", &self.active)
            .field("key_ids", &ids)
            .finish_non_exhaustive()
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
    /// Largest request body the `IdP` endpoints read, in bytes. `IdP` requests
    /// are small forms and JSON documents that must be parsed whole; anything
    /// larger is rejected with 413 before more of it is read. Default 1 MiB,
    /// nginx's `client_max_body_size` default.
    pub max_request_body_bytes: usize,
    /// Development mode. Default `false`. When `true`, the `IdP` serves
    /// `POST /auth/v1/dev/register`, a registration route without the
    /// proof-of-work step. Never turn it on in production.
    pub dev_mode: bool,
    /// How long a `ToS` code (from a `206 tos_required` login answer) stays
    /// valid. Default 10 minutes, the same as an authorization code
    /// ([`auth_code_ttl`](Self::auth_code_ttl)).
    pub tos_code_ttl: Duration,
    /// How long an MFA challenge (from a `200 mfa_required` login answer) stays
    /// valid. Default 5 minutes, the period of a typical one-time-code prompt.
    pub mfa_challenge_ttl: Duration,
    /// Keys that encrypt TOTP secrets at rest (see [`TotpKeyRing`]). Default:
    /// empty, which turns TOTP enrollment off.
    pub totp_keys: TotpKeyRing,
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
            dev_mode: false,
            tos_code_ttl: Duration::from_secs(600),
            mfa_challenge_ttl: Duration::from_secs(300),
            totp_keys: TotpKeyRing::default(),
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

    /// Turn development mode on or off (see [`dev_mode`](Self::dev_mode)).
    #[must_use]
    pub fn with_dev_mode(mut self, dev_mode: bool) -> Self {
        self.dev_mode = dev_mode;
        self
    }

    /// Set the life of a `ToS` code (see [`tos_code_ttl`](Self::tos_code_ttl)).
    #[must_use]
    pub fn with_tos_code_ttl(mut self, ttl: Duration) -> Self {
        self.tos_code_ttl = ttl;
        self
    }

    /// Set the keys that encrypt TOTP secrets (see [`TotpKeyRing`]).
    #[must_use]
    pub fn with_totp_keys(mut self, keys: TotpKeyRing) -> Self {
        self.totp_keys = keys;
        self
    }

    /// Set the life of an MFA challenge (see
    /// [`mfa_challenge_ttl`](Self::mfa_challenge_ttl)).
    #[must_use]
    pub fn with_mfa_challenge_ttl(mut self, ttl: Duration) -> Self {
        self.mfa_challenge_ttl = ttl;
        self
    }

    /// Set the largest request body the `IdP` endpoints read (see
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
            .field("dev_mode", &self.dev_mode)
            .field("tos_code_ttl", &self.tos_code_ttl)
            .field("mfa_challenge_ttl", &self.mfa_challenge_ttl)
            .field("totp_keys", &self.totp_keys)
            .finish_non_exhaustive()
    }
}

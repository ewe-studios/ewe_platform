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

// One public flag per setting; grouping them would change the config API.
#[allow(clippy::struct_excessive_bools)]
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
    /// Mark the session cookies `Secure` (sent over HTTPS only). Default
    /// `true`. Turn it off only for local development over plain HTTP.
    pub secure_cookies: bool,
    /// How long a started TOTP enrollment waits for its first code. Default
    /// 5 minutes. A new start replaces the pending enrollment.
    pub totp_enrollment_ttl: Duration,
    /// The hash new TOTP secrets use. Default SHA-1 (RFC 6238, and the only
    /// one most authenticator apps support). A stored secret keeps the hash it
    /// was enrolled with.
    pub totp_algorithm: crate::shared::two_factor::TotpAlgorithm,
    /// How often the server deletes expired short-lived entries (`ToS` codes,
    /// MFA challenges, `PoW` challenges) from the cache; see
    /// `IdpServer::start_sweeper`. Default 5 minutes.
    pub sweep_interval: Duration,
    /// Accept a cache whose `take` is not atomic (for example Workers KV).
    /// Default `false`: `IdpServer::try_new` refuses such a cache, because a
    /// `ToS` code or MFA challenge in it could be used twice by two requests at
    /// the same time. Use D1 or a Durable Object for single-use data instead;
    /// turn this on only if you accept that risk.
    pub allow_non_atomic_cache: bool,
    /// The page where a user accepts the Terms of Service during a social
    /// login. When a social login ends with an unaccepted `ToS`, the callback
    /// answers `302` to this URL with `#tos_await_code=...&tos_version=...` in
    /// the fragment (a fragment is never sent to a server or logged). The page
    /// posts the code to `POST /auth/v1/tos/accept`. With `None` (default) the
    /// callback answers `206` with the JSON body instead, which suits an API
    /// client, not a browser.
    pub tos_page_url: Option<String>,
    /// The key that signs session tokens (at least 32 bytes). The default is
    /// random per `IdpConfig`, so sessions in a persistent store stop being valid
    /// when the process restarts: **set a stable key from your secret store**
    /// ([`with_session_key`](Self::with_session_key)) for sessions that survive
    /// a restart or that several instances share.
    pub session_key: Vec<u8>,
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
            secure_cookies: true,
            totp_enrollment_ttl: Duration::from_secs(300),
            totp_algorithm: crate::shared::two_factor::TotpAlgorithm::default(),
            sweep_interval: Duration::from_secs(300),
            allow_non_atomic_cache: false,
            tos_page_url: None,
            session_key: {
                use foundation_compact::rng::Rng;
                let mut key = vec![0u8; 32];
                foundation_compact::rng::rng().fill_bytes(&mut key);
                key
            },
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

    /// Turn the `Secure` flag of the session cookies on or off (see
    /// [`secure_cookies`](Self::secure_cookies)). Off is for local
    /// development over plain HTTP only.
    #[must_use]
    pub fn with_secure_cookies(mut self, secure: bool) -> Self {
        self.secure_cookies = secure;
        self
    }

    /// Set the key that signs session tokens (see
    /// [`session_key`](Self::session_key)); at least 32 bytes.
    #[must_use]
    pub fn with_session_key(mut self, key: impl Into<Vec<u8>>) -> Self {
        self.session_key = key.into();
        self
    }

    /// Set the page for the `ToS` step of a social login (see
    /// [`tos_page_url`](Self::tos_page_url)).
    #[must_use]
    pub fn with_tos_page_url(mut self, url: impl Into<String>) -> Self {
        self.tos_page_url = Some(url.into());
        self
    }

    /// Accept a cache whose `take` is not atomic (see
    /// [`allow_non_atomic_cache`](Self::allow_non_atomic_cache)).
    #[must_use]
    pub fn with_allow_non_atomic_cache(mut self, allow: bool) -> Self {
        self.allow_non_atomic_cache = allow;
        self
    }

    /// Set how often expired cache entries are deleted (see
    /// [`sweep_interval`](Self::sweep_interval)).
    #[must_use]
    pub fn with_sweep_interval(mut self, interval: Duration) -> Self {
        self.sweep_interval = interval;
        self
    }

    /// Set the hash new TOTP secrets use (see
    /// [`totp_algorithm`](Self::totp_algorithm)).
    #[must_use]
    pub fn with_totp_algorithm(
        mut self,
        algorithm: crate::shared::two_factor::TotpAlgorithm,
    ) -> Self {
        self.totp_algorithm = algorithm;
        self
    }

    /// Set how long a started TOTP enrollment waits for its first code (see
    /// [`totp_enrollment_ttl`](Self::totp_enrollment_ttl)).
    #[must_use]
    pub fn with_totp_enrollment_ttl(mut self, ttl: Duration) -> Self {
        self.totp_enrollment_ttl = ttl;
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
            .field("secure_cookies", &self.secure_cookies)
            .field("totp_enrollment_ttl", &self.totp_enrollment_ttl)
            .field("totp_algorithm", &self.totp_algorithm)
            .field("sweep_interval", &self.sweep_interval)
            .field("allow_non_atomic_cache", &self.allow_non_atomic_cache)
            .field("tos_page_url", &self.tos_page_url)
            .finish_non_exhaustive()
    }
}

//! Session management — create, validate, revoke sessions with three-cookie system.
//!
//! WHY: Persistent login sessions with sliding expiration and secure cookies.
//!
//! WHAT: `SessionManager` with session CRUD, sliding/absolute expiration, and
//! three-cookie system (`session_token`, `session_data`, `dont_remember`).
//! HOW: Sessions stored via `CredentialStore`. Token signing via HMAC-SHA256.

use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Duration, Utc};
use foundation_compact::rng::Rng;
use foundation_core::valtron::Clock;
use foundation_netio::shared::client::{Cookie, SameSite};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

use super::types::ConfidentialText;
use crate::shared::clock::wall_now_utc;
use crate::shared::credential_store::CredentialStore;
use crate::shared::credential_store::CredentialStoreError;

type HmacSha256 = Hmac<Sha256>;

/// Session configuration.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// How long sessions last before requiring re-auth (absolute max).
    pub max_session_age: Duration,
    /// How long before expiry we extend the session (sliding window).
    pub sliding_window: Duration,
    /// Cookie name for the session token.
    pub token_cookie_name: String,
    /// Cookie name for the session data cache.
    pub data_cookie_name: String,
    /// Cookie name for the "don't remember me" flag.
    pub dont_remember_cookie_name: String,
    /// Enable sliding expiration (auto-extend on activity).
    pub sliding_expiration: bool,
    /// Mark the session cookies `Secure` (sent over HTTPS only). Default
    /// `true`. Turn it off only for local development over plain HTTP.
    pub secure_cookies: bool,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_session_age: Duration::days(7),
            sliding_window: Duration::minutes(5),
            token_cookie_name: "session_token".to_string(),
            data_cookie_name: "session_data".to_string(),
            dont_remember_cookie_name: "dont_remember".to_string(),
            sliding_expiration: true,
            secure_cookies: true,
        }
    }
}

/// Session record stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session ID.
    pub id: String,
    /// User ID associated with this session.
    pub user_id: String,
    /// Session token (signed).
    pub token: ConfidentialText,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When the session expires.
    pub expires_at: DateTime<Utc>,
    /// IP address at creation.
    pub ip_address: Option<String>,
    /// User agent at creation.
    pub user_agent: Option<String>,
    /// When the session was last active.
    pub last_active_at: DateTime<Utc>,
    /// Whether this session has been revoked.
    pub revoked: bool,
}

impl Session {
    /// Check if the session is valid (not revoked, not expired).
    #[must_use]
    pub fn is_valid(&self, clock: &dyn Clock) -> bool {
        !self.revoked && wall_now_utc(clock) < self.expires_at
    }
}

/// Session cache for the `session_data` cookie.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionDataCache {
    user_id: String,
    expires_at: i64,
}

/// Session manager — create, validate, revoke sessions.
pub struct SessionManager<S> {
    store: S,
    config: SessionConfig,
    signer: Arc<Mutex<TokenSigner>>,
    clock: Arc<dyn Clock>,
}

impl<S> SessionManager<S> {
    /// Borrow the underlying credential store.
    #[must_use]
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Get the session configuration.
    #[must_use]
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Create a new session manager with one signing key (key id
    /// [`DEFAULT_KEY_ID`]). Use [`SessionManager::with_key_ring`] to rotate keys.
    ///
    /// # Errors
    ///
    /// Returns `SessionError` if the signing key is shorter than 32 bytes.
    pub fn new(
        store: S,
        config: SessionConfig,
        signing_key: &[u8],
        clock: Arc<dyn Clock>,
    ) -> Result<Self, SessionError> {
        Ok(Self::with_key_ring(
            store,
            config,
            KeyRing::single(signing_key)?,
            clock,
        ))
    }

    /// Create a new session manager with a key ring (key rotation).
    #[must_use]
    pub fn with_key_ring(
        store: S,
        config: SessionConfig,
        keys: KeyRing,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            store,
            config,
            signer: Arc::new(Mutex::new(TokenSigner::new(keys))),
            clock,
        }
    }

    /// Replace the key ring at run time (key rotation).
    ///
    /// Sessions signed with a key that is in the new ring stay valid.
    ///
    /// # Panics
    ///
    /// Panics if the signer lock is poisoned.
    pub fn rotate_keys(&self, keys: KeyRing) {
        *self.signer.lock().expect("signer lock") = TokenSigner::new(keys);
    }

    #[allow(clippy::cast_sign_loss)]
    fn make_cookies(&self, token: &str, user_id: &str) -> Result<Vec<Cookie>, SessionError> {
        let mut cookies = Vec::new();

        let token_cookie = Cookie::new(&self.config.token_cookie_name, token)
            .path("/")
            .max_age(StdDuration::from_secs(
                self.config.max_session_age.num_seconds() as u64,
            ))
            .http_only(true)
            .secure(self.config.secure_cookies)
            .same_site(SameSite::Lax);
        cookies.push(token_cookie);

        let cache = SessionDataCache {
            user_id: user_id.to_string(),
            expires_at: (wall_now_utc(self.clock.as_ref()) + Duration::minutes(5)).timestamp(),
        };
        let cache_json = serde_json::to_string(&cache)
            .map_err(|e| SessionError::Serialization(e.to_string()))?;
        // A cookie value may not hold `,`, `;`, `"` or spaces: the JSON is
        // base64url-encoded (the raw JSON was split by HTTP header parsers).
        let data_cookie = Cookie::new(
            &self.config.data_cookie_name,
            &URL_SAFE_NO_PAD.encode(cache_json.as_bytes()),
        )
        .path("/")
        .max_age(StdDuration::from_secs(300))
        .secure(self.config.secure_cookies)
        .same_site(SameSite::Lax);
        cookies.push(data_cookie);

        if !self.config.sliding_expiration {
            let dont_remember = Cookie::new(&self.config.dont_remember_cookie_name, "1")
                .path("/")
                .secure(self.config.secure_cookies)
                .same_site(SameSite::Lax);
            cookies.push(dont_remember);
        }

        Ok(cookies)
    }
}

// ============================================================================
// Session methods — over `CredentialStore`.
// ============================================================================

impl<S: CredentialStore> SessionManager<S> {
    /// Create a new session for a user (async).
    ///
    /// # Errors
    ///
    /// Returns `SessionError::Storage` if the session cannot be stored.
    /// Returns `SessionError::Serialization` if the session data cannot be serialized.
    ///
    /// # Panics
    ///
    /// Panics if the signer lock is poisoned. The `expect("signer lock")` call
    /// fails then.
    pub async fn create_session(
        &self,
        user_id: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<(Session, Vec<Cookie>), SessionError> {
        let token = generate_token();
        let now = wall_now_utc(self.clock.as_ref());
        let expires_at = now + self.config.max_session_age;
        let session_id = session_key(&token);

        let signed_token = self.signer.lock().expect("signer lock").sign(&token);

        let session = Session {
            id: session_id.clone(),
            user_id: user_id.to_string(),
            token: ConfidentialText::new(signed_token),
            created_at: now,
            expires_at,
            ip_address: ip_address.map(String::from),
            user_agent: user_agent.map(String::from),
            last_active_at: now,
            revoked: false,
        };

        self.store
            .set(&session_id, session.clone())
            .await
            .map_err(SessionError::Storage)?;

        // Index entry for "revoke all sessions of a user". One key per session,
        // so two sessions created at the same time never overwrite each other.
        self.store
            .set(&user_index_key(user_id, &session_id), session_id.clone())
            .await
            .map_err(SessionError::Storage)?;

        let cookies = self.make_cookies(&token, user_id)?;

        Ok((session, cookies))
    }

    /// Get and validate a session by token (async).
    ///
    /// # Errors
    ///
    /// Returns `SessionError::Storage` if the store fails.
    /// Returns `SessionError::InvalidToken` if the token signature is invalid.
    ///
    /// # Panics
    ///
    /// Panics if the signer lock is poisoned. The `expect("signer lock")` call
    /// fails then.
    pub async fn get_session(&self, token: &str) -> Result<Option<Session>, SessionError> {
        // The store key is derived from the token: one `get`, no listing.
        let Some(session) = self
            .store
            .get::<Session>(&session_key(token))
            .await
            .map_err(SessionError::Storage)?
        else {
            return Ok(None);
        };
        // The stored signed token must also verify, so a key that left the
        // ring (see [`KeyRing`]) ends the sessions it signed.
        if session.revoked
            || !self
                .signer
                .lock()
                .expect("signer lock")
                .verify(&session.token.get(), token)
        {
            return Ok(None);
        }
        if !session.is_valid(self.clock.as_ref()) {
            return Ok(None);
        }
        if self.config.sliding_expiration {
            self.extend_session(&session).await?;
        }
        Ok(Some(session))
    }

    /// Revoke a single session (async).
    ///
    /// # Errors
    ///
    /// Returns `SessionError::Storage` if the store fails.
    pub async fn revoke_session(&self, session_id: &str) -> Result<(), SessionError> {
        if let Some(session) = self
            .store
            .get::<Session>(session_id)
            .await
            .map_err(SessionError::Storage)?
        {
            if session.revoked {
                return Ok(());
            }
            let mut updated = session;
            updated.revoked = true;
            let sid = updated.id.clone();
            self.store
                .set(&sid, updated)
                .await
                .map_err(SessionError::Storage)?;
        }
        Ok(())
    }

    /// Revoke all sessions for a user (async).
    ///
    /// # Errors
    ///
    /// Returns `SessionError::Storage` if the store fails.
    pub async fn revoke_all_sessions(&self, user_id: &str) -> Result<usize, SessionError> {
        // Index entries hold the session ids of this user. Keys of the old
        // format (`session:{user_id}:{prefix}`) are revoked too, so no old
        // session stays alive.
        let mut ids = Vec::new();
        for index_key in self
            .store
            .list_keys(Some(&format!("user_session:{user_id}:")))
            .await
            .map_err(SessionError::Storage)?
        {
            if let Some(id) = self
                .store
                .get::<String>(&index_key)
                .await
                .map_err(SessionError::Storage)?
            {
                ids.push(id);
            }
        }
        ids.extend(
            self.store
                .list_keys(Some(&format!("session:{user_id}:")))
                .await
                .map_err(SessionError::Storage)?,
        );

        let mut count = 0;
        for key in &ids {
            if let Some(session) = self
                .store
                .get::<Session>(key)
                .await
                .map_err(SessionError::Storage)?
            {
                if !session.revoked && session.user_id == user_id {
                    let mut updated = session;
                    updated.revoked = true;
                    let sid = updated.id.clone();
                    self.store
                        .set(&sid, updated)
                        .await
                        .map_err(SessionError::Storage)?;
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    /// Extend session expiration (async, sliding expiration).
    async fn extend_session(&self, session: &Session) -> Result<(), SessionError> {
        let now = wall_now_utc(self.clock.as_ref());
        let time_left = session.expires_at - now;
        if time_left < self.config.sliding_window {
            let mut updated = session.clone();
            updated.last_active_at = now;
            updated.expires_at = now + self.config.max_session_age;
            let sid = session.id.clone();
            self.store
                .set(&sid, updated)
                .await
                .map_err(SessionError::Storage)?;
        }
        Ok(())
    }
}

/// Longest key id, in bytes.
///
/// The id is stored in every token, so a short id keeps the cookie small.
pub const MAX_KEY_ID_LEN: usize = 16;

/// Key id the single-key convenience constructors use.
pub const DEFAULT_KEY_ID: &str = "0";

/// One named HMAC key.
///
/// The key bytes are zeroized when the value drops and never appear in
/// `Debug` output.
#[derive(Clone)]
pub struct SigningKey {
    id: String,
    key: Zeroizing<Vec<u8>>,
}

impl SigningKey {
    /// Create a named key.
    ///
    /// The id must be 1 to [`MAX_KEY_ID_LEN`] characters from `A-Z a-z 0-9 - _`
    /// (it cannot hold a `.`, because `.` separates the token parts).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidSigningKey`] if the key is shorter than 32
    /// bytes or the id is not valid.
    pub fn new(id: &str, key: &[u8]) -> Result<Self, SessionError> {
        let id_ok = !id.is_empty()
            && id.len() <= MAX_KEY_ID_LEN
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if key.len() < 32 || !id_ok {
            return Err(SessionError::InvalidSigningKey);
        }
        Ok(Self {
            id: id.to_string(),
            key: Zeroizing::new(key.to_vec()),
        })
    }

    /// The key id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    fn mac(&self, plain: &str) -> HmacSha256 {
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("HMAC can take key of any size");
        mac.update(plain.as_bytes());
        mac
    }
}

impl core::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SigningKey")
            .field("id", &self.id)
            .field("key", &"<redacted>")
            .finish()
    }
}

/// The keys a [`TokenSigner`] uses: one **current** key that signs and verifies,
/// and zero or more **previous** keys that only verify.
///
/// To rotate: make a new key current and move the old current key to the
/// previous list. Sessions signed with the old key stay valid. Remove the old
/// key from the ring when those sessions have expired (after
/// [`SessionConfig::max_session_age`]).
///
/// `allow_legacy_tokens` (default `true`) is the migration window for tokens
/// made before key ids existed (`token.sig`, two parts). They verify with the
/// current key. Set it to `false` once all such sessions have expired.
#[derive(Clone, Debug)]
pub struct KeyRing {
    current: SigningKey,
    previous: Vec<SigningKey>,
    allow_legacy_tokens: bool,
}

impl KeyRing {
    /// A ring with only a current key.
    #[must_use]
    pub fn new(current: SigningKey) -> Self {
        Self {
            current,
            previous: Vec::new(),
            allow_legacy_tokens: true,
        }
    }

    /// A ring from one raw key, with the id [`DEFAULT_KEY_ID`].
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidSigningKey`] if the key is shorter than 32 bytes.
    pub fn single(key: &[u8]) -> Result<Self, SessionError> {
        Ok(Self::new(SigningKey::new(DEFAULT_KEY_ID, key)?))
    }

    /// Add a previous key (verify only).
    ///
    /// # Errors
    ///
    /// Returns [`SessionError::InvalidSigningKey`] if the id is already in the ring.
    pub fn with_previous(mut self, key: SigningKey) -> Result<Self, SessionError> {
        if key.id == self.current.id || self.previous.iter().any(|k| k.id == key.id) {
            return Err(SessionError::InvalidSigningKey);
        }
        self.previous.push(key);
        Ok(self)
    }

    /// Accept (`true`, default) or reject (`false`) tokens without a key id.
    #[must_use]
    pub fn allow_legacy_tokens(mut self, allow: bool) -> Self {
        self.allow_legacy_tokens = allow;
        self
    }

    /// The id of the key that signs new tokens.
    #[must_use]
    pub fn current_id(&self) -> &str {
        &self.current.id
    }

    fn find(&self, kid: &str) -> Option<&SigningKey> {
        if self.current.id == kid {
            return Some(&self.current);
        }
        self.previous.iter().find(|k| k.id == kid)
    }
}

/// HMAC-SHA256 token signer for session tokens, with key rotation.
///
/// Token format: `{token}.{kid}.{tag}`
///
/// - `token`: the random session token (base64url, no `.`).
/// - `kid`: the id of the signing key (see [`SigningKey::new`]).
/// - `tag`: `base64url(HMAC-SHA256(key, token))`, no padding.
///
/// The old format `{token}.{tag}` (no key id) still verifies with the current key
/// while [`KeyRing::allow_legacy_tokens`] is `true`.
pub struct TokenSigner {
    ring: KeyRing,
}

impl TokenSigner {
    /// Create a signer from a key ring.
    #[must_use]
    pub fn new(keys: KeyRing) -> Self {
        Self { ring: keys }
    }

    /// Sign a raw session token with the current key, returning
    /// `"{token}.{kid}.{base64(signature)}"`.
    #[must_use]
    pub fn sign(&self, token: &str) -> String {
        let result = self.ring.current.mac(token).finalize().into_bytes();
        format!(
            "{token}.{}.{}",
            self.ring.current.id,
            URL_SAFE_NO_PAD.encode(result)
        )
    }

    /// Verify a signed token against its plain text.
    ///
    /// The key is chosen by the key id in the signed token. The tag comparison
    /// runs in constant time through [`Mac::verify_slice`], which also compares
    /// the lengths first, so a truncated tag is rejected.
    ///
    /// Every failure (wrong format, unknown key id, bad base64, wrong tag, legacy
    /// token while legacy tokens are off) returns `false`. The caller cannot tell
    /// which check failed, so a key id does not show which keys exist.
    #[must_use]
    pub fn verify(&self, signed: &str, plain: &str) -> bool {
        let parts: Vec<&str> = signed.split('.').collect();
        let (key, tag) = match parts.as_slice() {
            [_, tag] if self.ring.allow_legacy_tokens => (Some(&self.ring.current), *tag),
            [_, kid, tag] => (self.ring.find(kid), *tag),
            _ => (None, ""),
        };

        // Always do one HMAC and one compare, so an unknown key id takes
        // about as long as a wrong tag. A bad base64 tag becomes an empty tag,
        // which `verify_slice` rejects on length.
        let usable = key.is_some();
        let key = key.unwrap_or(&self.ring.current);
        let tag_bytes = URL_SAFE_NO_PAD.decode(tag).unwrap_or_default();
        let matched = key.mac(plain).verify_slice(&tag_bytes).is_ok();
        matched && usable
    }
}

/// Store key of a session: `session:{base64url(SHA-256(token))}`.
///
/// The key holds a hash, never the token, so a read of the store does not give
/// out a usable token. Keys of the old format `session:{user_id}:{prefix}`
/// cannot be found this way: sessions made before this change are logged out
/// (see [`SessionManager::revoke_all_sessions`], which still revokes them).
fn session_key(token: &str) -> String {
    use sha2::Digest;
    format!(
        "session:{}",
        URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
    )
}

/// Index key for [`SessionManager::revoke_all_sessions`].
fn user_index_key(user_id: &str, session_id: &str) -> String {
    format!("user_session:{user_id}:{session_id}")
}

/// Generate a cryptographically random session token.
fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    foundation_compact::rng::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Session management errors.
#[derive(derive_more::From, Debug)]
pub enum SessionError {
    /// Storage backend error.
    Storage(CredentialStoreError),
    /// Signing key too short (minimum 32 bytes).
    InvalidSigningKey,
    /// Invalid or malformed token.
    #[from(ignore)]
    InvalidToken,
    /// Session not found.
    #[from(ignore)]
    SessionNotFound,
    /// Serialization error.
    #[from(ignore)]
    Serialization(String),
}

impl core::fmt::Display for SessionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SessionError::Storage(e) => write!(f, "Storage error: {e}"),
            SessionError::InvalidSigningKey => {
                write!(f, "Signing key too short (minimum 32 bytes)")
            }
            SessionError::InvalidToken => write!(f, "Invalid session token"),
            SessionError::SessionNotFound => write!(f, "Session not found"),
            SessionError::Serialization(s) => write!(f, "Serialization error: {s}"),
        }
    }
}

impl std::error::Error for SessionError {}

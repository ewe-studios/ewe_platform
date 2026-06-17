//! Session management — create, validate, revoke sessions with three-cookie system.
//!
//! WHY: Persistent login sessions with sliding expiration and secure cookies.
//!
//! WHAT: `SessionManager` with session CRUD, sliding/absolute expiration, and
//! three-cookie system (`session_token`, `session_data`, `dont_remember`).
//! HOW: Sessions stored via `CredentialStore`. Token signing via HMAC-SHA256.

use std::sync::{Arc, Mutex};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Duration, Utc};
use foundation_netio::simple_http::client::shared::{Cookie, SameSite};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::shared::credential_store::{AsyncCredentialStore, CredentialStore};
use super::types::ConfidentialText;
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
    pub fn is_valid(&self) -> bool {
        !self.revoked && Utc::now() < self.expires_at
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
}

impl<S: CredentialStore> SessionManager<S> {
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

    /// Create a new session manager.
    ///
    /// # Errors
    ///
    /// Returns `SessionError` if the signer cannot be initialized.
    pub fn new(store: S, config: SessionConfig, signing_key: &[u8]) -> Result<Self, SessionError> {
        Ok(Self {
            store,
            config,
            signer: Arc::new(Mutex::new(TokenSigner::new(signing_key)?)),
        })
    }

    /// Create a new session for a user.
    ///
    /// # Errors
    ///
    /// Returns `SessionError` if the storage operation fails.
    /// # Panics
    /// Panics if the signer mutex is poisoned.
    #[allow(clippy::missing_panics_doc)]
    pub fn create_session(
        &self,
        user_id: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<(Session, Vec<Cookie>), SessionError> {
        let token = generate_token();
        let now = Utc::now();
        let expires_at = now + self.config.max_session_age;
        let token_prefix = token[..8].to_string();
        let session_id = format!("session:{user_id}:{token_prefix}");

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
            .map_err(SessionError::Storage)?;

        let cookies = self.make_cookies(&token, user_id)?;

        Ok((session, cookies))
    }

    /// Get and validate a session by token.
    ///
    /// # Errors
    ///
    /// Returns `SessionError` if the storage operation fails.
    /// # Panics
    /// Panics if the signer mutex is poisoned.
    #[allow(clippy::missing_panics_doc)]
    pub fn get_session(&self, token: &str) -> Result<Option<Session>, SessionError> {
        let cache = Self::get_cached_session(token);
        if let Some(ref cache_data) = cache {
            if !cache_data.user_id.is_empty() {
                let token_prefix = token[..token.len().min(8)].to_string();
                let key = format!("session:{}:{}", cache_data.user_id, token_prefix);
                if let Some(session) = self
                    .store
                    .get::<Session>(&key)
                    .map_err(SessionError::Storage)?
                {
                    if session.is_valid() {
                        if self.config.sliding_expiration {
                            self.extend_session(&session)?;
                        }
                        return Ok(Some(session));
                    }
                    return Ok(None);
                }
            }
        }

        let keys = self
            .store
            .list_keys(Some("session:"))
            .map_err(SessionError::Storage)?;
        for key in &keys {
            if let Some(session) = self
                .store
                .get::<Session>(key)
                .map_err(SessionError::Storage)?
            {
                if !session.revoked
                    && self
                        .signer
                        .lock()
                        .expect("signer lock")
                        .verify(&session.token.get(), token)?
                {
                    if session.is_valid() {
                        if self.config.sliding_expiration {
                            self.extend_session(&session)?;
                        }
                        return Ok(Some(session));
                    }
                    return Ok(None);
                }
            }
        }

        Ok(None)
    }

    /// Revoke a single session (sign out).
    ///
    /// # Errors
    ///
    /// Returns `SessionError` if the storage operation fails.
    pub fn revoke_session(&self, session_id: &str) -> Result<(), SessionError> {
        if let Some(session) = self
            .store
            .get::<Session>(session_id)
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
                .map_err(SessionError::Storage)?;
        }
        Ok(())
    }

    /// Revoke all sessions for a user (sign out everywhere).
    ///
    /// # Errors
    ///
    /// Returns `SessionError` if the storage operation fails.
    pub fn revoke_all_sessions(&self, user_id: &str) -> Result<usize, SessionError> {
        let prefix = format!("session:{user_id}:");
        let keys = self
            .store
            .list_keys(Some(&prefix))
            .map_err(SessionError::Storage)?;

        let mut count = 0;
        for key in &keys {
            if let Some(session) = self
                .store
                .get::<Session>(key)
                .map_err(SessionError::Storage)?
            {
                if !session.revoked {
                    let mut updated = session;
                    updated.revoked = true;
                    let sid = updated.id.clone();
                    self.store
                        .set(&sid, updated)
                        .map_err(SessionError::Storage)?;
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    /// Extend session expiration (sliding expiration).
    ///
    /// # Errors
    ///
    /// Returns `SessionError` if the storage operation fails.
    fn extend_session(&self, session: &Session) -> Result<(), SessionError> {
        let now = Utc::now();
        let time_left = session.expires_at - now;
        if time_left < self.config.sliding_window {
            let mut updated = session.clone();
            updated.last_active_at = now;
            updated.expires_at = now + self.config.max_session_age;
            let sid = session.id.clone();
            self.store
                .set(&sid, updated)
                .map_err(SessionError::Storage)?;
        }
        Ok(())
    }

    fn get_cached_session(_token: &str) -> Option<SessionDataCache> {
        None
    }

    #[allow(clippy::cast_sign_loss)]
    fn make_cookies(&self, token: &str, user_id: &str) -> Result<Vec<Cookie>, SessionError> {
        let mut cookies = Vec::new();

        let token_cookie = Cookie::new(&self.config.token_cookie_name, token)
            .path("/")
            .max_age(std::time::Duration::from_secs(
                self.config.max_session_age.num_seconds() as u64,
            ))
            .http_only(true)
            .same_site(SameSite::Lax);
        cookies.push(token_cookie);

        let cache = SessionDataCache {
            user_id: user_id.to_string(),
            expires_at: (Utc::now() + Duration::minutes(5)).timestamp(),
        };
        let cache_json = serde_json::to_string(&cache)
            .map_err(|e| SessionError::Serialization(e.to_string()))?;
        let data_cookie = Cookie::new(&self.config.data_cookie_name, &cache_json)
            .path("/")
            .max_age(std::time::Duration::from_secs(300));
        cookies.push(data_cookie);

        if !self.config.sliding_expiration {
            let dont_remember = Cookie::new(&self.config.dont_remember_cookie_name, "1").path("/");
            cookies.push(dont_remember);
        }

        Ok(cookies)
    }
}

// ============================================================================
// Async methods — require `S: AsyncCredentialStore` in addition to `CredentialStore`.
// These call `*_async` store methods instead of draining sync iterators.
// ============================================================================

impl<S: CredentialStore + AsyncCredentialStore> SessionManager<S> {
    /// Create a new session for a user (async).
    pub async fn create_session_async(
        &self,
        user_id: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<(Session, Vec<Cookie>), SessionError> {
        let token = generate_token();
        let now = Utc::now();
        let expires_at = now + self.config.max_session_age;
        let token_prefix = token[..8].to_string();
        let session_id = format!("session:{user_id}:{token_prefix}");

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
            .set_async(&session_id, session.clone())
            .await
            .map_err(SessionError::Storage)?;

        let cookies = self.make_cookies(&token, user_id)?;

        Ok((session, cookies))
    }

    /// Get and validate a session by token (async).
    pub async fn get_session_async(
        &self,
        token: &str,
    ) -> Result<Option<Session>, SessionError> {
        let cache = Self::get_cached_session(token);
        if let Some(ref cache_data) = cache {
            if !cache_data.user_id.is_empty() {
                let token_prefix = token[..token.len().min(8)].to_string();
                let key = format!("session:{}:{}", cache_data.user_id, token_prefix);
                if let Some(session) = self
                    .store
                    .get_async::<Session>(&key)
                    .await
                    .map_err(SessionError::Storage)?
                {
                    if session.is_valid() {
                        if self.config.sliding_expiration {
                            self.extend_session_async(&session).await?;
                        }
                        return Ok(Some(session));
                    }
                    return Ok(None);
                }
            }
        }

        let keys = self
            .store
            .list_keys_async(Some("session:"))
            .await
            .map_err(SessionError::Storage)?;
        for key in &keys {
            if let Some(session) = self
                .store
                .get_async::<Session>(key)
                .await
                .map_err(SessionError::Storage)?
            {
                if !session.revoked
                    && self
                        .signer
                        .lock()
                        .expect("signer lock")
                        .verify(&session.token.get(), token)?
                {
                    if session.is_valid() {
                        if self.config.sliding_expiration {
                            self.extend_session_async(&session).await?;
                        }
                        return Ok(Some(session));
                    }
                    return Ok(None);
                }
            }
        }

        Ok(None)
    }

    /// Revoke a single session (async).
    pub async fn revoke_session_async(
        &self,
        session_id: &str,
    ) -> Result<(), SessionError> {
        if let Some(session) = self
            .store
            .get_async::<Session>(session_id)
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
                .set_async(&sid, updated)
                .await
                .map_err(SessionError::Storage)?;
        }
        Ok(())
    }

    /// Revoke all sessions for a user (async).
    pub async fn revoke_all_sessions_async(
        &self,
        user_id: &str,
    ) -> Result<usize, SessionError> {
        let prefix = format!("session:{user_id}:");
        let keys = self
            .store
            .list_keys_async(Some(&prefix))
            .await
            .map_err(SessionError::Storage)?;

        let mut count = 0;
        for key in &keys {
            if let Some(session) = self
                .store
                .get_async::<Session>(key)
                .await
                .map_err(SessionError::Storage)?
            {
                if !session.revoked {
                    let mut updated = session;
                    updated.revoked = true;
                    let sid = updated.id.clone();
                    self.store
                        .set_async(&sid, updated)
                        .await
                        .map_err(SessionError::Storage)?;
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    /// Extend session expiration (async, sliding expiration).
    async fn extend_session_async(
        &self,
        session: &Session,
    ) -> Result<(), SessionError> {
        let now = Utc::now();
        let time_left = session.expires_at - now;
        if time_left < self.config.sliding_window {
            let mut updated = session.clone();
            updated.last_active_at = now;
            updated.expires_at = now + self.config.max_session_age;
            let sid = session.id.clone();
            self.store
                .set_async(&sid, updated)
                .await
                .map_err(SessionError::Storage)?;
        }
        Ok(())
    }
}

/// HMAC-SHA256 token signer for session tokens.
struct TokenSigner {
    key: Zeroizing<Vec<u8>>,
}

impl TokenSigner {
    fn new(key: &[u8]) -> Result<Self, SessionError> {
        if key.len() < 32 {
            return Err(SessionError::InvalidSigningKey);
        }
        Ok(Self {
            key: Zeroizing::new(key.to_vec()),
        })
    }

    fn sign(&self, token: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.key).expect("HMAC can take key of any size");
        mac.update(token.as_bytes());
        let result = mac.finalize().into_bytes();
        format!("{token}.{}", URL_SAFE_NO_PAD.encode(result))
    }

    fn verify(&self, signed: &str, plain: &str) -> Result<bool, SessionError> {
        let parts: Vec<&str> = signed.split('.').collect();
        if parts.len() != 2 {
            return Ok(false);
        }

        let mut mac = HmacSha256::new_from_slice(&self.key).expect("HMAC can take key of any size");
        mac.update(plain.as_bytes());
        let expected = mac.finalize().into_bytes();

        let actual = URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|_| SessionError::InvalidToken)?;

        Ok(expected.len() == actual.len() && expected.iter().zip(&actual).all(|(a, b)| a == b))
    }
}

/// Generate a cryptographically random session token.
fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
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


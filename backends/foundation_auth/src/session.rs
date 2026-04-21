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
use foundation_core::wire::simple_http::client::{Cookie, SameSite};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::credential_store::CredentialStore;
use crate::ConfidentialText;
use crate::CredentialStoreError;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credential_store::CredentialStorage;

    fn init_valtron() {
        foundation_core::valtron::single::initialize_pool(42);
    }

    fn make_manager() -> SessionManager<CredentialStorage> {
        init_valtron();
        let store = CredentialStorage::memory();
        let key = vec![0xAB; 32];
        SessionManager::new(store, SessionConfig::default(), &key).expect("create session manager")
    }

    #[test]
    fn test_create_session() {
        let mgr = make_manager();
        let (session, cookies) = mgr
            .create_session("user_1", Some("127.0.0.1"), Some("TestAgent"))
            .unwrap();

        assert_eq!(session.user_id, "user_1");
        assert!(!session.token.get().is_empty());
        assert!(!session.revoked);
        assert!(!cookies.is_empty());
    }

    #[test]
    fn test_revoke_single_session() {
        let mgr = make_manager();
        let (session, _) = mgr.create_session("user_1", None, None).unwrap();

        mgr.revoke_session(&session.id).unwrap();

        let stored: Option<Session> = mgr.store.get(&session.id).unwrap();
        assert!(stored.is_some_and(|s| s.revoked));
    }

    #[test]
    fn test_revoke_all_sessions() {
        let mgr = make_manager();
        mgr.create_session("user_1", None, None).unwrap();
        mgr.create_session("user_1", None, None).unwrap();
        mgr.create_session("user_2", None, None).unwrap();

        let count = mgr.revoke_all_sessions("user_1").unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_session_is_valid() {
        let session = Session {
            id: "test".to_string(),
            user_id: "user_1".to_string(),
            token: ConfidentialText::new("tok".to_string()),
            created_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(1),
            ip_address: None,
            user_agent: None,
            last_active_at: Utc::now(),
            revoked: false,
        };
        assert!(session.is_valid());

        let expired = Session {
            expires_at: Utc::now() - Duration::hours(1),
            ..session.clone()
        };
        assert!(!expired.is_valid());

        let revoked = Session {
            revoked: true,
            ..session.clone()
        };
        assert!(!revoked.is_valid());
    }

    #[test]
    fn test_signing_key_too_short() {
        let store = CredentialStorage::memory();
        let result = SessionManager::new(store, SessionConfig::default(), b"short");
        assert!(result.is_err());
    }

    #[test]
    fn test_token_signer_verify() {
        let key = vec![0xCD; 32];
        let signer = TokenSigner::new(&key).unwrap();
        let plain = "abc123";
        let signed = signer.sign(plain);
        assert!(signer.verify(&signed, plain).unwrap());
        assert!(!signer.verify(&signed, "wrong").unwrap());
    }
}

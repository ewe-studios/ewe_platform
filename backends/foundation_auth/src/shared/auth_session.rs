//! Upstream authentication session store (spec-57, F004).
//!
//! WHY: Social login is a redirect-based flow. When we redirect the user to
//! Google/GitHub/etc., we must remember the PKCE code verifier, the original
//! state, and which provider was requested — and retrieve it all when the user
//! returns via the callback endpoint.
//!
//! WHAT: [`UpstreamAuthSession`] is the value stored per flow.
//! [`UpstreamAuthSessionStore`] is the trait for CRUD on these sessions.
//! Sessions have a TTL (typically 10 minutes) and are keyed by the OAuth state.
//!
//! HOW: The session is inserted before the 302 redirect, looked up by state on
//! callback, validated (state match, not expired, not consumed), and then
//! consumed (one-time use).

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

/// The data stored while the user authenticates with an upstream provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamAuthSession {
    /// The OAuth state parameter (also the lookup key).
    pub state: String,
    /// Provider id (slug: "google", "github", ...).
    pub provider_id: String,
    /// PKCE code verifier (only for providers with PKCE enabled).
    pub code_verifier: Option<String>,
    /// Where to redirect the user after successful authentication.
    pub redirect_to: Option<String>,
    /// The app's original `state` query parameter — passed through unchanged on the final redirect.
    pub app_state: Option<String>,
    /// OIDC nonce (for OIDC providers).
    pub nonce: Option<String>,
    /// Creation time (epoch millis).
    pub created_at: u64,
    /// TTL in seconds.
    pub ttl_seconds: u64,
    /// Whether this session has been consumed (one-time use).
    pub consumed: bool,
}

impl UpstreamAuthSession {
    #[must_use]
    pub fn new(
        state: String,
        provider_id: String,
        code_verifier: Option<String>,
        nonce: Option<String>,
        redirect_to: Option<String>,
        ttl_seconds: u64,
    ) -> Self {
        Self::with_app_state(state, provider_id, code_verifier, nonce, redirect_to, ttl_seconds, None)
    }

    #[must_use]
    pub fn with_app_state(
        state: String,
        provider_id: String,
        code_verifier: Option<String>,
        nonce: Option<String>,
        redirect_to: Option<String>,
        ttl_seconds: u64,
        app_state: Option<String>,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            state,
            provider_id,
            code_verifier,
            redirect_to,
            app_state,
            nonce,
            created_at: now,
            ttl_seconds,
            consumed: false,
        }
    }

    /// Whether the session has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        (now as u64) > self.created_at / 1000 + self.ttl_seconds
    }
}

// ---------------------------------------------------------------------------
// Store trait
// ---------------------------------------------------------------------------

/// Errors from the auth session store.
#[derive(Debug)]
pub enum AuthSessionError {
    /// Session not found (expired or never created).
    NotFound,
    /// Session has already been consumed.
    AlreadyConsumed,
    /// Session has expired.
    Expired,
    /// Storage back-end error.
    Storage(String),
    /// Serialization error.
    Serialization(String),
}

impl core::fmt::Display for AuthSessionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotFound => write!(f, "auth session not found"),
            Self::AlreadyConsumed => write!(f, "auth session already consumed"),
            Self::Expired => write!(f, "auth session has expired"),
            Self::Storage(s) => write!(f, "auth session storage error: {s}"),
            Self::Serialization(s) => write!(f, "auth session serialization error: {s}"),
        }
    }
}

impl std::error::Error for AuthSessionError {}

/// CRUD for upstream authentication sessions.
///
/// Implementations use `foundation_db::KeyValueStore` with a TTL-aware key
/// prefix (`auth_session:<state>`) — sessions are short-lived and do not
/// need a dedicated SQL table.
pub trait UpstreamAuthSessionStore: Send + Sync {
    /// Insert a new session, keyed by state.
    ///
    /// # Errors
    ///
    /// Returns `AuthSessionError::Storage` on I/O failure.
    fn insert(&self, session: &UpstreamAuthSession) -> Result<(), AuthSessionError>;

    /// Look up a session by state and consume it (one-time use).
    ///
    /// After this call, the session is marked consumed and cannot be retrieved
    /// again. Returns `NotFound` if the key doesn't exist, `AlreadyConsumed` if
    /// consumed, or `Expired` if past its TTL.
    ///
    /// # Errors
    ///
    /// Returns `AuthSessionError` variants as described.
    fn take(&self, state: &str) -> Result<UpstreamAuthSession, AuthSessionError>;

    /// Remove a session without consuming it (cleanup).
    fn delete(&self, state: &str) -> Result<(), AuthSessionError>;
}

// ---------------------------------------------------------------------------
// Default TTL
// ---------------------------------------------------------------------------

/// Default TTL for an auth session: 10 minutes.
pub const DEFAULT_AUTH_SESSION_TTL_SECS: u64 = 600;

// ---------------------------------------------------------------------------
// In-memory store (for tests)
// ---------------------------------------------------------------------------

pub mod memory {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// In-memory auth session store for testing.
    pub struct MemoryAuthSessionStore {
        sessions: Mutex<HashMap<String, UpstreamAuthSession>>,
    }

    impl MemoryAuthSessionStore {
        #[must_use]
        pub fn new() -> Self {
            Self {
                sessions: Mutex::new(HashMap::new()),
            }
        }
    }

    impl Default for MemoryAuthSessionStore {
        fn default() -> Self {
            Self::new()
        }
    }

    impl UpstreamAuthSessionStore for MemoryAuthSessionStore {
        fn insert(&self, session: &UpstreamAuthSession) -> Result<(), AuthSessionError> {
            let mut map = self.sessions.lock().map_err(|e| {
                AuthSessionError::Storage(format!("mutex poisoned: {e}"))
            })?;
            map.insert(session.state.clone(), session.clone());
            Ok(())
        }

        fn take(&self, state: &str) -> Result<UpstreamAuthSession, AuthSessionError> {
            let mut map = self.sessions.lock().map_err(|e| {
                AuthSessionError::Storage(format!("mutex poisoned: {e}"))
            })?;
            let session = map.remove(state).ok_or(AuthSessionError::NotFound)?;
            if session.consumed {
                return Err(AuthSessionError::AlreadyConsumed);
            }
            if session.is_expired() {
                return Err(AuthSessionError::Expired);
            }
            Ok(session)
        }

        fn delete(&self, state: &str) -> Result<(), AuthSessionError> {
            let mut map = self.sessions.lock().map_err(|e| {
                AuthSessionError::Storage(format!("mutex poisoned: {e}"))
            })?;
            map.remove(state);
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use memory::MemoryAuthSessionStore;

    #[test]
    fn session_insert_and_take_round_trips() {
        let store = MemoryAuthSessionStore::new();
        let session = UpstreamAuthSession::new(
            "state-abc123".into(),
            "google".into(),
            Some("pkce-verifier".into()),
            Some("nonce-123".into()),
            Some("https://app.example.com/dashboard".into()),
            600,
        );

        store.insert(&session).expect("insert");
        let taken = store.take("state-abc123").expect("take");
        assert_eq!(taken.provider_id, "google");
        assert_eq!(taken.code_verifier.unwrap(), "pkce-verifier");
        assert_eq!(taken.nonce.unwrap(), "nonce-123");
        assert_eq!(taken.redirect_to.unwrap(), "https://app.example.com/dashboard");
    }

    #[test]
    fn take_removes_session_one_time_only() {
        let store = MemoryAuthSessionStore::new();
        let session = UpstreamAuthSession::new(
            "state-xyz".into(),
            "github".into(),
            None,
            None,
            None,
            600,
        );
        store.insert(&session).expect("insert");
        let _first = store.take("state-xyz").expect("first take");
        let second = store.take("state-xyz");
        assert!(matches!(second, Err(AuthSessionError::NotFound)));
    }

    #[test]
    fn not_found_returns_error() {
        let store = MemoryAuthSessionStore::new();
        let err = store.take("nonexistent").unwrap_err();
        assert!(matches!(err, AuthSessionError::NotFound));
    }

    #[test]
    fn expired_session_returns_error() {
        let store = MemoryAuthSessionStore::new();
        let mut session = UpstreamAuthSession::new(
            "expired-state".into(),
            "google".into(),
            None,
            None,
            None,
            1, // 1-second TTL
        );
        // Fake expiry — set created_at to 2 seconds ago
        session.created_at = session.created_at.saturating_sub(2000);
        store.insert(&session).expect("insert");
        let err = store.take("expired-state").unwrap_err();
        assert!(matches!(err, AuthSessionError::Expired));
    }
}

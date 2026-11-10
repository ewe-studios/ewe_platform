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

use foundation_compact::time;
use foundation_core::valtron::Ctx;
use serde::{Deserialize, Serialize};

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
    /// The OAuth client that asked for the login. The authorization code made
    /// at the end of the flow is bound to it.
    #[serde(default)]
    pub client_id: Option<String>,
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
        ctx: &Ctx,
    ) -> Self {
        Self::with_app_state(
            state,
            provider_id,
            code_verifier,
            nonce,
            redirect_to,
            ttl_seconds,
            None,
            ctx,
        )
    }

    /// Bind the session to the client that asked for the login.
    #[must_use]
    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn with_app_state(
        state: String,
        provider_id: String,
        code_verifier: Option<String>,
        nonce: Option<String>,
        redirect_to: Option<String>,
        ttl_seconds: u64,
        app_state: Option<String>,
        ctx: &Ctx,
    ) -> Self {
        let now = u64::try_from(time::unix_millis(ctx.clock().wall_now())).unwrap_or_default();
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
            client_id: None,
        }
    }

    /// Whether the session has expired.
    #[must_use]
    pub fn is_expired(&self, ctx: &Ctx) -> bool {
        let now = u64::try_from(time::unix_seconds(ctx.clock().wall_now())).unwrap_or_default();
        now > self.created_at / 1000 + self.ttl_seconds
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
    fn take(&self, state: &str, ctx: &Ctx) -> Result<UpstreamAuthSession, AuthSessionError>;

    /// Remove a session without consuming it (cleanup).
    ///
    /// # Errors
    ///
    /// Returns `AuthSessionError::Storage` if the storage back end fails.
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
    use super::{AuthSessionError, UpstreamAuthSession, UpstreamAuthSessionStore};
    use foundation_core::valtron::Ctx;
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
            let mut map = self
                .sessions
                .lock()
                .map_err(|e| AuthSessionError::Storage(format!("mutex poisoned: {e}")))?;
            map.insert(session.state.clone(), session.clone());
            Ok(())
        }

        fn take(&self, state: &str, ctx: &Ctx) -> Result<UpstreamAuthSession, AuthSessionError> {
            let mut map = self
                .sessions
                .lock()
                .map_err(|e| AuthSessionError::Storage(format!("mutex poisoned: {e}")))?;
            let session = map.remove(state).ok_or(AuthSessionError::NotFound)?;
            if session.consumed {
                return Err(AuthSessionError::AlreadyConsumed);
            }
            if session.is_expired(ctx) {
                return Err(AuthSessionError::Expired);
            }
            Ok(session)
        }

        fn delete(&self, state: &str) -> Result<(), AuthSessionError> {
            let mut map = self
                .sessions
                .lock()
                .map_err(|e| AuthSessionError::Storage(format!("mutex poisoned: {e}")))?;
            map.remove(state);
            Ok(())
        }
    }
}

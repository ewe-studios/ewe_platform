//! Session management service — wraps existing `SessionManager`.

use crate::shared::credential_store::{CredentialStorage, CredentialStore};
use crate::shared::session::{Session, SessionError, SessionManager};
use foundation_context::Ctx;
use foundation_netio::shared::client::Cookie;
use std::sync::Arc;

#[derive(Debug)]
pub enum SessionServiceError {
    Session(SessionError),
}

impl core::fmt::Display for SessionServiceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Session(e) => write!(f, "Session error: {e}"),
        }
    }
}

impl std::error::Error for SessionServiceError {}

impl From<SessionError> for SessionServiceError {
    fn from(e: SessionError) -> Self {
        Self::Session(e)
    }
}

/// Creates, reads and revokes login sessions over a [`CredentialStore`].
///
/// The `IdP` uses [`KvCredentialStore`](crate::shared::credential_store::KvCredentialStore)
/// over its own cache, so sessions live where the rest of its state lives. In
/// memory only if that store is the in-memory one.
pub struct SessionService<S: CredentialStore = CredentialStorage> {
    session_mgr: Arc<SessionManager<S>>,
}

impl<S: CredentialStore> SessionService<S> {
    #[must_use]
    pub fn new(session_mgr: Arc<SessionManager<S>>) -> Self {
        Self { session_mgr }
    }

    /// A session service over `store`, signing tokens with `signing_key`
    /// (at least 32 bytes). Keep the key stable: sessions signed with another
    /// key are not valid.
    ///
    /// # Errors
    ///
    /// Returns [`SessionServiceError`] if the key is shorter than 32 bytes.
    pub fn over_store(
        store: S,
        config: crate::shared::session::SessionConfig,
        signing_key: &[u8],
        ctx: Ctx,
    ) -> Result<Self, SessionServiceError> {
        let mgr = SessionManager::new(store, config, signing_key, ctx)?;
        Ok(Self::new(Arc::new(mgr)))
    }

    /// Cookies that clear the session cookies in the browser (same names and
    /// path, empty value, `Max-Age=0`).
    #[must_use]
    pub fn clearing_cookies(&self) -> Vec<Cookie> {
        let config = self.session_mgr.config();
        [&config.token_cookie_name, &config.data_cookie_name]
            .into_iter()
            .map(|name| {
                Cookie::new(name, "")
                    .path("/")
                    .max_age(std::time::Duration::ZERO)
                    .http_only(true)
                    .secure(config.secure_cookies)
                    .same_site(foundation_netio::shared::client::SameSite::Lax)
            })
            .collect()
    }

    /// The name of the session token cookie.
    #[must_use]
    pub fn token_cookie_name(&self) -> &str {
        &self.session_mgr.config().token_cookie_name
    }

    /// Create session.
    ///
    /// # Errors
    ///
    /// Returns [`SessionServiceError`] if the operation fails.
    pub async fn create_session(
        &self,
        user_id: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<(Session, Vec<Cookie>), SessionServiceError> {
        self.session_mgr
            .create_session(user_id, ip_address, user_agent)
            .await
            .map_err(SessionServiceError::Session)
    }

    /// Get session.
    ///
    /// # Errors
    ///
    /// Returns [`SessionServiceError`] if the operation fails.
    pub async fn get_session(&self, token: &str) -> Result<Option<Session>, SessionServiceError> {
        self.session_mgr
            .get_session(token)
            .await
            .map_err(SessionServiceError::Session)
    }

    /// Revoke session.
    ///
    /// # Errors
    ///
    /// Returns [`SessionServiceError`] if the operation fails.
    pub async fn revoke_session(&self, session_id: &str) -> Result<(), SessionServiceError> {
        self.session_mgr
            .revoke_session(session_id)
            .await
            .map_err(SessionServiceError::Session)
    }

    /// Revoke all sessions.
    ///
    /// # Errors
    ///
    /// Returns [`SessionServiceError`] if the operation fails.
    pub async fn revoke_all_sessions(&self, user_id: &str) -> Result<usize, SessionServiceError> {
        self.session_mgr
            .revoke_all_sessions(user_id)
            .await
            .map_err(SessionServiceError::Session)
    }
}

impl SessionService<CredentialStorage> {
    /// A session service over in-memory storage and a random signing key.
    ///
    /// Sessions live in this process and end when it stops. Use
    /// [`SessionService::new`] with a persistent `SessionManager` for a real
    /// deployment.
    ///
    /// # Errors
    ///
    /// Returns [`SessionServiceError`] if the session manager cannot be built.
    pub fn in_memory(ctx: Ctx) -> Result<Self, SessionServiceError> {
        Self::in_memory_with(crate::shared::session::SessionConfig::default(), ctx)
    }

    /// Like [`in_memory`](Self::in_memory), with this session configuration
    /// (cookie names, lifetimes, the `Secure` flag).
    ///
    /// # Errors
    ///
    /// Returns [`SessionServiceError`] if the session manager cannot be built.
    pub fn in_memory_with(
        config: crate::shared::session::SessionConfig,
        ctx: Ctx,
    ) -> Result<Self, SessionServiceError> {
        use foundation_compact::rng::Rng;
        let mut key = [0u8; 32];
        foundation_compact::rng::rng().fill_bytes(&mut key);
        let mgr = SessionManager::new(CredentialStorage::memory(), config, &key, ctx)?;
        Ok(Self::new(Arc::new(mgr)))
    }
}

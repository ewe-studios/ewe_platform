//! Session management service — wraps existing `SessionManager`.

use crate::shared::credential_store::CredentialStorage;
use crate::shared::session::{Session, SessionError, SessionManager};
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

pub struct SessionService {
    session_mgr: Arc<SessionManager<CredentialStorage>>,
}

impl SessionService {
    #[must_use]
    pub fn new(session_mgr: Arc<SessionManager<CredentialStorage>>) -> Self {
        Self { session_mgr }
    }

    /// A session service over in-memory storage and a random signing key.
    ///
    /// Sessions live in this process and end when it stops. Use
    /// [`SessionService::new`] with a persistent `SessionManager` for a real
    /// deployment.
    ///
    /// # Errors
    ///
    /// Returns [`SessionServiceError`] if the session manager cannot be built.
    pub fn in_memory() -> Result<Self, SessionServiceError> {
        use foundation_compact::rng::Rng;
        let mut key = [0u8; 32];
        foundation_compact::rng::rng().fill_bytes(&mut key);
        let mgr = SessionManager::new(
            CredentialStorage::memory(),
            crate::shared::session::SessionConfig::default(),
            &key,
        )?;
        Ok(Self::new(Arc::new(mgr)))
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

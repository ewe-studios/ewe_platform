//! Session management service — wraps existing SessionManager.

use crate::shared::credential_store::CredentialStorage;
use crate::shared::session::{Session, SessionError, SessionManager};
use foundation_netio::simple_http::client::shared::Cookie;
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

    pub fn create_session(
        &self,
        user_id: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<(Session, Vec<Cookie>), SessionServiceError> {
        self.session_mgr
            .create_session(user_id, ip_address, user_agent)
            .map_err(SessionServiceError::Session)
    }

    pub fn get_session(&self, token: &str) -> Result<Option<Session>, SessionServiceError> {
        self.session_mgr
            .get_session(token)
            .map_err(SessionServiceError::Session)
    }

    pub fn revoke_session(&self, session_id: &str) -> Result<(), SessionServiceError> {
        self.session_mgr
            .revoke_session(session_id)
            .map_err(SessionServiceError::Session)
    }

    pub fn revoke_all_sessions(&self, user_id: &str) -> Result<usize, SessionServiceError> {
        self.session_mgr
            .revoke_all_sessions(user_id)
            .map_err(SessionServiceError::Session)
    }
}

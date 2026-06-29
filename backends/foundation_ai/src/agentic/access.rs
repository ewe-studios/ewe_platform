//! Access control & budget surfacing (F18).
//!
//! WHY: The agent needs a clean boundary that defaults to allow-everything locally
//! and can gate session/model access + retrieve per-user token budgets in hosted
//! settings — without colliding with `foundation_ai::AuthProvider` (provider
//! credentials, a different concern).
//!
//! WHAT: `SessionAccessProvider` trait (domain methods + budget), `AllowAllAccess`
//! (local default), `TokenBudget` (limit + used → caps F04's ledger).

use super::errors::{AuthError, UserId};
use crate::types::SessionId;

// ---------------------------------------------------------------------------
// TokenBudget

/// A user's token budget — `limit: None` means unlimited.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenBudget {
    pub limit: Option<u64>,
    pub used: u64,
}

impl TokenBudget {
    #[must_use]
    pub fn remaining(&self) -> Option<u64> {
        self.limit.map(|l| l.saturating_sub(self.used))
    }

    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.limit.is_some_and(|l| self.used >= l)
    }

    #[must_use]
    pub fn unlimited() -> Self {
        Self {
            limit: None,
            used: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// SessionAccessProvider trait

/// The agentic access boundary — DISTINCT from `foundation_ai::AuthProvider`
/// (which supplies provider credentials like API keys).
///
/// Domain methods for session/model/tool gating + token budget retrieval.
/// `AllowAllAccess` is the local default; hosted impls bridge to
/// `foundation_auth` (identity) + `foundation_cedar` (policies).
pub trait SessionAccessProvider: Send + Sync {
    /// # Errors
    /// Returns [`AuthError`] if the user cannot access the session.
    fn can_access_session(&self, user: &UserId, session: &SessionId) -> Result<bool, AuthError>;

    /// # Errors
    /// Returns [`AuthError`] if the user cannot use the model.
    fn can_use_model(&self, user: &UserId, model: &str) -> Result<bool, AuthError>;

    /// # Errors
    /// Returns [`AuthError`] if the user cannot use the tool.
    fn can_use_tool(&self, _user: &UserId, _tool: &str) -> Result<bool, AuthError> {
        Ok(true)
    }

    /// # Errors
    /// Returns [`AuthError`] if the user cannot spend the tokens.
    fn can_spend(&self, _user: &UserId, _tokens: u64) -> Result<bool, AuthError> {
        Ok(true)
    }

    /// # Errors
    /// Returns [`AuthError`] if the budget cannot be retrieved.
    fn token_budget(&self, user: &UserId) -> Result<TokenBudget, AuthError>;

    /// # Errors
    /// Returns [`AuthError`] if the usage cannot be recorded.
    fn record_usage(&self, _user: &UserId, _tokens: u64) -> Result<(), AuthError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AllowAllAccess — local / no-auth default

/// Everything allowed, unlimited budget. The local/test default.
pub struct AllowAllAccess;

impl SessionAccessProvider for AllowAllAccess {
    fn can_access_session(&self, _user: &UserId, _session: &SessionId) -> Result<bool, AuthError> {
        Ok(true)
    }

    fn can_use_model(&self, _user: &UserId, _model: &str) -> Result<bool, AuthError> {
        Ok(true)
    }

    fn token_budget(&self, _user: &UserId) -> Result<TokenBudget, AuthError> {
        Ok(TokenBudget::unlimited())
    }
}


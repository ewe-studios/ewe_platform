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
    fn can_access_session(&self, user: &UserId, session: &SessionId) -> Result<bool, AuthError>;

    fn can_use_model(&self, user: &UserId, model: &str) -> Result<bool, AuthError>;

    fn can_use_tool(&self, _user: &UserId, _tool: &str) -> Result<bool, AuthError> {
        Ok(true)
    }

    fn can_spend(&self, _user: &UserId, _tokens: u64) -> Result<bool, AuthError> {
        Ok(true)
    }

    fn token_budget(&self, user: &UserId) -> Result<TokenBudget, AuthError>;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_user() -> UserId {
        UserId("test-user".into())
    }

    #[test]
    fn allow_all_permits_everything() {
        let access = AllowAllAccess;
        let user = test_user();
        let sid = SessionId::new();

        assert!(access.can_access_session(&user, &sid).unwrap());
        assert!(access.can_use_model(&user, "gpt-4").unwrap());
        assert!(access.can_use_tool(&user, "read_file").unwrap());
        assert!(access.can_spend(&user, 999_999).unwrap());
    }

    #[test]
    fn allow_all_budget_is_unlimited() {
        let access = AllowAllAccess;
        let budget = access.token_budget(&test_user()).unwrap();
        assert_eq!(budget.limit, None);
        assert_eq!(budget.used, 0);
        assert_eq!(budget.remaining(), None);
        assert!(!budget.is_exhausted());
    }

    #[test]
    fn allow_all_record_usage_is_noop() {
        let access = AllowAllAccess;
        assert!(access.record_usage(&test_user(), 1000).is_ok());
    }

    #[test]
    fn token_budget_remaining_and_exhausted() {
        let budget = TokenBudget {
            limit: Some(1000),
            used: 600,
        };
        assert_eq!(budget.remaining(), Some(400));
        assert!(!budget.is_exhausted());

        let exhausted = TokenBudget {
            limit: Some(1000),
            used: 1000,
        };
        assert_eq!(exhausted.remaining(), Some(0));
        assert!(exhausted.is_exhausted());

        let over = TokenBudget {
            limit: Some(1000),
            used: 1200,
        };
        assert_eq!(over.remaining(), Some(0));
        assert!(over.is_exhausted());
    }

    #[test]
    fn unlimited_budget_never_exhausted() {
        let budget = TokenBudget::unlimited();
        assert!(!budget.is_exhausted());
        assert_eq!(budget.remaining(), None);
    }

    #[test]
    fn auth_error_is_clone_and_partial_eq() {
        let e1 = AuthError {
            reason: "forbidden".into(),
        };
        let e2 = e1.clone();
        assert_eq!(e1, e2);
    }

    #[test]
    fn custom_access_provider_can_deny() {
        struct DenyAll;
        impl SessionAccessProvider for DenyAll {
            fn can_access_session(
                &self,
                _user: &UserId,
                _session: &SessionId,
            ) -> Result<bool, AuthError> {
                Ok(false)
            }
            fn can_use_model(&self, _user: &UserId, _model: &str) -> Result<bool, AuthError> {
                Ok(false)
            }
            fn can_use_tool(&self, _user: &UserId, _tool: &str) -> Result<bool, AuthError> {
                Ok(false)
            }
            fn can_spend(&self, _user: &UserId, _tokens: u64) -> Result<bool, AuthError> {
                Ok(false)
            }
            fn token_budget(&self, _user: &UserId) -> Result<TokenBudget, AuthError> {
                Ok(TokenBudget {
                    limit: Some(0),
                    used: 0,
                })
            }
        }

        let access = DenyAll;
        let user = test_user();
        let sid = SessionId::new();

        assert!(!access.can_access_session(&user, &sid).unwrap());
        assert!(!access.can_use_model(&user, "gpt-4").unwrap());
        assert!(!access.can_use_tool(&user, "shell").unwrap());
        assert!(!access.can_spend(&user, 1).unwrap());

        let budget = access.token_budget(&user).unwrap();
        assert_eq!(budget.limit, Some(0));
    }
}

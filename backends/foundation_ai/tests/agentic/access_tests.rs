use foundation_ai::agentic::access::*;
use foundation_ai::agentic::errors::{AuthError, UserId};
use foundation_ai::types::agentic::SessionId;

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

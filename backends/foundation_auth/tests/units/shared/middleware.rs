//! Shared middleware tests.

use foundation_auth::shared::middleware::{
    extract_bearer_token, extract_session_token, optional_auth, require_auth, AuthContext,
    GuardResult,
};
use foundation_auth::shared::auth_token::AuthToken;
use foundation_auth::shared::types::ConfidentialText;

#[test]
fn test_require_auth_with_valid_token() {
    let future = chrono::Utc::now().timestamp() as f64 + 3600.0;
    let ctx =
        AuthContext::new("/api/data".to_string(), None, None).with_token(AuthToken::OAuth {
            access_token: ConfidentialText::new("tok".to_string()),
            refresh_token: None,
            token_type: "Bearer".to_string(),
            expires_at: future,
            scope: Some("read".to_string()),
        });

    match require_auth(ctx) {
        GuardResult::Authorized(c) => assert_eq!(c.path, "/api/data"),
        other => panic!("expected Authorized, got {other:?}"),
    }
}

#[test]
fn test_require_auth_expired_token() {
    let past = chrono::Utc::now().timestamp() as f64 - 3600.0;
    let ctx =
        AuthContext::new("/api/data".to_string(), None, None).with_token(AuthToken::OAuth {
            access_token: ConfidentialText::new("tok".to_string()),
            refresh_token: None,
            token_type: "Bearer".to_string(),
            expires_at: past,
            scope: None,
        });

    assert!(matches!(require_auth(ctx), GuardResult::TokenExpired));
}

#[test]
fn test_require_auth_no_token() {
    let ctx = AuthContext::new("/api/data".to_string(), None, None);
    assert!(matches!(require_auth(ctx), GuardResult::Unauthorized));
}

#[test]
fn test_optional_auth_strips_expired_token() {
    let past = chrono::Utc::now().timestamp() as f64 - 3600.0;
    let ctx =
        AuthContext::new("/api/data".to_string(), None, None).with_token(AuthToken::OAuth {
            access_token: ConfidentialText::new("tok".to_string()),
            refresh_token: None,
            token_type: "Bearer".to_string(),
            expires_at: past,
            scope: None,
        });

    if let GuardResult::Authorized(c) = optional_auth(ctx) {
        assert!(c.token.is_none());
    } else {
        panic!("optional_auth should always return Authorized");
    }
}

#[test]
fn test_extract_session_token() {
    let cookies = vec![
        "session_token=abc123; Path=/; HttpOnly",
        "session_data={}; Path=/",
    ];
    assert_eq!(
        extract_session_token(&cookies, "session_token"),
        Some("abc123".to_string())
    );
}

#[test]
fn test_extract_bearer_token() {
    assert_eq!(
        extract_bearer_token(Some("Bearer abc123")),
        Some("abc123".to_string())
    );
    assert!(extract_bearer_token(Some("Invalid abc123")).is_none());
    assert!(extract_bearer_token(None).is_none());
}

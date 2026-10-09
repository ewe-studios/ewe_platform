//! Shared auth_token tests.

use chrono::Utc;
use foundation_auth::shared::auth_token::AuthToken;
use foundation_auth::shared::types::ConfidentialText;

#[test]
fn test_oauth_token_not_expired() {
    let future = Utc::now().timestamp() as f64 + 3600.0;
    let token = AuthToken::OAuth {
        access_token: ConfidentialText::new("tok".to_string()),
        refresh_token: None,
        token_type: "Bearer".to_string(),
        expires_at: future,
        scope: None,
    };
    assert!(!token.is_expired());
}

#[test]
fn test_oauth_token_expired() {
    let past = Utc::now().timestamp() as f64 - 3600.0;
    let token = AuthToken::OAuth {
        access_token: ConfidentialText::new("tok".to_string()),
        refresh_token: None,
        token_type: "Bearer".to_string(),
        expires_at: past,
        scope: None,
    };
    assert!(token.is_expired());
}

#[test]
fn test_api_key_never_expires() {
    let token = AuthToken::ApiKey {
        key: ConfidentialText::new("key123".to_string()),
    };
    assert!(!token.is_expired());
}

#[test]
fn test_bearer_token_format() {
    let token = AuthToken::OAuth {
        access_token: ConfidentialText::new("abc".to_string()),
        refresh_token: None,
        token_type: "Bearer".to_string(),
        expires_at: Utc::now().timestamp() as f64 + 3600.0,
        scope: None,
    };
    assert_eq!(token.bearer_token(), Some("Bearer abc".to_string()));
}

#[test]
fn test_session_no_bearer() {
    let token = AuthToken::Session {
        session_id: "s1".to_string(),
        token: ConfidentialText::new("tok".to_string()),
        expires_at: Utc::now().timestamp() as f64 + 3600.0,
        cookie: None,
    };
    assert!(token.bearer_token().is_none());
}

use chrono::Utc;
use foundation_auth::{JwtManager, JwtToken};

#[test]
fn test_jwt_token_creation() {
    let future_time = Utc::now().timestamp() + 3600;
    let token = JwtToken::from_parts(
        "test_token".to_string(),
        Some("test_refresh".to_string()),
        future_time,
        Some("read:write".to_string()),
        Some("api".to_string()),
        Some("auth.example.com".to_string()),
    );

    assert!(!token.is_expired());
    assert_eq!(token.access_token(), "test_token");
    assert_eq!(token.refresh_token(), Some("test_refresh".to_string()));
    assert_eq!(token.scope, Some("read:write".to_string()));
}

#[test]
fn test_jwt_token_expiration() {
    let past_time = Utc::now().timestamp() - 3600;
    let token = JwtToken::from_parts(
        "expired_token".to_string(),
        None,
        past_time,
        None,
        None,
        None,
    );

    assert!(token.is_expired());
    assert!(token.expires_in() == 0);
}

#[test]
fn test_jwt_token_expires_within() {
    let soon = Utc::now().timestamp() + 120;
    let token =
        JwtToken::from_parts("expiring_token".to_string(), None, soon, None, None, None);

    assert!(token.expires_within(300));
    assert!(!token.expires_within(60));
}

#[test]
fn test_jwt_manager_refresh() {
    let mut manager = JwtManager::new().with_refresh_buffer(300);

    assert!(!manager.has_valid_token());

    let soon = Utc::now().timestamp() + 60;
    manager.set_token(JwtToken::from_parts(
        "test_token".to_string(),
        Some("refresh_token".to_string()),
        soon,
        None,
        None,
        None,
    ));

    assert!(manager.get_token().unwrap().expires_within(300));

    let refresh_fn = |_refresh_token: String| {
        let future = Utc::now().timestamp() + 3600;
        Ok(JwtToken::from_parts(
            "new_token".to_string(),
            Some("new_refresh".to_string()),
            future,
            None,
            None,
            None,
        ))
    };

    let refreshed = manager.refresh_if_needed(refresh_fn).unwrap();
    assert!(refreshed);
    assert_eq!(manager.get_token().unwrap().access_token(), "new_token");
}

use std::sync::Arc;

use chrono::Utc;
use foundation_auth::{JwtManager, JwtToken};
use foundation_core::valtron::{Clock, MonotonicClock};

fn test_clock() -> Arc<dyn Clock> {
    Arc::new(MonotonicClock::new())
}

#[test]
fn test_jwt_token_creation() {
    let clock = test_clock();
    let future_time = Utc::now().timestamp() + 3600;
    let token = JwtToken::from_parts(
        "test_token".to_string(),
        Some("test_refresh".to_string()),
        future_time,
        Some("read:write".to_string()),
        Some("api".to_string()),
        Some("auth.example.com".to_string()),
        clock.as_ref(),
    );

    assert!(!token.is_expired(clock.as_ref()));
    assert_eq!(token.access_token(), "test_token");
    assert_eq!(token.refresh_token(), Some("test_refresh".to_string()));
    assert_eq!(token.scope, Some("read:write".to_string()));
}

#[test]
fn test_jwt_token_expiration() {
    let clock = test_clock();
    let past_time = Utc::now().timestamp() - 3600;
    let token = JwtToken::from_parts(
        "expired_token".to_string(),
        None,
        past_time,
        None,
        None,
        None,
        clock.as_ref(),
    );

    assert!(token.is_expired(clock.as_ref()));
    assert_eq!(token.expires_in(clock.as_ref()), 0);
}

#[test]
fn test_jwt_token_expires_within() {
    let clock = test_clock();
    let soon = Utc::now().timestamp() + 120;
    let token = JwtToken::from_parts(
        "expiring_token".to_string(),
        None,
        soon,
        None,
        None,
        None,
        clock.as_ref(),
    );

    assert!(token.expires_within(300, clock.as_ref()));
    assert!(!token.expires_within(60, clock.as_ref()));
}

#[test]
fn test_jwt_manager_refresh() {
    let clock = test_clock();
    let mut manager = JwtManager::new(Arc::clone(&clock)).with_refresh_buffer(300);

    assert!(!manager.has_valid_token());

    let soon = Utc::now().timestamp() + 60;
    manager.set_token(JwtToken::from_parts(
        "test_token".to_string(),
        Some("refresh_token".to_string()),
        soon,
        None,
        None,
        None,
        clock.as_ref(),
    ));

    assert!(manager
        .get_token()
        .unwrap()
        .expires_within(300, clock.as_ref()));

    let refresh_fn = |_refresh_token: String| {
        let future = Utc::now().timestamp() + 3600;
        Ok(JwtToken::from_parts(
            "new_token".to_string(),
            Some("new_refresh".to_string()),
            future,
            None,
            None,
            None,
            clock.as_ref(),
        ))
    };

    let refreshed = manager.refresh_if_needed(refresh_fn).unwrap();
    assert!(refreshed);
    assert_eq!(manager.get_token().unwrap().access_token(), "new_token");
}

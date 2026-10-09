//! User model tests.

use chrono::Utc;
use foundation_auth::server::models::user::User;
use std::time::Duration;

fn test_user() -> User {
    User {
        id: "user_1".into(),
        email: "test@example.com".into(),
        username: None,
        password_hash: Some("$argon2id$hash".into()),
        email_verified: false,
        email_verified_at: None,
        created_at: Utc::now().timestamp_millis(),
        updated_at: Utc::now().timestamp_millis(),
        metadata: None,
        failed_login_attempts: 0,
        locked_until: None,
        deleted_at: None,
    }
}

#[test]
fn test_not_locked() {
    let user = test_user();
    assert!(!user.is_locked());
}

#[test]
fn test_locked_future() {
    let mut user = test_user();
    user.locked_until = Some(Utc::now().timestamp_millis() + 60_000);
    assert!(user.is_locked());
}

#[test]
fn test_locked_past() {
    let mut user = test_user();
    user.locked_until = Some(Utc::now().timestamp_millis() - 60_000);
    assert!(!user.is_locked());
}

#[test]
fn test_record_failed_attempts_locks() {
    let mut user = test_user();
    for _ in 0..5 {
        user.record_failed_attempt(5, Duration::from_secs(900));
    }
    assert!(user.is_locked());
    assert_eq!(user.failed_login_attempts, 5);
}

#[test]
fn test_reset_failed_attempts() {
    let mut user = test_user();
    user.failed_login_attempts = 3;
    user.locked_until = Some(Utc::now().timestamp_millis() + 60_000);
    user.reset_failed_attempts();
    assert_eq!(user.failed_login_attempts, 0);
    assert!(!user.is_locked());
}

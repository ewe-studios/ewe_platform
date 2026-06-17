//! UserService tests — password hashing and validation.

use foundation_auth::server::services::user_service::{hash_password, validate_password, verify_password};
use foundation_auth::server::config::PasswordPolicy;

#[test]
fn test_hash_and_verify() {
    let hash = hash_password("Str0ng!Password#2026").unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert!(verify_password(&hash, "Str0ng!Password#2026").unwrap());
    assert!(!verify_password(&hash, "wrong_password").unwrap());
}

#[test]
fn test_validate_password_pass() {
    let policy = PasswordPolicy::default();
    assert!(validate_password("Str0ng!Pass#2", &policy).is_ok());
}

#[test]
fn test_validate_password_too_short() {
    let policy = PasswordPolicy::default();
    let result = validate_password("Sh0rt!", &policy);
    assert!(result.is_err());
}

#[test]
fn test_validate_password_no_uppercase() {
    let policy = PasswordPolicy::default();
    let result = validate_password("str0ng!pass#2026", &policy);
    assert!(result.is_err());
}

//! AuthorizationCode and DeviceCode model tests.

use foundation_auth::server::models::code::{AuthorizationCode, DeviceCode};
use sha2::{Digest, Sha256};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use std::time::Duration;

#[test]
fn test_auth_code_not_expired() {
    let code = AuthorizationCode::new(
        "u1".into(),
        "c1".into(),
        "https://example.com/cb".into(),
        None,
        "openid".into(),
        None,
        Duration::from_secs(600),
    );
    assert!(!code.is_expired());
    assert!(!code.code.is_empty());
}

#[test]
fn test_auth_code_pkce_verify() {
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    let code = AuthorizationCode::new(
        "u1".into(),
        "c1".into(),
        "https://example.com/cb".into(),
        Some(challenge),
        "openid".into(),
        None,
        Duration::from_secs(600),
    );

    assert!(code.verify_pkce(verifier));
    assert!(!code.verify_pkce("wrong_verifier"));
}

#[test]
fn test_auth_code_pkce_none() {
    let code = AuthorizationCode::new(
        "u1".into(),
        "c1".into(),
        "https://example.com/cb".into(),
        None,
        "openid".into(),
        None,
        Duration::from_secs(600),
    );
    assert!(code.verify_pkce("anything"));
}

#[test]
fn test_device_code_creation() {
    let dc = DeviceCode::new("c1".into(), "openid".into(), Duration::from_secs(600), 5);
    assert!(!dc.is_expired());
    assert!(!dc.is_approved());
    assert!(dc.user_code.contains('-'));
    assert_eq!(dc.user_code.len(), 9);
}

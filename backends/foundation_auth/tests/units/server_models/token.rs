//! RefreshToken model tests.

use chrono::Utc;
use foundation_auth::server::models::token::RefreshToken;

fn hex_sha256(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(s.as_bytes());
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

fn test_token(plaintext: &str) -> RefreshToken {
    RefreshToken {
        id: "rt_1".into(),
        user_id: "u1".into(),
        client_id: "c1".into(),
        token_hash: hex_sha256(plaintext),
        expires_at: Utc::now().timestamp_millis() + 86_400_000,
        rotated_at: None,
        created_at: Utc::now().timestamp_millis(),
    }
}

#[test]
fn test_verify_correct() {
    let rt = test_token("my_refresh_token");
    assert!(rt.verify("my_refresh_token"));
}

#[test]
fn test_verify_wrong() {
    let rt = test_token("my_refresh_token");
    assert!(!rt.verify("wrong_token"));
}

#[test]
fn test_not_expired() {
    let rt = test_token("tok");
    assert!(!rt.is_expired());
}

#[test]
fn test_not_rotated() {
    let rt = test_token("tok");
    assert!(!rt.is_rotated());
}

#[test]
fn test_rotated() {
    let mut rt = test_token("tok");
    rt.rotated_at = Some(Utc::now().timestamp_millis());
    assert!(rt.is_rotated());
}

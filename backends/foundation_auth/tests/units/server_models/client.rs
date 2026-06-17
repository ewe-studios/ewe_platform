//! OAuthClient model tests.

use foundation_auth::server::models::client::OAuthClient;

fn test_client(secret: &str) -> OAuthClient {
    OAuthClient {
        id: "client_1".into(),
        name: "Test App".into(),
        client_secret_hash: hex_sha256(secret),
        redirect_uris: vec!["https://app.example.com/callback".into()],
        grant_types: vec![
            "authorization_code".into(),
            "refresh_token".into(),
        ],
        scopes: vec!["openid".into(), "profile".into(), "email".into()],
        is_public: false,
        created_at: 0,
    }
}

fn hex_sha256(s: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(s.as_bytes());
    hash.iter().map(|b| format!("{:02x}", b)).collect()
}

#[test]
fn test_verify_secret_correct() {
    let client = test_client("my_secret");
    assert!(client.verify_secret("my_secret"));
}

#[test]
fn test_verify_secret_wrong() {
    let client = test_client("my_secret");
    assert!(!client.verify_secret("wrong_secret"));
}

#[test]
fn test_allows_redirect() {
    let client = test_client("s");
    assert!(client.allows_redirect("https://app.example.com/callback"));
    assert!(!client.allows_redirect("https://evil.com/callback"));
}

#[test]
fn test_allows_grant() {
    let client = test_client("s");
    assert!(client.allows_grant("authorization_code"));
    assert!(client.allows_grant("refresh_token"));
    assert!(!client.allows_grant("client_credentials"));
}

#[test]
fn test_allows_scope() {
    let client = test_client("s");
    assert!(client.allows_scope("openid profile"));
    assert!(client.allows_scope("openid"));
    assert!(!client.allows_scope("openid admin"));
}

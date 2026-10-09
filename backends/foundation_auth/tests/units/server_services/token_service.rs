//! TokenService tests.

use std::sync::Arc;
use chrono::Utc;
use foundation_auth::server::config::IdpConfig;
use foundation_auth::server::models::client::OAuthClient;
use foundation_auth::server::models::user::User;
use foundation_auth::server::services::token_service::TokenService;

fn test_config() -> Arc<IdpConfig> {
    Arc::new(IdpConfig::new("https://auth.test.com".into()))
}

fn test_user() -> User {
    User {
        id: "user_1".into(),
        email: "alice@example.com".into(),
        username: Some("Alice".into()),
        password_hash: None,
        email_verified: true,
        email_verified_at: None,
        created_at: 0,
        updated_at: 0,
        metadata: None,
        failed_login_attempts: 0,
        locked_until: None,
        deleted_at: None,
    }
}

fn test_client() -> OAuthClient {
    OAuthClient {
        id: "client_1".into(),
        name: "Test App".into(),
        client_secret_hash: String::new(),
        redirect_uris: vec!["https://app.example.com/cb".into()],
        grant_types: vec!["authorization_code".into()],
        scopes: vec!["openid".into(), "profile".into()],
        is_public: false,
        created_at: 0,
    }
}

#[test]
fn test_generate_tokens() {
    let svc = TokenService::new(test_config());
    let result = svc.generate_tokens(&test_user(), &test_client(), "openid profile", None);
    assert!(result.is_ok());
    let pair = result.unwrap();
    assert!(!pair.access_token.is_empty());
    assert!(!pair.id_token.is_empty());
    assert!(!pair.refresh_token.is_empty());
    assert_eq!(pair.scope, "openid profile");
}

#[test]
fn test_generate_tokens_with_nonce() {
    let svc = TokenService::new(test_config());
    let result =
        svc.generate_tokens(&test_user(), &test_client(), "openid", Some("nonce123"));
    assert!(result.is_ok());
}

#[test]
fn test_generate_client_credentials() {
    let svc = TokenService::new(test_config());
    let result = svc.generate_client_credentials_tokens(&test_client(), "openid");
    assert!(result.is_ok());
    let pair = result.unwrap();
    assert!(!pair.access_token.is_empty());
    assert!(pair.id_token.is_empty());
    assert!(pair.refresh_token.is_empty());
}

#[test]
fn test_hash_refresh_token() {
    let hash1 = TokenService::hash_refresh_token("token_a");
    let hash2 = TokenService::hash_refresh_token("token_a");
    let hash3 = TokenService::hash_refresh_token("token_b");
    assert_eq!(hash1, hash2);
    assert_ne!(hash1, hash3);
}

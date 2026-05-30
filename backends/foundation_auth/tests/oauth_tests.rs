//! Native OAuth client tests.

use foundation_auth::native::oauth::NativeOAuth;
use foundation_auth::shared::oauth::{OAuthConfig, OAuthManager, PkceChallenge};
use foundation_auth::shared::oauth_token::OAuthToken;

#[test]
fn test_oauth_config_builder() {
    let config = OAuthConfig::builder()
        .client_id("test_client_id")
        .client_secret("test_client_secret")
        .authorization_url("https://auth.example.com/oauth/authorize")
        .token_url("https://auth.example.com/oauth/token")
        .redirect_uri("https://app.example.com/callback")
        .scope("openid")
        .scope("profile")
        .scope("email")
        .pkce_enabled(true)
        .build();

    assert_eq!(config.client_id, "test_client_id");
    assert_eq!(config.client_secret, Some("test_client_secret".to_string()));
    assert_eq!(config.scopes.len(), 3);
    assert!(config.pkce_enabled);
}

#[test]
fn test_oauth_config_validation() {
    let config = OAuthConfig::default();
    assert!(config.validate().is_err());

    let config = OAuthConfig::builder()
        .client_id("test")
        .authorization_url("https://auth.example.com")
        .token_url("https://auth.example.com/token")
        .redirect_uri("https://app.example.com/callback")
        .build();
    assert!(config.validate().is_ok());
}

#[test]
fn test_pkce_challenge_generation() {
    let challenge = PkceChallenge::generate();
    assert_eq!(challenge.code_verifier.len(), 43);
    assert_eq!(challenge.code_challenge.len(), 43);
    assert_eq!(challenge.challenge_method, "S256");
}

#[test]
fn test_state_generation() {
    let state1 = OAuthManager::generate_state();
    let state2 = OAuthManager::generate_state();
    assert_ne!(state1, state2);
    assert!(state1.len() > 30);
}

#[test]
fn test_state_validation() {
    let state = "test_state_value";
    assert!(OAuthManager::validate_state(state, state));
    assert!(!OAuthManager::validate_state(state, "different_state"));
}

#[test]
fn test_authorization_url_generation() {
    let config = OAuthConfig::builder()
        .client_id("test_client")
        .authorization_url("https://auth.example.com/oauth/authorize")
        .token_url("https://auth.example.com/oauth/token")
        .redirect_uri("https://app.example.com/callback")
        .scope("openid profile")
        .pkce_enabled(true)
        .build();

    let manager = NativeOAuth::new(config);
    let state = OAuthManager::generate_state();
    let (url, pkce) = manager.manager().get_authorization_url(&state).unwrap();

    assert!(url.contains("response_type=code"));
    assert!(url.contains("client_id=test_client"));
    assert!(url.contains("redirect_uri=https%3A%2F%2Fapp.example.com%2Fcallback"));
    assert!(url.contains(&format!("state={state}")));
    assert!(url.contains("scope=openid+profile"));
    assert!(pkce.is_some());
    assert!(url.contains("code_challenge="));
    assert!(url.contains("code_challenge_method=S256"));
}

#[test]
fn test_oauth_token_conversion() {
    let oauth_token = OAuthToken {
        access_token: "access_123".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: Some(3600),
        refresh_token: Some("refresh_456".to_string()),
        scope: Some("openid profile".to_string()),
        id_token: None,
    };

    let jwt_token = oauth_token.into_jwt_token().unwrap();
    assert_eq!(jwt_token.access_token(), "access_123");
    assert_eq!(jwt_token.refresh_token(), Some("refresh_456".to_string()));
    assert!(!jwt_token.is_expired());
}

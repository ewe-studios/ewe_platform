//! `UpstreamOidcClient` configuration and profile mapping (no network).
//! Moved from the inline test module of `src/shared/upstream_client.rs`.

use foundation_auth::{
    OAuthManager, ProviderMapping, ProviderType, UpstreamClientError, UpstreamOidcClient,
    UpstreamProfile, UpstreamProvider,
};

fn google_provider() -> UpstreamProvider {
    UpstreamProvider {
        id: "google".into(),
        name: "Google".into(),
        provider_type: ProviderType::Oidc,
        client_id: "test-client-id.apps.googleusercontent.com".into(),
        client_secret_ciphertext: None,
        encryption_key_id: "default".into(),
        authorization_url: None,
        token_url: None,
        userinfo_url: None,
        discovery_url: Some(
            "https://accounts.google.com/.well-known/openid-configuration".into(),
        ),
        scopes: vec!["openid".into(), "email".into(), "profile".into()],
        is_active: true,
        mapping_config: ProviderMapping::default(),
        created_at: 0,
        updated_at: 0,
    }
}

fn github_provider() -> UpstreamProvider {
    UpstreamProvider {
        id: "github".into(),
        name: "GitHub".into(),
        provider_type: ProviderType::Oauth2,
        client_id: "github-client-id".into(),
        client_secret_ciphertext: None,
        encryption_key_id: "default".into(),
        authorization_url: Some("https://github.com/login/oauth/authorize".into()),
        token_url: Some("https://github.com/login/oauth/access_token".into()),
        userinfo_url: Some("https://api.github.com/user".into()),
        discovery_url: None,
        scopes: vec!["user:email".into()],
        is_active: true,
        mapping_config: ProviderMapping {
            subject_field: "id".into(),
            email_field: "email".into(),
            email_verified_field: "email_verified".into(),
            name_field: "login".into(),
            username_field: None,
        },
        created_at: 0,
        updated_at: 0,
    }
}

#[test]
fn oidc_provider_builds_authorize_url_with_pkce() {
    // Configure from explicit endpoints for testing (no network)
    let mut provider = google_provider();
    provider.authorization_url = Some("https://accounts.google.com/o/oauth2/v2/auth".into());
    provider.token_url = Some("https://oauth2.googleapis.com/token".into());
    let mut client =
        UpstreamOidcClient::new(provider, "https://auth.example.com/callback".into());
    client.configure().expect("configure");

    let state = OAuthManager::generate_state();
    let (url, pkce) = client.authorize_url(&state).expect("authorize_url");

    assert!(url.contains("response_type=code"));
    assert!(url.contains("code_challenge="));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("state="));
    assert!(!pkce.code_verifier.is_empty());
}

#[test]
fn oauth2_provider_with_explicit_urls_configures() {
    let mut client = UpstreamOidcClient::new(
        github_provider(),
        "https://auth.example.com/callback".into(),
    );
    client.configure().expect("configure");
    assert_eq!(
        client.oauth_manager().config().authorization_url,
        "https://github.com/login/oauth/authorize"
    );
}

#[test]
fn provider_with_no_endpoints_returns_error() {
    let mut p = google_provider();
    p.discovery_url = None;
    let mut client =
        UpstreamOidcClient::new(p, "https://auth.example.com/callback".into());
    let err = client.configure().unwrap_err();
    assert!(matches!(err, UpstreamClientError::NoEndpoints));
}

#[test]
fn profile_extraction_from_userinfo() {
    let json = serde_json::json!({
        "sub": "12345",
        "email": "user@example.com",
        "email_verified": true,
        "name": "Test User",
        "preferred_username": "testuser",
    });
    let profile = UpstreamProfile::from_userinfo(&json, &ProviderMapping::default())
        .expect("extract profile");
    assert_eq!(profile.sub, "12345");
    assert_eq!(profile.email.unwrap(), "user@example.com");
    assert!(profile.email_verified);
    assert_eq!(profile.name.unwrap(), "Test User");
    assert_eq!(profile.preferred_username.unwrap(), "testuser");
}

#[test]
fn profile_extraction_uses_custom_mapping() {
    let json = serde_json::json!({
        "id": 42,
        "login": "ghuser",
        "email": "gh@example.com",
    });
    let mapping = ProviderMapping {
        subject_field: "login".into(),
        email_field: "email".into(),
        email_verified_field: "email_verified".into(),
        name_field: "login".into(),
        username_field: None,
    };
    let profile =
        UpstreamProfile::from_userinfo(&json, &mapping).expect("extract profile");
    assert_eq!(profile.sub, "ghuser");
    assert_eq!(profile.name.unwrap(), "ghuser");
}

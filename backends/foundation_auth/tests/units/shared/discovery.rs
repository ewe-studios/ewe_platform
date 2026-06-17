//! Shared discovery tests.

use foundation_auth::shared::discovery::OidcDiscovery;

#[test]
fn test_parse_discovery_json() {
    let json = r#"{
        "issuer": "https://auth.example.com",
        "authorization_endpoint": "https://auth.example.com/authorize",
        "token_endpoint": "https://auth.example.com/token",
        "userinfo_endpoint": "https://auth.example.com/userinfo",
        "jwks_uri": "https://auth.example.com/jwks",
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["EdDSA"],
        "scopes_supported": ["openid", "profile", "email"],
        "code_challenge_methods_supported": ["S256"]
    }"#;

    let discovery: OidcDiscovery = serde_json::from_str(json).unwrap();
    assert_eq!(discovery.issuer, "https://auth.example.com");
    assert_eq!(
        discovery.jwks_url(),
        "https://auth.example.com/jwks"
    );
}

#[test]
fn test_to_oauth_config() {
    let json = r#"{
        "issuer": "https://auth.example.com",
        "authorization_endpoint": "https://auth.example.com/authorize",
        "token_endpoint": "https://auth.example.com/token",
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["EdDSA"],
        "scopes_supported": ["openid", "profile", "email"],
        "code_challenge_methods_supported": ["S256"]
    }"#;

    let discovery: OidcDiscovery = serde_json::from_str(json).unwrap();
    let config = discovery.to_oauth_config("my-client", "https://app.com/callback");

    assert_eq!(
        config.authorization_url,
        "https://auth.example.com/authorize"
    );
    assert_eq!(config.token_url, "https://auth.example.com/token");
    assert!(config.pkce_enabled);
    assert!(config.scopes.contains(&"openid".to_string()));
}

//! IdpConfig and PasswordPolicy tests.

use foundation_auth::server::config::{IdpConfig, PasswordPolicy};
use std::time::Duration;

#[test]
fn test_default_config() {
    let config = IdpConfig::new("https://auth.example.com".into());
    assert_eq!(config.issuer_url, "https://auth.example.com");
    assert_eq!(config.access_token_ttl, Duration::from_secs(900));
    assert_eq!(config.refresh_token_ttl, Duration::from_secs(604_800));
    assert!(config.require_pkce);
}

#[test]
fn test_config_builder() {
    let config = IdpConfig::new("https://auth.example.com".into())
        .with_access_token_ttl(Duration::from_secs(1800))
        .with_require_pkce(false);
    assert_eq!(config.access_token_ttl, Duration::from_secs(1800));
    assert!(!config.require_pkce);
}

#[test]
fn test_password_policy_defaults() {
    let policy = PasswordPolicy::default();
    assert_eq!(policy.min_length, 12);
    assert!(policy.require_uppercase);
    assert_eq!(policy.max_failed_attempts, 5);
}

#[test]
fn test_config_debug() {
    let config = IdpConfig::new("https://auth.example.com".into());
    let debug = format!("{config:?}");
    assert!(debug.contains("IdpConfig"));
}

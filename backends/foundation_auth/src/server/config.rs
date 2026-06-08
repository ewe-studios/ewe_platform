//! IdP server configuration.

use std::time::Duration;

use crate::shared::jwt::JwtSigningKey;

#[derive(Debug, Clone)]
pub struct PasswordPolicy {
    pub min_length: usize,
    pub require_uppercase: bool,
    pub require_lowercase: bool,
    pub require_number: bool,
    pub require_special: bool,
    pub max_failed_attempts: u32,
    pub lockout_duration: Duration,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 12,
            require_uppercase: true,
            require_lowercase: true,
            require_number: true,
            require_special: true,
            max_failed_attempts: 5,
            lockout_duration: Duration::from_secs(900),
        }
    }
}

pub struct IdpConfig {
    pub issuer_url: String,
    pub signing_key: JwtSigningKey,
    pub access_token_ttl: Duration,
    pub refresh_token_ttl: Duration,
    pub session_ttl: Duration,
    pub auth_code_ttl: Duration,
    pub device_code_ttl: Duration,
    pub device_code_interval: u32,
    pub require_pkce: bool,
    pub password_policy: PasswordPolicy,
}

impl IdpConfig {
    #[must_use]
    pub fn new(issuer_url: String) -> Self {
        Self {
            issuer_url,
            signing_key: JwtSigningKey::generate_ed25519(),
            access_token_ttl: Duration::from_secs(900),
            refresh_token_ttl: Duration::from_secs(604_800),
            session_ttl: Duration::from_secs(28_800),
            auth_code_ttl: Duration::from_secs(600),
            device_code_ttl: Duration::from_secs(600),
            device_code_interval: 5,
            require_pkce: true,
            password_policy: PasswordPolicy::default(),
        }
    }

    #[must_use]
    pub fn with_signing_key(mut self, key: JwtSigningKey) -> Self {
        self.signing_key = key;
        self
    }

    #[must_use]
    pub fn with_access_token_ttl(mut self, ttl: Duration) -> Self {
        self.access_token_ttl = ttl;
        self
    }

    #[must_use]
    pub fn with_refresh_token_ttl(mut self, ttl: Duration) -> Self {
        self.refresh_token_ttl = ttl;
        self
    }

    #[must_use]
    pub fn with_session_ttl(mut self, ttl: Duration) -> Self {
        self.session_ttl = ttl;
        self
    }

    #[must_use]
    pub fn with_require_pkce(mut self, require: bool) -> Self {
        self.require_pkce = require;
        self
    }

    #[must_use]
    pub fn with_password_policy(mut self, policy: PasswordPolicy) -> Self {
        self.password_policy = policy;
        self
    }
}

impl core::fmt::Debug for IdpConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IdpConfig")
            .field("issuer_url", &self.issuer_url)
            .field("access_token_ttl", &self.access_token_ttl)
            .field("refresh_token_ttl", &self.refresh_token_ttl)
            .field("session_ttl", &self.session_ttl)
            .field("require_pkce", &self.require_pkce)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

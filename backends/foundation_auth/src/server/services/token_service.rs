//! Token generation and signing service.

use std::sync::Arc;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use rand::RngCore;
use serde::{Deserialize, Serialize};

use super::super::config::IdpConfig;
use super::super::models::{OAuthClient, User};
use super::super::models::client::hex_sha256;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub id_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub scope: String,
}

#[derive(Debug)]
pub enum TokenServiceError {
    SigningFailed(String),
    Storage(String),
    InvalidToken,
}

impl core::fmt::Display for TokenServiceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SigningFailed(s) => write!(f, "Token signing failed: {s}"),
            Self::Storage(s) => write!(f, "Storage error: {s}"),
            Self::InvalidToken => write!(f, "Invalid token"),
        }
    }
}

impl std::error::Error for TokenServiceError {}

pub struct TokenService {
    config: Arc<IdpConfig>,
}

impl TokenService {
    #[must_use]
    pub fn new(config: Arc<IdpConfig>) -> Self {
        Self { config }
    }

    pub fn generate_tokens(
        &self,
        user: &User,
        client: &OAuthClient,
        scope: &str,
        nonce: Option<&str>,
    ) -> Result<TokenPair, TokenServiceError> {
        let now = Utc::now().timestamp();
        let expires_in = self.config.access_token_ttl.as_secs();
        let exp = now + expires_in as i64;
        let jti = uuid::Uuid::new_v4().to_string();

        let access_claims = serde_json::json!({
            "iss": self.config.issuer_url,
            "sub": user.id,
            "aud": client.id,
            "exp": exp,
            "iat": now,
            "jti": jti,
            "scope": scope,
            "client_id": client.id,
        });

        let access_token = self
            .config
            .signing_key
            .sign_claims(&access_claims)
            .map_err(|e| TokenServiceError::SigningFailed(e.to_string()))?;

        let mut id_claims = serde_json::json!({
            "iss": self.config.issuer_url,
            "sub": user.id,
            "aud": client.id,
            "exp": exp,
            "iat": now,
            "email": user.email,
            "email_verified": user.email_verified,
        });

        if let Some(ref name) = user.username {
            id_claims["name"] = serde_json::Value::String(name.clone());
        }
        if let Some(nonce) = nonce {
            id_claims["nonce"] = serde_json::Value::String(nonce.to_string());
        }

        let id_token = self
            .config
            .signing_key
            .sign_claims(&id_claims)
            .map_err(|e| TokenServiceError::SigningFailed(e.to_string()))?;

        let refresh_token = generate_refresh_token();

        Ok(TokenPair {
            access_token,
            id_token,
            refresh_token,
            expires_in,
            scope: scope.to_string(),
        })
    }

    pub fn generate_client_credentials_tokens(
        &self,
        client: &OAuthClient,
        scope: &str,
    ) -> Result<TokenPair, TokenServiceError> {
        let now = Utc::now().timestamp();
        let expires_in = self.config.access_token_ttl.as_secs();
        let exp = now + expires_in as i64;
        let jti = uuid::Uuid::new_v4().to_string();

        let claims = serde_json::json!({
            "iss": self.config.issuer_url,
            "sub": client.id,
            "aud": client.id,
            "exp": exp,
            "iat": now,
            "jti": jti,
            "scope": scope,
            "client_id": client.id,
        });

        let access_token = self
            .config
            .signing_key
            .sign_claims(&claims)
            .map_err(|e| TokenServiceError::SigningFailed(e.to_string()))?;

        Ok(TokenPair {
            access_token,
            id_token: String::new(),
            refresh_token: String::new(),
            expires_in,
            scope: scope.to_string(),
        })
    }

    #[must_use]
    pub fn hash_refresh_token(token: &str) -> String {
        hex_sha256(token)
    }
}

fn generate_refresh_token() -> String {
    let mut bytes = [0u8; 64];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

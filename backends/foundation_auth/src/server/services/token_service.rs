//! Token generation and signing service.

use std::sync::Arc;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use foundation_compact::rng::Rng;
use foundation_core::valtron::Ctx;
use serde::{Deserialize, Serialize};

use crate::shared::clock::now_seconds;

use super::super::config::IdpConfig;
use super::super::models::client::hex_sha256;
use super::super::models::{OAuthClient, User};

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
    ctx: Ctx,
}

impl TokenService {
    #[must_use]
    pub fn new(config: Arc<IdpConfig>, ctx: Ctx) -> Self {
        Self { config, ctx }
    }

    /// Generate tokens.
    ///
    /// # Errors
    ///
    /// Returns [`TokenServiceError`] if the operation fails.
    pub fn generate_tokens(
        &self,
        user: &User,
        client: &OAuthClient,
        scope: &str,
        nonce: Option<&str>,
    ) -> Result<TokenPair, TokenServiceError> {
        let now = now_seconds(&self.ctx);
        let expires_in = self.config.access_token_ttl.as_secs();
        let exp = now.saturating_add(i64::try_from(expires_in).unwrap_or(i64::MAX));
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
            .sign_claims(&access_claims, &self.ctx)
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
            .sign_claims(&id_claims, &self.ctx)
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

    /// Generate client credentials tokens.
    ///
    /// # Errors
    ///
    /// Returns [`TokenServiceError`] if the operation fails.
    pub fn generate_client_credentials_tokens(
        &self,
        client: &OAuthClient,
        scope: &str,
    ) -> Result<TokenPair, TokenServiceError> {
        let now = now_seconds(&self.ctx);
        let expires_in = self.config.access_token_ttl.as_secs();
        let exp = now.saturating_add(i64::try_from(expires_in).unwrap_or(i64::MAX));
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
            .sign_claims(&claims, &self.ctx)
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

    /// Verify an access token's signature and return the decoded claims.
    /// Returns `InvalidToken` if the signature is invalid or the token is expired.
    ///
    /// # Errors
    ///
    /// Returns [`TokenServiceError`] if the operation fails.
    pub fn verify_access_token(&self, token: &str) -> Result<serde_json::Value, TokenServiceError> {
        use jwt_simple::prelude::{Ed25519PublicKey, EdDSAPublicKeyLike};
        let public_pem = self
            .config
            .signing_key
            .public_key_pem()
            .map_err(|e| TokenServiceError::SigningFailed(e.to_string()))?;
        let crate::shared::jwt::JwtSigningKey::Ed25519(_) = &self.config.signing_key else {
            return Err(TokenServiceError::SigningFailed(
                "Only Ed25519 verification is supported".into(),
            ));
        };
        let pk = Ed25519PublicKey::from_pem(&public_pem)
            .map_err(|e| TokenServiceError::SigningFailed(e.to_string()))?;
        let claims: jwt_simple::claims::JWTClaims<serde_json::Value> = pk
            .verify_token(token, None)
            .map_err(|_| TokenServiceError::InvalidToken)?;
        // Convert jwt-simple Claims to serde_json::Value
        let subject = claims.subject.ok_or(TokenServiceError::InvalidToken)?;
        let issuer = claims.issuer.ok_or(TokenServiceError::InvalidToken)?;
        let aud = match &claims.audiences {
            Some(a) => match a {
                jwt_simple::claims::Audiences::AsString(s) => s.clone(),
                jwt_simple::claims::Audiences::AsSet(set) => {
                    set.iter().next().cloned().unwrap_or_default()
                }
            },
            None => String::new(),
        };
        let exp = claims.expires_at.map_or(0, |e| e.as_secs());
        let iat = claims.issued_at.map_or(0, |t| t.as_secs());
        let jti = claims.jwt_id.unwrap_or_default();
        let scope = claims
            .custom
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let client_id = claims
            .custom
            .get("client_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let nonce = claims
            .custom
            .get("nonce")
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string);

        let mut map = serde_json::Map::new();
        map.insert("sub".into(), serde_json::Value::String(subject));
        map.insert("iss".into(), serde_json::Value::String(issuer));
        map.insert("aud".into(), serde_json::Value::String(aud));
        map.insert(
            "exp".into(),
            serde_json::Value::Number(serde_json::Number::from(exp)),
        );
        map.insert(
            "iat".into(),
            serde_json::Value::Number(serde_json::Number::from(iat)),
        );
        map.insert("jti".into(), serde_json::Value::String(jti));
        map.insert("scope".into(), serde_json::Value::String(scope));
        map.insert("client_id".into(), serde_json::Value::String(client_id));
        if let Some(n) = nonce {
            map.insert("nonce".into(), serde_json::Value::String(n));
        }
        // Copy any extra custom claims from the JSON object
        if let Some(obj) = claims.custom.as_object() {
            for (k, v) in obj {
                if !matches!(k.as_str(), "scope" | "client_id" | "nonce") {
                    map.insert(k.clone(), v.clone());
                }
            }
        }
        Ok(serde_json::Value::Object(map))
    }
}

fn generate_refresh_token() -> String {
    let mut bytes = [0u8; 64];
    foundation_compact::rng::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

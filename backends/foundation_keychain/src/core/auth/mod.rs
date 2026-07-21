//! Bitwarden-specific auth adapters over foundation_auth (spec-57, F008).
//!
//! WHY: foundation_auth provides general JWT/TOTP/PBKDF2; Bitwarden adds
//! specific claims (`sstamp` for security stamp, `orgowner`/`orgadmin` for
//! org roles) and a distinctive login response shape (`Key`, `PrivateKey`,
//! `Kdf`, `UserDecryptionOptions`).
//!
//! WHAT: `BitwardenClaims` maps onto `VerifiedClaims::custom`. The security
//! stamp middleware checks `sstamp` against the DB after JWT verify. The
//! token response adapter builds the full `LoginResponse`.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;

use foundation_auth::shared::jwt::{JwtSigningKey, JwtVerifier};

use crate::core::error::{AppError, AppResult};
use crate::core::store::users::UserRow;

/// JWT issuer for keychain-minted tokens.
pub const ISSUER: &str = "urn:foundation_keychain";
/// Access token lifetime (1 hour).
pub const ACCESS_TOKEN_TTL_SECS: i64 = 3600;
/// Refresh token lifetime (30 days).
pub const REFRESH_TOKEN_TTL_SECS: i64 = 30 * 24 * 3600;
/// `type` claim value for access tokens.
pub const TYPE_ACCESS: &str = "access_token";
/// `type` claim value for refresh tokens.
pub const TYPE_REFRESH: &str = "refresh_token";

/// Mint a Bitwarden access token for `user` on `device_identifier`.
///
/// # Errors
///
/// Returns [`AppError::Internal`] if signing fails.
pub fn mint_access_token(
    signing_key: &JwtSigningKey,
    user: &UserRow,
    device_identifier: &str,
) -> AppResult<String> {
    let now = Utc::now().timestamp();
    let claims = json!({
        "sub": user.uuid,
        "iss": ISSUER,
        "type": TYPE_ACCESS,
        "nbf": now,
        "exp": now + ACCESS_TOKEN_TTL_SECS,
        "premium": true,
        "name": user.name,
        "email": user.email,
        "email_verified": user.email_verified,
        "sstamp": user.security_stamp,
        "device": device_identifier,
        "scope": ["api", "offline_access"],
    });
    signing_key
        .sign_claims(&claims)
        .map_err(|e| AppError::Internal(format!("sign access token: {e}")))
}

/// Mint a Bitwarden refresh token for `user_uuid` on `device_identifier`.
///
/// # Errors
///
/// Returns [`AppError::Internal`] if signing fails.
pub fn mint_refresh_token(
    signing_key: &JwtSigningKey,
    user_uuid: &str,
    device_identifier: &str,
) -> AppResult<String> {
    let now = Utc::now().timestamp();
    let claims = json!({
        "sub": user_uuid,
        "iss": ISSUER,
        "type": TYPE_REFRESH,
        "nbf": now,
        "exp": now + REFRESH_TOKEN_TTL_SECS,
        "device": device_identifier,
    });
    signing_key
        .sign_claims(&claims)
        .map_err(|e| AppError::Internal(format!("sign refresh token: {e}")))
}

/// The authenticated principal extracted from a verified access token.
#[derive(Debug, Clone)]
pub struct AuthedToken {
    pub user_uuid: String,
    pub security_stamp: Option<String>,
    pub device: Option<String>,
}

fn custom_str(
    claims: &foundation_auth::shared::jwt::VerifiedClaims,
    key: &str,
) -> Option<String> {
    claims.custom.get(key).and_then(|v| v.as_str()).map(String::from)
}

/// Verify an access token and extract the authenticated principal.
///
/// The security stamp is returned but NOT checked against the DB here — callers
/// that need stamp enforcement pass it to [`verify_security_stamp`] after loading
/// the user (a token minted before a password change must be rejected).
///
/// # Errors
///
/// Returns [`AppError::Unauthorized`] if the signature/expiry is invalid or the
/// token is not an access token.
pub fn verify_access_token(verifier: &JwtVerifier, token: &str) -> AppResult<AuthedToken> {
    let claims = verifier.verify(token).map_err(|_| AppError::Unauthorized)?;
    if custom_str(&claims, "type").as_deref() != Some(TYPE_ACCESS) {
        return Err(AppError::Unauthorized);
    }
    let security_stamp = custom_str(&claims, "sstamp");
    let device = custom_str(&claims, "device");
    Ok(AuthedToken {
        user_uuid: claims.sub,
        security_stamp,
        device,
    })
}

/// Verify a refresh token, returning `(user_uuid, device_identifier)`.
///
/// # Errors
///
/// Returns [`AppError::Unauthorized`] if invalid or not a refresh token.
pub fn verify_refresh_token(
    verifier: &JwtVerifier,
    token: &str,
) -> AppResult<(String, Option<String>)> {
    let claims = verifier.verify(token).map_err(|_| AppError::Unauthorized)?;
    if custom_str(&claims, "type").as_deref() != Some(TYPE_REFRESH) {
        return Err(AppError::Unauthorized);
    }
    let device = custom_str(&claims, "device");
    Ok((claims.sub, device))
}

/// Bitwarden-specific claims carried inside the JWT `custom` map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitwardenClaims {
    /// Security stamp — changes on password rotation, invalidates old tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sstamp: Option<String>,
    /// Whether the user is an org owner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orgowner: Option<String>,
    /// Whether the user is an org admin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orgadmin: Option<String>,
    /// Whether the user is an org user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orguser: Option<String>,
    /// Whether the user is an org manager.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orgmanager: Option<String>,
    /// Premium flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub premium: Option<bool>,
    /// Device identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

/// Verify that the JWT's security stamp matches the user's current stamp.
///
/// Returns `true` if the token is still valid (stamps match or no stamp claim).
#[must_use]
pub fn verify_security_stamp(token_stamp: Option<&str>, db_stamp: &str) -> bool {
    match token_stamp {
        Some(ts) => ts == db_stamp,
        None => true, // tokens without sstamp pre-date the mechanism — allow
    }
}

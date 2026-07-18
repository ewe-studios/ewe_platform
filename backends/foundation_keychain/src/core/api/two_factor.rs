//! Two-factor (TOTP) API — portable handlers (spec-57, F008 Stage 1).
//!
//! WHY: authenticator-app 2FA. Ported from OrangeVault `api/two_factor.rs`,
//! decoupled from any transport. The TOTP algorithm is
//! `foundation_auth::TOTPSecret` (decision 01); this module owns only the setup
//! flow and secret storage.
//!
//! WHAT: `get_authenticator` (fetch/generate the secret), `enable_authenticator`
//! (verify a code, turn 2FA on, issue a recovery code), `disable_authenticator`,
//! and `verify` (used by the login 2FA challenge — accepts a TOTP code or the
//! recovery code).
//!
//! HOW: the secret is a base32-encoded 256-bit random key; verification rebuilds
//! `TOTPSecret::from_bytes` from the decoded secret.

use base32::Alphabet;
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use foundation_auth::shared::two_factor::TOTPSecret;

use crate::core::context::KeychainContext;
use crate::core::error::{AppError, AppResult};
use crate::core::store::two_factor as store;
use crate::core::store::two_factor::TwoFactorRow;

const ALPHABET: Alphabet = Alphabet::Rfc4648 { padding: false };
/// TOTP verification window (±1 time step for clock drift).
const TOLERANCE: u64 = 1;

/// `GET /api/two-factor/get-authenticator` response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticatorResponse {
    pub key: String,
    pub enabled: bool,
    pub object: &'static str,
}

/// `POST /api/two-factor/authenticator` (enable) response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnableAuthenticatorResponse {
    pub enabled: bool,
    pub key: String,
    pub recovery_code: String,
}

fn new_secret_base32() -> String {
    // 256-bit random secret from two UUID v4s (cross-platform getrandom).
    let mut bytes = Uuid::new_v4().as_bytes().to_vec();
    bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    base32::encode(ALPHABET, &bytes)
}

fn totp_from_base32(secret: &str) -> AppResult<TOTPSecret> {
    let bytes = base32::decode(ALPHABET, secret)
        .ok_or_else(|| AppError::Internal("stored TOTP secret is not valid base32".into()))?;
    Ok(TOTPSecret::from_bytes(bytes))
}

/// Fetch the user's authenticator secret, generating (and persisting, disabled) a
/// fresh one if none exists yet.
pub async fn get_authenticator(
    ctx: &KeychainContext,
    user_uuid: &str,
) -> AppResult<AuthenticatorResponse> {
    if let Some(existing) = store::get(ctx.db(), user_uuid).await? {
        return Ok(AuthenticatorResponse {
            key: existing.secret,
            enabled: existing.enabled,
            object: "twoFactorAuthenticator",
        });
    }
    let now = Utc::now();
    let row = TwoFactorRow {
        user_uuid: user_uuid.to_string(),
        secret: new_secret_base32(),
        enabled: false,
        recovery_code: None,
        created_at: now,
        updated_at: now,
    };
    store::upsert(ctx.db(), &row).await?;
    Ok(AuthenticatorResponse {
        key: row.secret,
        enabled: false,
        object: "twoFactorAuthenticator",
    })
}

/// Enable 2FA after verifying a current code. Issues a recovery code.
pub async fn enable_authenticator(
    ctx: &KeychainContext,
    user_uuid: &str,
    token: &str,
) -> AppResult<EnableAuthenticatorResponse> {
    let mut row = store::get(ctx.db(), user_uuid)
        .await?
        .ok_or_else(|| AppError::BadRequest("call get-authenticator first".into()))?;

    if !totp_from_base32(&row.secret)?.verify(token, TOLERANCE) {
        return Err(AppError::BadRequest("invalid verification code".into()));
    }

    let recovery_code = Uuid::new_v4().to_string().replace('-', "");
    row.enabled = true;
    row.recovery_code = Some(recovery_code.clone());
    row.updated_at = Utc::now();
    store::upsert(ctx.db(), &row).await?;

    Ok(EnableAuthenticatorResponse {
        enabled: true,
        key: row.secret,
        recovery_code,
    })
}

/// Disable 2FA after verifying a current code.
pub async fn disable_authenticator(
    ctx: &KeychainContext,
    user_uuid: &str,
    token: &str,
) -> AppResult<()> {
    let row = store::get(ctx.db(), user_uuid)
        .await?
        .ok_or_else(|| AppError::BadRequest("two-factor is not configured".into()))?;
    if !totp_from_base32(&row.secret)?.verify(token, TOLERANCE) {
        return Err(AppError::BadRequest("invalid verification code".into()));
    }
    store::delete(ctx.db(), user_uuid).await
}

/// Whether the user has 2FA enabled (login must issue a challenge).
pub async fn is_enabled(ctx: &KeychainContext, user_uuid: &str) -> AppResult<bool> {
    Ok(store::get(ctx.db(), user_uuid)
        .await?
        .is_some_and(|r| r.enabled))
}

/// Verify a login 2FA response — accepts a TOTP code or the recovery code.
pub async fn verify(ctx: &KeychainContext, user_uuid: &str, code: &str) -> AppResult<bool> {
    let Some(row) = store::get(ctx.db(), user_uuid).await? else {
        return Ok(false);
    };
    if !row.enabled {
        return Ok(false);
    }
    if row.recovery_code.as_deref() == Some(code) {
        return Ok(true);
    }
    Ok(totp_from_base32(&row.secret)?.verify(code, TOLERANCE))
}

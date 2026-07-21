//! Accounts API — portable handlers (spec-57, F008 Stage 1).
//!
//! WHY: Bitwarden account lifecycle — prelogin (KDF params), registration
//! (server-side password verifier), profile read/update, and master-password
//! verification (the credential check `connect/token` and `verify-password` use).
//! Ported from OrangeVault `api/accounts.rs` + `api/identity.rs`, decoupled from
//! any transport.
//!
//! WHAT: `prelogin`, `register`, `get_profile`, `update_profile`,
//! `verify_master_password`.
//!
//! HOW: users persist via [`crate::core::store::users`]; the password verifier is
//! PBKDF2 over the client's `master_password_hash` (via [`crate::core::crypto`],
//! decision 01). Email is normalized (trim + lowercase). Prelogin returns default
//! KDF params for unknown emails to avoid user-enumeration.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::Utc;
use uuid::Uuid;

use crate::core::context::KeychainContext;
use crate::core::crypto;
use crate::core::error::{AppError, AppResult};
use crate::core::models::user::{
    KdfType, PreloginResponse, ProfileResponse, RegisterRequest, UpdateProfileRequest, UserAccount,
};
use crate::core::store::users as store;
use crate::core::store::users::UserRow;

/// Bitwarden default client KDF when a client doesn't specify one.
const DEFAULT_KDF_ITERATIONS: i32 = 600_000;
const DEFAULT_CULTURE: &str = "en-US";

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

fn kdf_from_i64(v: i64) -> KdfType {
    match v {
        1 => KdfType::Argon2id,
        _ => KdfType::Pbkdf2Sha256,
    }
}

fn to_account(row: &UserRow) -> UserAccount {
    UserAccount {
        id: row.uuid.clone(),
        email: row.email.clone(),
        name: row.name.clone(),
        email_verified: row.email_verified,
        culture: DEFAULT_CULTURE.to_string(),
        premium: false,
        two_factor_enabled: false,
        security_stamp: row.security_stamp.clone(),
    }
}

fn to_profile(row: &UserRow) -> ProfileResponse {
    ProfileResponse {
        id: row.uuid.clone(),
        email: row.email.clone(),
        name: row.name.clone(),
        email_verified: row.email_verified,
        culture: DEFAULT_CULTURE.to_string(),
        premium: false,
        two_factor_enabled: false,
        security_stamp: row.security_stamp.clone(),
        organizations: Vec::new(),
    }
}

/// `POST /identity/accounts/prelogin` — KDF params for the email, or defaults.
pub async fn prelogin(ctx: &KeychainContext, email: &str) -> AppResult<PreloginResponse> {
    let email = normalize_email(email);
    match store::find_by_email(ctx.db(), &email).await? {
        Some(user) => Ok(PreloginResponse {
            kdf: kdf_from_i64(user.client_kdf_type),
            kdf_iterations: user.client_kdf_iter as i32,
            kdf_memory: user.client_kdf_memory.map(|v| v as i32),
            kdf_parallelism: user.client_kdf_parallelism.map(|v| v as i32),
        }),
        // Unknown user — return defaults so existence isn't leaked.
        None => Ok(PreloginResponse {
            kdf: KdfType::Pbkdf2Sha256,
            kdf_iterations: DEFAULT_KDF_ITERATIONS,
            kdf_memory: None,
            kdf_parallelism: None,
        }),
    }
}

/// `POST /identity/accounts/register` — create an account with a server-side
/// password verifier. Fails with `Conflict` if the email is taken.
pub async fn register(ctx: &KeychainContext, req: RegisterRequest) -> AppResult<UserAccount> {
    let email = normalize_email(&req.email);
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::BadRequest("a valid email is required".into()));
    }
    if req.master_password_hash.is_empty() {
        return Err(AppError::BadRequest("master password hash is required".into()));
    }
    if store::find_by_email(ctx.db(), &email).await?.is_some() {
        return Err(AppError::Conflict("email already registered".into()));
    }

    let salt = crypto::random_salt();
    let hash = crypto::derive_password_hash(
        req.master_password_hash.as_bytes(),
        &salt,
        crypto::SERVER_ITERATIONS,
    )
    .await?;

    let now = Utc::now();
    let kdf = req.kdf.unwrap_or(KdfType::Pbkdf2Sha256);
    let row = UserRow {
        uuid: Uuid::new_v4().to_string(),
        email,
        name: req.name,
        password_hash: STANDARD.encode(&hash),
        salt: STANDARD.encode(&salt),
        password_iterations: i64::from(crypto::SERVER_ITERATIONS),
        security_stamp: Uuid::new_v4().to_string(),
        akey: req.key,
        client_kdf_type: kdf as i64,
        client_kdf_iter: i64::from(req.kdf_iterations.unwrap_or(DEFAULT_KDF_ITERATIONS)),
        client_kdf_memory: req.kdf_memory.map(i64::from),
        client_kdf_parallelism: req.kdf_parallelism.map(i64::from),
        master_password_hint: req.master_password_hint,
        email_verified: false,
        created_at: now,
        updated_at: now,
    };
    store::insert(ctx.db(), &row).await?;
    Ok(to_account(&row))
}

/// `GET /api/accounts/profile` — the user's profile.
pub async fn get_profile(ctx: &KeychainContext, user_uuid: &str) -> AppResult<ProfileResponse> {
    let user = store::find_by_uuid(ctx.db(), user_uuid)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(to_profile(&user))
}

/// The user's account summary (the `profile` block of a sync payload).
pub async fn account(ctx: &KeychainContext, user_uuid: &str) -> AppResult<UserAccount> {
    let user = store::find_by_uuid(ctx.db(), user_uuid)
        .await?
        .ok_or(AppError::Unauthorized)?;
    Ok(to_account(&user))
}

/// `PUT /api/accounts/profile` — update mutable profile fields.
pub async fn update_profile(
    ctx: &KeychainContext,
    user_uuid: &str,
    req: UpdateProfileRequest,
) -> AppResult<ProfileResponse> {
    let mut user = store::find_by_uuid(ctx.db(), user_uuid)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let now = Utc::now();
    if req.name.is_some() {
        user.name = req.name;
    }
    if req.master_password_hint.is_some() {
        user.master_password_hint = req.master_password_hint;
    }
    store::update_profile(ctx.db(), user_uuid, &user.name, &user.master_password_hint, now).await?;
    user.updated_at = now;
    Ok(to_profile(&user))
}

/// Verify a client-provided `master_password_hash` against the stored verifier.
///
/// Used by `POST /api/accounts/verify-password` and the `connect/token` password
/// grant. Returns `false` for a wrong password; `Unauthorized` for no such user.
pub async fn verify_master_password(
    ctx: &KeychainContext,
    user_uuid: &str,
    master_password_hash: &str,
) -> AppResult<bool> {
    let user = store::find_by_uuid(ctx.db(), user_uuid)
        .await?
        .ok_or(AppError::Unauthorized)?;
    verify_user_password(&user, master_password_hash).await
}

/// Verify a password against an already-loaded user row.
pub async fn verify_user_password(user: &UserRow, master_password_hash: &str) -> AppResult<bool> {
    let expected = STANDARD
        .decode(&user.password_hash)
        .map_err(|e| AppError::Internal(format!("stored hash decode: {e}")))?;
    let salt = STANDARD
        .decode(&user.salt)
        .map_err(|e| AppError::Internal(format!("stored salt decode: {e}")))?;
    crypto::verify_password_hash(
        master_password_hash.as_bytes(),
        &salt,
        &expected,
        user.password_iterations as u32,
    )
    .await
}

//! App registry handlers (feature 009, native).
//!
//! An app registers once and receives a one-time secret of the form
//! `{app_uuid}.{random}`. The secret is Argon2id-hashed at rest; on every
//! provisioning call the app presents it and we verify + resolve the app uuid.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use foundation_auth::shared::password_hash::{
    argon2id_derive, argon2id_verify, ARGON2ID_ITERATIONS, ARGON2ID_MEMORY_KB, ARGON2ID_PARALLELISM,
};

use crate::core::context::KeychainContext;
use crate::core::error::{AppError, AppResult};
use crate::core::provisioning::store::{self, AppRow};

const KEY_LEN: u32 = 32;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAppRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAppResponse {
    pub app_id: String,
    pub name: String,
    /// Shown once; the server only stores its Argon2id hash.
    pub secret: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
}

fn random_secret_value() -> String {
    let mut bytes = Uuid::new_v4().as_bytes().to_vec();
    bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    STANDARD.encode(&bytes)
}

fn hash_secret(secret: &str, salt: &[u8]) -> String {
    let key = argon2id_derive(
        secret.as_bytes(),
        salt,
        ARGON2ID_ITERATIONS,
        ARGON2ID_MEMORY_KB,
        ARGON2ID_PARALLELISM,
        KEY_LEN,
    );
    STANDARD.encode(key.as_bytes())
}

/// `POST /api/apps/register` — create an app, returning the one-time secret.
pub async fn register(
    ctx: &KeychainContext,
    req: RegisterAppRequest,
) -> AppResult<RegisterAppResponse> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("app name is required".into()));
    }
    let uuid = Uuid::new_v4().to_string();
    // Secret embeds the uuid so it resolves the app on later calls.
    let secret = format!("{uuid}.{}", random_secret_value());
    let salt = Uuid::new_v4().as_bytes().to_vec();
    let now = Utc::now();

    let row = AppRow {
        uuid: uuid.clone(),
        name: req.name,
        description: req.description,
        secret_hash: hash_secret(&secret, &salt),
        secret_salt: STANDARD.encode(&salt),
        created_at: now,
    };
    store::insert_app(ctx.db(), &row).await?;

    Ok(RegisterAppResponse {
        app_id: uuid,
        name: row.name,
        secret,
        created_at: now.to_rfc3339(),
    })
}

/// Resolve + verify an app secret, returning the app uuid.
pub async fn authenticate(ctx: &KeychainContext, secret: &str) -> AppResult<String> {
    let app_id = secret.split('.').next().unwrap_or_default();
    if app_id.is_empty() {
        return Err(AppError::Unauthorized);
    }
    let app = store::find_app(ctx.db(), app_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    let salt = STANDARD
        .decode(&app.secret_salt)
        .map_err(|e| AppError::Internal(format!("app salt decode: {e}")))?;
    let expected = STANDARD
        .decode(&app.secret_hash)
        .map_err(|e| AppError::Internal(format!("app hash decode: {e}")))?;
    let ok = argon2id_verify(
        secret.as_bytes(),
        &salt,
        &expected,
        ARGON2ID_ITERATIONS,
        ARGON2ID_MEMORY_KB,
        ARGON2ID_PARALLELISM,
        KEY_LEN,
    );
    if ok {
        Ok(app.uuid)
    } else {
        Err(AppError::Unauthorized)
    }
}

/// `GET /api/apps/:id` — app details (the caller must own it).
pub async fn get(ctx: &KeychainContext, app_id: &str, requested_id: &str) -> AppResult<AppResponse> {
    if app_id != requested_id {
        return Err(AppError::Forbidden);
    }
    let app = store::find_app(ctx.db(), app_id)
        .await?
        .ok_or_else(|| AppError::NotFound("app not found".into()))?;
    Ok(AppResponse {
        id: app.uuid,
        name: app.name,
        description: app.description,
        created_at: app.created_at.to_rfc3339(),
    })
}

/// `DELETE /api/apps/:id` — delete the app + its keys.
pub async fn delete(ctx: &KeychainContext, app_id: &str, requested_id: &str) -> AppResult<()> {
    if app_id != requested_id {
        return Err(AppError::Forbidden);
    }
    store::delete_app(ctx.db(), app_id).await
}

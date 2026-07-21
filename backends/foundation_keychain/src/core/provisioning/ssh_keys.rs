//! SSH key provisioning handlers (feature 009, native).
//!
//! Keys are generated server-side, the private key `age`-encrypted at rest with
//! the server master key, and returned in full only on create/get.

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::context::KeychainContext;
use crate::core::error::{AppError, AppResult};
use crate::core::provisioning::keygen;
use crate::core::provisioning::store::{self, SshKeyRow};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSshKeyRequest {
    pub name: String,
    pub key_type: Option<String>,
    pub comment: Option<String>,
    pub ttl_hours: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyResponse {
    pub id: String,
    pub name: String,
    pub key_type: String,
    pub public_key: String,
    /// Present only on create/get (decrypted); omitted from list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_key: Option<String>,
    pub comment: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
}

fn to_response(row: &SshKeyRow, private_key: Option<String>) -> SshKeyResponse {
    SshKeyResponse {
        id: row.uuid.clone(),
        name: row.name.clone(),
        key_type: row.key_type.clone(),
        public_key: row.public_key.clone(),
        private_key,
        comment: row.comment.clone(),
        created_at: row.created_at.to_rfc3339(),
        expires_at: row.expires_at.map(|d| d.to_rfc3339()),
    }
}

async fn owned(ctx: &KeychainContext, app_id: &str, key_id: &str) -> AppResult<SshKeyRow> {
    let key = store::find_key(ctx.db(), key_id)
        .await?
        .ok_or_else(|| AppError::NotFound("ssh key not found".into()))?;
    if key.app_uuid != app_id {
        return Err(AppError::Forbidden);
    }
    Ok(key)
}

/// `POST /api/credentials/ssh-keys` — generate + store a key for the app.
pub async fn create(
    ctx: &KeychainContext,
    master_key: &str,
    app_id: &str,
    req: CreateSshKeyRequest,
) -> AppResult<SshKeyResponse> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("key name is required".into()));
    }
    let comment = req.comment.clone().unwrap_or_default();
    let generated = keygen::generate(req.key_type.as_deref().unwrap_or("ed25519"), &comment)?;

    let now = Utc::now();
    let expires_at = req.ttl_hours.filter(|h| *h > 0).map(|h| now + Duration::hours(h));
    let row = SshKeyRow {
        uuid: Uuid::new_v4().to_string(),
        app_uuid: app_id.to_string(),
        name: req.name,
        key_type: generated.key_type,
        public_key: generated.public_openssh,
        private_key_encrypted: keygen::encrypt(master_key, generated.private_openssh.as_bytes())?,
        comment: req.comment,
        expires_at,
        created_at: now,
    };
    store::insert_key(ctx.db(), &row).await?;
    Ok(to_response(&row, Some(generated.private_openssh)))
}

/// `GET /api/credentials/ssh-keys` — the app's keys (public only).
pub async fn list(ctx: &KeychainContext, app_id: &str) -> AppResult<Vec<SshKeyResponse>> {
    Ok(store::find_keys_by_app(ctx.db(), app_id)
        .await?
        .iter()
        .map(|row| to_response(row, None))
        .collect())
}

/// `GET /api/credentials/ssh-keys/:id` — one key with the decrypted private key.
pub async fn get(
    ctx: &KeychainContext,
    master_key: &str,
    app_id: &str,
    key_id: &str,
) -> AppResult<SshKeyResponse> {
    let row = owned(ctx, app_id, key_id).await?;
    let private = keygen::decrypt(master_key, &row.private_key_encrypted)?;
    let private_pem = String::from_utf8(private)
        .map_err(|e| AppError::Internal(format!("private key utf8: {e}")))?;
    Ok(to_response(&row, Some(private_pem)))
}

/// `DELETE /api/credentials/ssh-keys/:id` — delete an owned key.
pub async fn delete(ctx: &KeychainContext, app_id: &str, key_id: &str) -> AppResult<()> {
    owned(ctx, app_id, key_id).await?;
    store::delete_key(ctx.db(), key_id).await
}

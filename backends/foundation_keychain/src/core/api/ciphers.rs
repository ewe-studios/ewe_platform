//! Ciphers API — portable handlers (spec-57, F008 Stage 1).
//!
//! WHY: the vault items — logins, cards, identities, notes. Ported from
//! OrangeVault `api/ciphers.rs`, decoupled from any transport.
//!
//! WHAT: `list`/`create`/`get`/`update`/`delete` (hard) + `soft_delete`/`restore`
//! (trash), each returning the API [`Cipher`] (or an [`AppError`]). Ownership is
//! enforced.
//!
//! HOW: the client-encrypted cipher content is serialized to the `data` column
//! as JSON; server-authoritative metadata (id, type, favorite, folder, dates,
//! deleted state) lives in columns and is overlaid onto the [`Cipher`] on read.

use chrono::Utc;
use uuid::Uuid;

use crate::core::context::KeychainContext;
use crate::core::error::{AppError, AppResult};
use crate::core::models::cipher::{Cipher, CipherCreateRequest, CipherRepromptType, CipherUpdateRequest};
use crate::core::store::ciphers as store;
use crate::core::store::ciphers::CipherRow;

fn to_json(cipher: &Cipher) -> AppResult<String> {
    serde_json::to_string(cipher).map_err(|e| AppError::Internal(format!("serialize cipher: {e}")))
}

/// Reconstruct the API [`Cipher`] from a row: deserialize the stored content, then
/// overlay the server-authoritative fields.
fn to_cipher(row: &CipherRow) -> AppResult<Cipher> {
    let mut cipher: Cipher = serde_json::from_str(&row.data)
        .map_err(|e| AppError::Internal(format!("deserialize cipher: {e}")))?;
    cipher.id = row.uuid.clone();
    cipher.organization_id = row.organization_uuid.clone();
    cipher.favorite = row.favorite;
    cipher.revision_date = row.updated_at;
    cipher.creation_date = row.created_at;
    cipher.deleted_date = row.deleted_at;
    Ok(cipher)
}

/// Load a cipher and assert the caller owns it.
async fn owned(ctx: &KeychainContext, user_uuid: &str, id: &str) -> AppResult<CipherRow> {
    let row = store::find(ctx.db(), id)
        .await?
        .ok_or_else(|| AppError::NotFound("cipher not found".into()))?;
    if row.user_uuid != user_uuid {
        return Err(AppError::Forbidden);
    }
    Ok(row)
}

/// `GET /api/ciphers` — all of the user's ciphers (including trash).
pub async fn list(ctx: &KeychainContext, user_uuid: &str) -> AppResult<Vec<Cipher>> {
    store::find_by_user(ctx.db(), user_uuid)
        .await?
        .iter()
        .map(to_cipher)
        .collect()
}

/// `POST /api/ciphers` — create a cipher.
pub async fn create(
    ctx: &KeychainContext,
    user_uuid: &str,
    req: CipherCreateRequest,
) -> AppResult<Cipher> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("cipher name is required".into()));
    }
    let now = Utc::now();
    let uuid = Uuid::new_v4().to_string();
    let cipher = Cipher {
        id: uuid.clone(),
        organization_id: req.organization_id.clone(),
        cipher_type: req.cipher_type,
        name: req.name,
        notes: req.notes,
        fields: req.fields,
        login: req.login,
        card: req.card,
        identity: req.identity,
        secure_note: req.secure_note,
        data: req.data,
        favorite: req.favorite.unwrap_or(false),
        reprompt: CipherRepromptType::None,
        revision_date: now,
        creation_date: now,
        deleted_date: None,
    };
    let row = CipherRow {
        uuid,
        user_uuid: user_uuid.to_string(),
        organization_uuid: req.organization_id,
        atype: cipher.cipher_type as i64,
        folder_uuid: None,
        favorite: cipher.favorite,
        name: cipher.name.clone(),
        data: to_json(&cipher)?,
        deleted_at: None,
        created_at: now,
        updated_at: now,
    };
    store::insert(ctx.db(), &row).await?;
    to_cipher(&row)
}

/// `GET /api/ciphers/:id` — a single owned cipher.
pub async fn get(ctx: &KeychainContext, user_uuid: &str, id: &str) -> AppResult<Cipher> {
    to_cipher(&owned(ctx, user_uuid, id).await?)
}

/// `PUT /api/ciphers/:id` — update an owned cipher.
pub async fn update(
    ctx: &KeychainContext,
    user_uuid: &str,
    id: &str,
    req: CipherUpdateRequest,
) -> AppResult<Cipher> {
    let mut row = owned(ctx, user_uuid, id).await?;
    let mut cipher = to_cipher(&row)?;

    if let Some(name) = req.name {
        if name.trim().is_empty() {
            return Err(AppError::BadRequest("cipher name is required".into()));
        }
        cipher.name = name;
    }
    if req.notes.is_some() {
        cipher.notes = req.notes;
    }
    if req.fields.is_some() {
        cipher.fields = req.fields;
    }
    if req.login.is_some() {
        cipher.login = req.login;
    }
    if req.card.is_some() {
        cipher.card = req.card;
    }
    if req.identity.is_some() {
        cipher.identity = req.identity;
    }
    if req.secure_note.is_some() {
        cipher.secure_note = req.secure_note;
    }
    if req.data.is_some() {
        cipher.data = req.data;
    }
    if let Some(favorite) = req.favorite {
        cipher.favorite = favorite;
    }

    let now = Utc::now();
    cipher.revision_date = now;

    row.name = cipher.name.clone();
    row.favorite = cipher.favorite;
    row.atype = cipher.cipher_type as i64;
    if let Some(folder_id) = req.folder_id {
        row.folder_uuid = if folder_id.is_empty() { None } else { Some(folder_id) };
    }
    row.data = to_json(&cipher)?;
    row.updated_at = now;
    store::update(ctx.db(), &row).await?;
    to_cipher(&row)
}

/// `DELETE /api/ciphers/:id` — hard-delete.
pub async fn delete(ctx: &KeychainContext, user_uuid: &str, id: &str) -> AppResult<()> {
    owned(ctx, user_uuid, id).await?;
    store::delete(ctx.db(), id).await
}

/// `PUT /api/ciphers/:id/delete` — soft-delete (move to trash).
pub async fn soft_delete(ctx: &KeychainContext, user_uuid: &str, id: &str) -> AppResult<Cipher> {
    owned(ctx, user_uuid, id).await?;
    let now = Utc::now();
    store::soft_delete(ctx.db(), id, now).await?;
    to_cipher(&owned(ctx, user_uuid, id).await?)
}

/// `PUT /api/ciphers/:id/restore` — restore from trash.
pub async fn restore(ctx: &KeychainContext, user_uuid: &str, id: &str) -> AppResult<Cipher> {
    owned(ctx, user_uuid, id).await?;
    let now = Utc::now();
    store::restore(ctx.db(), id, now).await?;
    to_cipher(&owned(ctx, user_uuid, id).await?)
}

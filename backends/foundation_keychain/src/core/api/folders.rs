//! Folders API — portable handlers (spec-57, F008 Stage 1).
//!
//! WHY: Bitwarden folders CRUD, decoupled from any transport. Ported from
//! OrangeVault `api/folders.rs`, replacing `worker::RouteContext`/`Request` with
//! a [`KeychainContext`] + the authenticated `user_uuid` + a parsed request, so
//! the same logic serves both the native and Workers backends and is unit-testable.
//!
//! WHAT: `list`/`create`/`get`/`update`/`delete`, each returning the API
//! [`Folder`] model (or an [`AppError`]). Ownership is enforced: a folder that
//! belongs to another user is `Forbidden`, a missing one is `NotFound`.
//!
//! HOW: persistence via [`crate::core::store::folders`]; the transport layer is
//! responsible for auth extraction, JSON (de)serialization, and notifications.

use chrono::Utc;
use uuid::Uuid;

use crate::core::context::KeychainContext;
use crate::core::error::{AppError, AppResult};
use crate::core::models::folder::{Folder, FolderCreateRequest, FolderUpdateRequest};
use crate::core::store::folders as store;
use crate::core::store::folders::FolderRow;

fn to_api(row: FolderRow) -> Folder {
    Folder {
        id: row.uuid,
        name: row.name,
        revision_date: row.updated_at,
    }
}

fn validated_name(name: &str) -> AppResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("folder name is required".into()));
    }
    Ok(trimmed.to_string())
}

/// Load a folder and assert the caller owns it.
async fn owned(ctx: &KeychainContext, user_uuid: &str, id: &str) -> AppResult<FolderRow> {
    let row = store::find(ctx.db(), id)
        .await?
        .ok_or_else(|| AppError::NotFound("folder not found".into()))?;
    if row.user_uuid != user_uuid {
        return Err(AppError::Forbidden);
    }
    Ok(row)
}

/// `GET /api/folders` — all folders owned by the user.
pub async fn list(ctx: &KeychainContext, user_uuid: &str) -> AppResult<Vec<Folder>> {
    let rows = store::find_by_user(ctx.db(), user_uuid).await?;
    Ok(rows.into_iter().map(to_api).collect())
}

/// `POST /api/folders` — create a folder for the user.
pub async fn create(
    ctx: &KeychainContext,
    user_uuid: &str,
    req: FolderCreateRequest,
) -> AppResult<Folder> {
    let name = validated_name(&req.name)?;
    let now = Utc::now();
    let row = FolderRow {
        uuid: Uuid::new_v4().to_string(),
        user_uuid: user_uuid.to_string(),
        name,
        created_at: now,
        updated_at: now,
    };
    store::insert(ctx.db(), &row).await?;
    Ok(to_api(row))
}

/// `GET /api/folders/:id` — a single owned folder.
pub async fn get(ctx: &KeychainContext, user_uuid: &str, id: &str) -> AppResult<Folder> {
    Ok(to_api(owned(ctx, user_uuid, id).await?))
}

/// `PUT /api/folders/:id` — rename an owned folder.
pub async fn update(
    ctx: &KeychainContext,
    user_uuid: &str,
    id: &str,
    req: FolderUpdateRequest,
) -> AppResult<Folder> {
    let mut row = owned(ctx, user_uuid, id).await?;
    let name = validated_name(&req.name)?;
    let now = Utc::now();
    store::update_name(ctx.db(), id, &name, now).await?;
    row.name = name;
    row.updated_at = now;
    Ok(to_api(row))
}

/// `DELETE /api/folders/:id` — delete an owned folder.
pub async fn delete(ctx: &KeychainContext, user_uuid: &str, id: &str) -> AppResult<()> {
    owned(ctx, user_uuid, id).await?;
    store::delete(ctx.db(), id).await?;
    Ok(())
}

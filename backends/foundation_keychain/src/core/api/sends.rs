//! Sends API — portable handlers (spec-57, F008 Stage 1).
//!
//! WHY: time-limited, access-counted secure sharing. Ported from OrangeVault
//! `api/sends.rs`, decoupled from any transport.
//!
//! WHAT: owner CRUD (`list`/`create`/`get`/`update`/`delete`) plus anonymous
//! `access` which enforces disabled / expiry / deletion / max-access-count /
//! password and increments the counter.
//!
//! HOW: the send row is server-authoritative; the [`Send`] response never echoes
//! the access `password`.

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::core::context::KeychainContext;
use crate::core::error::{AppError, AppResult};
use crate::core::models::send::{Send, SendCreateRequest, SendType, SendUpdateRequest};
use crate::core::store::sends as store;
use crate::core::store::sends::SendRow;

/// Default send lifetime when the client doesn't specify a deletion date.
const DEFAULT_DELETION_DAYS: i64 = 7;

fn parse_date(s: &str) -> AppResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| AppError::BadRequest(format!("invalid date {s:?}: {e}")))
}

fn send_type_from_i64(v: i64) -> SendType {
    match v {
        1 => SendType::File,
        _ => SendType::Text,
    }
}

/// Build the API [`Send`] from a row. The access `password` is never echoed.
fn to_send(row: &SendRow) -> Send {
    Send {
        id: row.uuid.clone(),
        name: row.name.clone(),
        notes: row.notes.clone(),
        send_type: send_type_from_i64(row.atype),
        data: row.data.clone(),
        key: row.akey.clone(),
        password: None,
        max_access_count: row.max_access_count.map(|v| v as i32),
        access_count: row.access_count as i32,
        disabled: row.disabled,
        hide_email: row.hide_email,
        deletion_date: row.deletion_date,
        expiration_date: row.expiration_date,
        revision_date: row.updated_at,
    }
}

async fn owned(ctx: &KeychainContext, user_uuid: &str, id: &str) -> AppResult<SendRow> {
    let row = store::find(ctx.db(), id)
        .await?
        .ok_or_else(|| AppError::NotFound("send not found".into()))?;
    if row.user_uuid != user_uuid {
        return Err(AppError::Forbidden);
    }
    Ok(row)
}

/// `GET /api/sends` — the user's sends.
pub async fn list(ctx: &KeychainContext, user_uuid: &str) -> AppResult<Vec<Send>> {
    Ok(store::find_by_user(ctx.db(), user_uuid)
        .await?
        .iter()
        .map(to_send)
        .collect())
}

/// `POST /api/sends` — create a text/file send.
pub async fn create(
    ctx: &KeychainContext,
    user_uuid: &str,
    req: SendCreateRequest,
) -> AppResult<Send> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("send name is required".into()));
    }
    let now = Utc::now();
    let deletion_date = match req.deletion_date.as_deref() {
        Some(s) => parse_date(s)?,
        None => now + Duration::days(DEFAULT_DELETION_DAYS),
    };
    let expiration_date = req.expiration_date.as_deref().map(parse_date).transpose()?;

    let row = SendRow {
        uuid: Uuid::new_v4().to_string(),
        user_uuid: user_uuid.to_string(),
        atype: req.send_type as i64,
        name: req.name,
        notes: req.notes,
        data: req.data,
        akey: req.key,
        password: req.password,
        max_access_count: req.max_access_count.map(i64::from),
        access_count: 0,
        disabled: req.disabled.unwrap_or(false),
        hide_email: req.hide_email.unwrap_or(false),
        expiration_date,
        deletion_date,
        created_at: now,
        updated_at: now,
    };
    store::insert(ctx.db(), &row).await?;
    Ok(to_send(&row))
}

/// `GET /api/sends/:id` — an owned send.
pub async fn get(ctx: &KeychainContext, user_uuid: &str, id: &str) -> AppResult<Send> {
    Ok(to_send(&owned(ctx, user_uuid, id).await?))
}

/// `PUT /api/sends/:id` — update an owned send.
pub async fn update(
    ctx: &KeychainContext,
    user_uuid: &str,
    id: &str,
    req: SendUpdateRequest,
) -> AppResult<Send> {
    let mut row = owned(ctx, user_uuid, id).await?;
    if let Some(name) = req.name {
        if name.trim().is_empty() {
            return Err(AppError::BadRequest("send name is required".into()));
        }
        row.name = name;
    }
    if req.notes.is_some() {
        row.notes = req.notes;
    }
    if req.data.is_some() {
        row.data = req.data;
    }
    if req.key.is_some() {
        row.akey = req.key;
    }
    if req.password.is_some() {
        row.password = req.password;
    }
    if req.max_access_count.is_some() {
        row.max_access_count = req.max_access_count.map(i64::from);
    }
    if let Some(disabled) = req.disabled {
        row.disabled = disabled;
    }
    if let Some(hide_email) = req.hide_email {
        row.hide_email = hide_email;
    }
    row.updated_at = Utc::now();
    store::update(ctx.db(), &row).await?;
    Ok(to_send(&row))
}

/// `DELETE /api/sends/:id` — delete an owned send.
pub async fn delete(ctx: &KeychainContext, user_uuid: &str, id: &str) -> AppResult<()> {
    owned(ctx, user_uuid, id).await?;
    store::delete(ctx.db(), id).await
}

/// `POST /api/sends/access/:id` — anonymous access.
///
/// Enforces disabled / deletion / expiry / max-access-count / password, then
/// increments the access counter and returns the send (a "not found" is returned
/// for any access-denied case so a probe can't distinguish reasons).
pub async fn access(
    ctx: &KeychainContext,
    id: &str,
    password: Option<&str>,
) -> AppResult<Send> {
    let mut row = store::find(ctx.db(), id)
        .await?
        .ok_or_else(|| AppError::NotFound("send not found".into()))?;

    let now = Utc::now();
    let gone = row.disabled
        || row.deletion_date <= now
        || row.expiration_date.is_some_and(|e| e <= now)
        || row
            .max_access_count
            .is_some_and(|max| row.access_count >= max);
    if gone {
        return Err(AppError::NotFound("send not found".into()));
    }

    if let Some(required) = row.password.as_deref() {
        if password != Some(required) {
            return Err(AppError::Unauthorized);
        }
    }

    row.access_count += 1;
    store::increment_access(ctx.db(), id, row.access_count).await?;
    Ok(to_send(&row))
}

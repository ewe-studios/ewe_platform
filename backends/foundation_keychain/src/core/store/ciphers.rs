//! Cipher persistence (spec-57, F008 Stage 1).
//!
//! Ported from OrangeVault `db/queries.rs` (ciphers). The client-encrypted cipher
//! content is stored opaquely in `data` (JSON); the other columns are the
//! server-authoritative metadata used for listing, sync, folder assignment,
//! favorites, and soft-delete.

use chrono::{DateTime, Utc};

use foundation_db::core::storage_provider::{AsyncQueryStore, DataValue, SqlRow};

use crate::core::error::AppResult;
use crate::core::store::{parse_ts, store_err};

/// A `ciphers` table row.
#[derive(Debug, Clone)]
pub struct CipherRow {
    pub uuid: String,
    pub user_uuid: String,
    pub organization_uuid: Option<String>,
    pub atype: i64,
    pub folder_uuid: Option<String>,
    pub favorite: bool,
    pub name: String,
    pub data: String,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const COLUMNS: &str = "uuid, user_uuid, organization_uuid, atype, folder_uuid, favorite, \
     name, data, deleted_at, created_at, updated_at";

fn opt_text(o: &Option<String>) -> DataValue {
    match o {
        Some(s) => DataValue::Text(s.clone()),
        None => DataValue::Null,
    }
}

fn map_row(row: &SqlRow) -> AppResult<CipherRow> {
    let favorite: i64 = row.get_by_name("favorite").map_err(store_err)?;
    let deleted_at: Option<String> = row.get_by_name("deleted_at").map_err(store_err)?;
    let created_at: String = row.get_by_name("created_at").map_err(store_err)?;
    let updated_at: String = row.get_by_name("updated_at").map_err(store_err)?;
    Ok(CipherRow {
        uuid: row.get_by_name("uuid").map_err(store_err)?,
        user_uuid: row.get_by_name("user_uuid").map_err(store_err)?,
        organization_uuid: row.get_by_name("organization_uuid").map_err(store_err)?,
        atype: row.get_by_name("atype").map_err(store_err)?,
        folder_uuid: row.get_by_name("folder_uuid").map_err(store_err)?,
        favorite: favorite != 0,
        name: row.get_by_name("name").map_err(store_err)?,
        data: row.get_by_name("data").map_err(store_err)?,
        deleted_at: deleted_at.as_deref().map(parse_ts).transpose()?,
        created_at: parse_ts(&created_at)?,
        updated_at: parse_ts(&updated_at)?,
    })
}

/// Insert a new cipher.
pub async fn insert(db: &dyn AsyncQueryStore, row: &CipherRow) -> AppResult<()> {
    db.execute_async(
        "INSERT INTO ciphers \
         (uuid, user_uuid, organization_uuid, atype, folder_uuid, favorite, name, data, \
          deleted_at, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            DataValue::Text(row.uuid.clone()),
            DataValue::Text(row.user_uuid.clone()),
            opt_text(&row.organization_uuid),
            DataValue::Integer(row.atype),
            opt_text(&row.folder_uuid),
            DataValue::Integer(i64::from(row.favorite)),
            DataValue::Text(row.name.clone()),
            DataValue::Text(row.data.clone()),
            match &row.deleted_at {
                Some(d) => DataValue::Text(d.to_rfc3339()),
                None => DataValue::Null,
            },
            DataValue::Text(row.created_at.to_rfc3339()),
            DataValue::Text(row.updated_at.to_rfc3339()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

/// All ciphers owned by a user (including soft-deleted — the sync/list layer decides).
pub async fn find_by_user(db: &dyn AsyncQueryStore, user_uuid: &str) -> AppResult<Vec<CipherRow>> {
    let rows = db
        .query_async(
            &format!("SELECT {COLUMNS} FROM ciphers WHERE user_uuid = ? ORDER BY created_at"),
            &[DataValue::Text(user_uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.iter().map(map_row).collect()
}

/// A single cipher by uuid.
pub async fn find(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<Option<CipherRow>> {
    let rows = db
        .query_async(
            &format!("SELECT {COLUMNS} FROM ciphers WHERE uuid = ?"),
            &[DataValue::Text(uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.first().map(map_row).transpose()
}

/// Update the mutable content + metadata of a cipher.
pub async fn update(db: &dyn AsyncQueryStore, row: &CipherRow) -> AppResult<()> {
    db.execute_async(
        "UPDATE ciphers SET atype = ?, folder_uuid = ?, favorite = ?, name = ?, data = ?, \
         updated_at = ? WHERE uuid = ?",
        &[
            DataValue::Integer(row.atype),
            opt_text(&row.folder_uuid),
            DataValue::Integer(i64::from(row.favorite)),
            DataValue::Text(row.name.clone()),
            DataValue::Text(row.data.clone()),
            DataValue::Text(row.updated_at.to_rfc3339()),
            DataValue::Text(row.uuid.clone()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

/// Soft-delete: set `deleted_at` (moves the cipher to trash).
pub async fn soft_delete(db: &dyn AsyncQueryStore, uuid: &str, when: DateTime<Utc>) -> AppResult<()> {
    db.execute_async(
        "UPDATE ciphers SET deleted_at = ?, updated_at = ? WHERE uuid = ?",
        &[
            DataValue::Text(when.to_rfc3339()),
            DataValue::Text(when.to_rfc3339()),
            DataValue::Text(uuid.to_string()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

/// Restore a soft-deleted cipher (clears `deleted_at`).
pub async fn restore(db: &dyn AsyncQueryStore, uuid: &str, when: DateTime<Utc>) -> AppResult<()> {
    db.execute_async(
        "UPDATE ciphers SET deleted_at = NULL, updated_at = ? WHERE uuid = ?",
        &[
            DataValue::Text(when.to_rfc3339()),
            DataValue::Text(uuid.to_string()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

/// Hard-delete a cipher.
pub async fn delete(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<()> {
    db.execute_async(
        "DELETE FROM ciphers WHERE uuid = ?",
        &[DataValue::Text(uuid.to_string())],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

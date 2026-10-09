//! Folder persistence (spec-57, F008 Stage 1).
//!
//! Ported from OrangeVault `db/queries.rs` (folders) onto `foundation_db`'s
//! `AsyncQueryStore`. The row carries `user_uuid` so the handler layer can
//! enforce ownership.

use chrono::{DateTime, Utc};

use foundation_db::core::storage_provider::{AsyncQueryStore, DataValue, SqlRow};

use crate::core::error::AppResult;
use crate::core::store::{parse_ts, store_err};

/// A `folders` table row.
#[derive(Debug, Clone)]
pub struct FolderRow {
    pub uuid: String,
    pub user_uuid: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const COLUMNS: &str = "uuid, user_uuid, name, created_at, updated_at";

fn map_row(row: &SqlRow) -> AppResult<FolderRow> {
    let created_at: String = row.get_by_name("created_at").map_err(store_err)?;
    let updated_at: String = row.get_by_name("updated_at").map_err(store_err)?;
    Ok(FolderRow {
        uuid: row.get_by_name("uuid").map_err(store_err)?,
        user_uuid: row.get_by_name("user_uuid").map_err(store_err)?,
        name: row.get_by_name("name").map_err(store_err)?,
        created_at: parse_ts(&created_at)?,
        updated_at: parse_ts(&updated_at)?,
    })
}

/// Insert a new folder.
pub async fn insert(db: &dyn AsyncQueryStore, row: &FolderRow) -> AppResult<()> {
    db.execute_async(
        "INSERT INTO folders (uuid, user_uuid, name, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?)",
        &[
            DataValue::Text(row.uuid.clone()),
            DataValue::Text(row.user_uuid.clone()),
            DataValue::Text(row.name.clone()),
            DataValue::Text(row.created_at.to_rfc3339()),
            DataValue::Text(row.updated_at.to_rfc3339()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

/// All folders owned by a user, ordered by name.
pub async fn find_by_user(db: &dyn AsyncQueryStore, user_uuid: &str) -> AppResult<Vec<FolderRow>> {
    let rows = db
        .query_async(
            &format!("SELECT {COLUMNS} FROM folders WHERE user_uuid = ? ORDER BY name"),
            &[DataValue::Text(user_uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.iter().map(map_row).collect()
}

/// A single folder by uuid, if it exists.
pub async fn find(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<Option<FolderRow>> {
    let rows = db
        .query_async(
            &format!("SELECT {COLUMNS} FROM folders WHERE uuid = ?"),
            &[DataValue::Text(uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.first().map(map_row).transpose()
}

/// Rename a folder and bump its `updated_at`.
pub async fn update_name(
    db: &dyn AsyncQueryStore,
    uuid: &str,
    name: &str,
    updated_at: DateTime<Utc>,
) -> AppResult<()> {
    db.execute_async(
        "UPDATE folders SET name = ?, updated_at = ? WHERE uuid = ?",
        &[
            DataValue::Text(name.to_string()),
            DataValue::Text(updated_at.to_rfc3339()),
            DataValue::Text(uuid.to_string()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

/// Delete a folder by uuid.
pub async fn delete(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<()> {
    db.execute_async(
        "DELETE FROM folders WHERE uuid = ?",
        &[DataValue::Text(uuid.to_string())],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

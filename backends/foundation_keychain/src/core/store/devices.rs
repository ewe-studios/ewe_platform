//! Device persistence (spec-57, F008 Stage 1).
//!
//! A device is created on first login for a given client `identifier` and holds
//! that client's current refresh token. The `refresh_token` grant rotates it: a
//! presented refresh token must match the stored one, and a fresh one replaces it
//! (so a stolen-then-rotated token stops working).

use chrono::{DateTime, Utc};

use foundation_db::core::storage_provider::{AsyncQueryStore, DataValue, SqlRow};

use crate::core::error::AppResult;
use crate::core::store::{parse_ts, store_err};

/// A `vault_devices` row.
#[derive(Debug, Clone)]
pub struct DeviceRow {
    pub uuid: String,
    pub user_uuid: String,
    pub identifier: String,
    pub name: Option<String>,
    pub atype: i64,
    pub refresh_token: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const COLUMNS: &str = "uuid, user_uuid, identifier, name, atype, refresh_token, created_at, updated_at";

fn map_row(row: &SqlRow) -> AppResult<DeviceRow> {
    let created_at: String = row.get_by_name("created_at").map_err(store_err)?;
    let updated_at: String = row.get_by_name("updated_at").map_err(store_err)?;
    Ok(DeviceRow {
        uuid: row.get_by_name("uuid").map_err(store_err)?,
        user_uuid: row.get_by_name("user_uuid").map_err(store_err)?,
        identifier: row.get_by_name("identifier").map_err(store_err)?,
        name: row.get_by_name("name").map_err(store_err)?,
        atype: row.get_by_name("atype").map_err(store_err)?,
        refresh_token: row.get_by_name("refresh_token").map_err(store_err)?,
        created_at: parse_ts(&created_at)?,
        updated_at: parse_ts(&updated_at)?,
    })
}

/// Find a device by (user, client identifier).
pub async fn find(
    db: &dyn AsyncQueryStore,
    user_uuid: &str,
    identifier: &str,
) -> AppResult<Option<DeviceRow>> {
    let rows = db
        .query_async(
            &format!("SELECT {COLUMNS} FROM vault_devices WHERE user_uuid = ? AND identifier = ?"),
            &[
                DataValue::Text(user_uuid.to_string()),
                DataValue::Text(identifier.to_string()),
            ],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.first().map(map_row).transpose()
}

/// Find a device by its refresh-grant `(user, identifier)` from a refresh token.
pub async fn find_by_user_identifier(
    db: &dyn AsyncQueryStore,
    user_uuid: &str,
    identifier: &str,
) -> AppResult<Option<DeviceRow>> {
    find(db, user_uuid, identifier).await
}

/// Insert a new device.
pub async fn insert(db: &dyn AsyncQueryStore, row: &DeviceRow) -> AppResult<()> {
    db.execute_async(
        "INSERT INTO vault_devices \
         (uuid, user_uuid, identifier, name, atype, refresh_token, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            DataValue::Text(row.uuid.clone()),
            DataValue::Text(row.user_uuid.clone()),
            DataValue::Text(row.identifier.clone()),
            match &row.name {
                Some(n) => DataValue::Text(n.clone()),
                None => DataValue::Null,
            },
            DataValue::Integer(row.atype),
            DataValue::Text(row.refresh_token.clone()),
            DataValue::Text(row.created_at.to_rfc3339()),
            DataValue::Text(row.updated_at.to_rfc3339()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

/// Replace a device's stored refresh token (rotation).
pub async fn update_refresh_token(
    db: &dyn AsyncQueryStore,
    uuid: &str,
    refresh_token: &str,
    updated_at: DateTime<Utc>,
) -> AppResult<()> {
    db.execute_async(
        "UPDATE vault_devices SET refresh_token = ?, updated_at = ? WHERE uuid = ?",
        &[
            DataValue::Text(refresh_token.to_string()),
            DataValue::Text(updated_at.to_rfc3339()),
            DataValue::Text(uuid.to_string()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

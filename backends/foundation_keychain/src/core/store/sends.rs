//! Send persistence (spec-57, F008 Stage 1).
//!
//! Ported from OrangeVault `db/queries.rs` (sends). Holds the owner's send plus
//! the access controls (`password`, `max_access_count`/`access_count`,
//! `disabled`, `expiration_date`, `deletion_date`) enforced on anonymous access.

use chrono::{DateTime, Utc};

use foundation_db::core::storage_provider::{AsyncQueryStore, DataValue, SqlRow};

use crate::core::error::AppResult;
use crate::core::store::{parse_ts, store_err};

/// A `sends` table row.
#[derive(Debug, Clone)]
pub struct SendRow {
    pub uuid: String,
    pub user_uuid: String,
    pub atype: i64,
    pub name: String,
    pub notes: Option<String>,
    pub data: Option<String>,
    pub akey: Option<String>,
    pub password: Option<String>,
    pub max_access_count: Option<i64>,
    pub access_count: i64,
    pub disabled: bool,
    pub hide_email: bool,
    pub expiration_date: Option<DateTime<Utc>>,
    pub deletion_date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const COLUMNS: &str = "uuid, user_uuid, atype, name, notes, data, akey, password, \
     max_access_count, access_count, disabled, hide_email, expiration_date, deletion_date, \
     created_at, updated_at";

fn opt_text(o: &Option<String>) -> DataValue {
    match o {
        Some(s) => DataValue::Text(s.clone()),
        None => DataValue::Null,
    }
}

fn opt_int(o: Option<i64>) -> DataValue {
    match o {
        Some(i) => DataValue::Integer(i),
        None => DataValue::Null,
    }
}

fn opt_ts(o: Option<DateTime<Utc>>) -> DataValue {
    match o {
        Some(d) => DataValue::Text(d.to_rfc3339()),
        None => DataValue::Null,
    }
}

fn map_row(row: &SqlRow) -> AppResult<SendRow> {
    let disabled: i64 = row.get_by_name("disabled").map_err(store_err)?;
    let hide_email: i64 = row.get_by_name("hide_email").map_err(store_err)?;
    let expiration_date: Option<String> = row.get_by_name("expiration_date").map_err(store_err)?;
    let deletion_date: String = row.get_by_name("deletion_date").map_err(store_err)?;
    let created_at: String = row.get_by_name("created_at").map_err(store_err)?;
    let updated_at: String = row.get_by_name("updated_at").map_err(store_err)?;
    Ok(SendRow {
        uuid: row.get_by_name("uuid").map_err(store_err)?,
        user_uuid: row.get_by_name("user_uuid").map_err(store_err)?,
        atype: row.get_by_name("atype").map_err(store_err)?,
        name: row.get_by_name("name").map_err(store_err)?,
        notes: row.get_by_name("notes").map_err(store_err)?,
        data: row.get_by_name("data").map_err(store_err)?,
        akey: row.get_by_name("akey").map_err(store_err)?,
        password: row.get_by_name("password").map_err(store_err)?,
        max_access_count: row.get_by_name("max_access_count").map_err(store_err)?,
        access_count: row.get_by_name("access_count").map_err(store_err)?,
        disabled: disabled != 0,
        hide_email: hide_email != 0,
        expiration_date: expiration_date.as_deref().map(parse_ts).transpose()?,
        deletion_date: parse_ts(&deletion_date)?,
        created_at: parse_ts(&created_at)?,
        updated_at: parse_ts(&updated_at)?,
    })
}

/// Insert a new send.
pub async fn insert(db: &dyn AsyncQueryStore, row: &SendRow) -> AppResult<()> {
    db.execute_async(
        "INSERT INTO sends \
         (uuid, user_uuid, atype, name, notes, data, akey, password, max_access_count, \
          access_count, disabled, hide_email, expiration_date, deletion_date, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            DataValue::Text(row.uuid.clone()),
            DataValue::Text(row.user_uuid.clone()),
            DataValue::Integer(row.atype),
            DataValue::Text(row.name.clone()),
            opt_text(&row.notes),
            opt_text(&row.data),
            opt_text(&row.akey),
            opt_text(&row.password),
            opt_int(row.max_access_count),
            DataValue::Integer(row.access_count),
            DataValue::Integer(i64::from(row.disabled)),
            DataValue::Integer(i64::from(row.hide_email)),
            opt_ts(row.expiration_date),
            DataValue::Text(row.deletion_date.to_rfc3339()),
            DataValue::Text(row.created_at.to_rfc3339()),
            DataValue::Text(row.updated_at.to_rfc3339()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

/// All sends owned by a user.
pub async fn find_by_user(db: &dyn AsyncQueryStore, user_uuid: &str) -> AppResult<Vec<SendRow>> {
    let rows = db
        .query_async(
            &format!("SELECT {COLUMNS} FROM sends WHERE user_uuid = ? ORDER BY created_at"),
            &[DataValue::Text(user_uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.iter().map(map_row).collect()
}

/// A single send by uuid.
pub async fn find(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<Option<SendRow>> {
    let rows = db
        .query_async(
            &format!("SELECT {COLUMNS} FROM sends WHERE uuid = ?"),
            &[DataValue::Text(uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.first().map(map_row).transpose()
}

/// Update the mutable fields of a send.
pub async fn update(db: &dyn AsyncQueryStore, row: &SendRow) -> AppResult<()> {
    db.execute_async(
        "UPDATE sends SET name = ?, notes = ?, data = ?, akey = ?, password = ?, \
         max_access_count = ?, disabled = ?, hide_email = ?, expiration_date = ?, \
         deletion_date = ?, updated_at = ? WHERE uuid = ?",
        &[
            DataValue::Text(row.name.clone()),
            opt_text(&row.notes),
            opt_text(&row.data),
            opt_text(&row.akey),
            opt_text(&row.password),
            opt_int(row.max_access_count),
            DataValue::Integer(i64::from(row.disabled)),
            DataValue::Integer(i64::from(row.hide_email)),
            opt_ts(row.expiration_date),
            DataValue::Text(row.deletion_date.to_rfc3339()),
            DataValue::Text(row.updated_at.to_rfc3339()),
            DataValue::Text(row.uuid.clone()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

/// Bump `access_count` (on a successful anonymous access).
pub async fn increment_access(
    db: &dyn AsyncQueryStore,
    uuid: &str,
    new_count: i64,
) -> AppResult<()> {
    db.execute_async(
        "UPDATE sends SET access_count = ? WHERE uuid = ?",
        &[DataValue::Integer(new_count), DataValue::Text(uuid.to_string())],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

/// Delete a send.
pub async fn delete(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<()> {
    db.execute_async(
        "DELETE FROM sends WHERE uuid = ?",
        &[DataValue::Text(uuid.to_string())],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

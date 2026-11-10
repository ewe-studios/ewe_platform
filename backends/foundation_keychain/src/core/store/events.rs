//! Event (audit log) persistence (spec-57, F008 Stage 1).

use chrono::{DateTime, Utc};

use foundation_core::valtron::Ctx;
use foundation_db::core::storage_provider::{DataValue, QueryStore, SqlRow};

use crate::core::clock::wall_now_utc;
use crate::core::error::AppResult;
use crate::core::store::{parse_ts, store_err};

/// A `vault_events` row.
#[derive(Debug, Clone)]
pub struct EventRow {
    pub uuid: String,
    pub user_uuid: String,
    pub atype: i64,
    pub cipher_uuid: Option<String>,
    pub event_date: DateTime<Utc>,
}

const COLUMNS: &str = "uuid, user_uuid, atype, cipher_uuid, event_date, created_at";

fn map_row(row: &SqlRow) -> AppResult<EventRow> {
    let event_date: String = row.get_by_name("event_date").map_err(store_err)?;
    Ok(EventRow {
        uuid: row.get_by_name("uuid").map_err(store_err)?,
        user_uuid: row.get_by_name("user_uuid").map_err(store_err)?,
        atype: row.get_by_name("atype").map_err(store_err)?,
        cipher_uuid: row.get_by_name("cipher_uuid").map_err(store_err)?,
        event_date: parse_ts(&event_date)?,
    })
}

/// Append an event.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn insert(db: &dyn QueryStore, row: &EventRow, ctx: &Ctx) -> AppResult<()> {
    db.execute(
        "INSERT INTO vault_events (uuid, user_uuid, atype, cipher_uuid, event_date, created_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
        &[
            DataValue::Text(row.uuid.clone()),
            DataValue::Text(row.user_uuid.clone()),
            DataValue::Integer(row.atype),
            match &row.cipher_uuid {
                Some(c) => DataValue::Text(c.clone()),
                None => DataValue::Null,
            },
            DataValue::Text(row.event_date.to_rfc3339()),
            DataValue::Text(wall_now_utc(ctx).to_rfc3339()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

/// The user's events, newest first.
///
/// # Errors
///
/// Returns an error if the operation fails.
pub async fn find_by_user(db: &dyn QueryStore, user_uuid: &str) -> AppResult<Vec<EventRow>> {
    let rows = db
        .query(
            &format!(
                "SELECT {COLUMNS} FROM vault_events WHERE user_uuid = ? ORDER BY event_date DESC"
            ),
            &[DataValue::Text(user_uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.iter().map(map_row).collect()
}

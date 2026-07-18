//! Two-factor (TOTP) persistence (spec-57, F008 Stage 1).
//!
//! One row per user holding the base32 TOTP secret, whether 2FA is enabled, and a
//! recovery code. The TOTP algorithm itself is `foundation_auth::TOTPSecret`
//! (decision 01) — this layer only stores the secret.

use chrono::{DateTime, Utc};

use foundation_db::core::storage_provider::{AsyncQueryStore, DataValue, SqlRow};

use crate::core::error::AppResult;
use crate::core::store::{parse_ts, store_err};

/// A `vault_two_factor` row.
#[derive(Debug, Clone)]
pub struct TwoFactorRow {
    pub user_uuid: String,
    pub secret: String,
    pub enabled: bool,
    pub recovery_code: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const COLUMNS: &str = "user_uuid, secret, enabled, recovery_code, created_at, updated_at";

fn map_row(row: &SqlRow) -> AppResult<TwoFactorRow> {
    let enabled: i64 = row.get_by_name("enabled").map_err(store_err)?;
    let created_at: String = row.get_by_name("created_at").map_err(store_err)?;
    let updated_at: String = row.get_by_name("updated_at").map_err(store_err)?;
    Ok(TwoFactorRow {
        user_uuid: row.get_by_name("user_uuid").map_err(store_err)?,
        secret: row.get_by_name("secret").map_err(store_err)?,
        enabled: enabled != 0,
        recovery_code: row.get_by_name("recovery_code").map_err(store_err)?,
        created_at: parse_ts(&created_at)?,
        updated_at: parse_ts(&updated_at)?,
    })
}

/// The user's 2FA row, if any.
pub async fn get(db: &dyn AsyncQueryStore, user_uuid: &str) -> AppResult<Option<TwoFactorRow>> {
    let rows = db
        .query_async(
            &format!("SELECT {COLUMNS} FROM vault_two_factor WHERE user_uuid = ?"),
            &[DataValue::Text(user_uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.first().map(map_row).transpose()
}

/// Insert or replace the user's 2FA row.
pub async fn upsert(db: &dyn AsyncQueryStore, row: &TwoFactorRow) -> AppResult<()> {
    db.execute_async(
        "INSERT OR REPLACE INTO vault_two_factor \
         (user_uuid, secret, enabled, recovery_code, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
        &[
            DataValue::Text(row.user_uuid.clone()),
            DataValue::Text(row.secret.clone()),
            DataValue::Integer(i64::from(row.enabled)),
            match &row.recovery_code {
                Some(c) => DataValue::Text(c.clone()),
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

/// Remove the user's 2FA (disable).
pub async fn delete(db: &dyn AsyncQueryStore, user_uuid: &str) -> AppResult<()> {
    db.execute_async(
        "DELETE FROM vault_two_factor WHERE user_uuid = ?",
        &[DataValue::Text(user_uuid.to_string())],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

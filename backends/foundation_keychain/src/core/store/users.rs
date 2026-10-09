//! User persistence (spec-57, F008 Stage 1).
//!
//! Ported from OrangeVault `db/queries.rs` (users) onto `foundation_db`'s
//! `AsyncQueryStore`. The row carries the server-side password verifier
//! (`password_hash` + `salt` + `password_iterations`), the client KDF params the
//! prelogin endpoint echoes back, and the `security_stamp` that invalidates
//! tokens on password change.

use chrono::{DateTime, Utc};

use foundation_db::core::storage_provider::{AsyncQueryStore, DataValue, SqlRow};

use crate::core::error::AppResult;
use crate::core::store::{parse_ts, store_err};

/// A `users` table row.
#[derive(Debug, Clone)]
pub struct UserRow {
    pub uuid: String,
    pub email: String,
    pub name: Option<String>,
    pub password_hash: String,
    pub salt: String,
    pub password_iterations: i64,
    pub security_stamp: String,
    pub akey: Option<String>,
    pub client_kdf_type: i64,
    pub client_kdf_iter: i64,
    pub client_kdf_memory: Option<i64>,
    pub client_kdf_parallelism: Option<i64>,
    pub master_password_hint: Option<String>,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const COLUMNS: &str = "uuid, email, name, password_hash, salt, password_iterations, \
     security_stamp, akey, client_kdf_type, client_kdf_iter, client_kdf_memory, \
     client_kdf_parallelism, master_password_hint, email_verified, created_at, updated_at";

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

fn map_row(row: &SqlRow) -> AppResult<UserRow> {
    let email_verified: i64 = row.get_by_name("email_verified").map_err(store_err)?;
    let created_at: String = row.get_by_name("created_at").map_err(store_err)?;
    let updated_at: String = row.get_by_name("updated_at").map_err(store_err)?;
    Ok(UserRow {
        uuid: row.get_by_name("uuid").map_err(store_err)?,
        email: row.get_by_name("email").map_err(store_err)?,
        name: row.get_by_name("name").map_err(store_err)?,
        password_hash: row.get_by_name("password_hash").map_err(store_err)?,
        salt: row.get_by_name("salt").map_err(store_err)?,
        password_iterations: row.get_by_name("password_iterations").map_err(store_err)?,
        security_stamp: row.get_by_name("security_stamp").map_err(store_err)?,
        akey: row.get_by_name("akey").map_err(store_err)?,
        client_kdf_type: row.get_by_name("client_kdf_type").map_err(store_err)?,
        client_kdf_iter: row.get_by_name("client_kdf_iter").map_err(store_err)?,
        client_kdf_memory: row.get_by_name("client_kdf_memory").map_err(store_err)?,
        client_kdf_parallelism: row.get_by_name("client_kdf_parallelism").map_err(store_err)?,
        master_password_hint: row.get_by_name("master_password_hint").map_err(store_err)?,
        email_verified: email_verified != 0,
        created_at: parse_ts(&created_at)?,
        updated_at: parse_ts(&updated_at)?,
    })
}

/// Insert a new user.
pub async fn insert(db: &dyn AsyncQueryStore, row: &UserRow) -> AppResult<()> {
    db.execute_async(
        "INSERT INTO vault_accounts \
         (uuid, email, name, password_hash, salt, password_iterations, security_stamp, akey, \
          client_kdf_type, client_kdf_iter, client_kdf_memory, client_kdf_parallelism, \
          master_password_hint, email_verified, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            DataValue::Text(row.uuid.clone()),
            DataValue::Text(row.email.clone()),
            opt_text(&row.name),
            DataValue::Text(row.password_hash.clone()),
            DataValue::Text(row.salt.clone()),
            DataValue::Integer(row.password_iterations),
            DataValue::Text(row.security_stamp.clone()),
            opt_text(&row.akey),
            DataValue::Integer(row.client_kdf_type),
            DataValue::Integer(row.client_kdf_iter),
            opt_int(row.client_kdf_memory),
            opt_int(row.client_kdf_parallelism),
            opt_text(&row.master_password_hint),
            DataValue::Integer(i64::from(row.email_verified)),
            DataValue::Text(row.created_at.to_rfc3339()),
            DataValue::Text(row.updated_at.to_rfc3339()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

async fn find_one(
    db: &dyn AsyncQueryStore,
    where_col: &str,
    value: &str,
) -> AppResult<Option<UserRow>> {
    let rows = db
        .query_async(
            &format!("SELECT {COLUMNS} FROM vault_accounts WHERE {where_col} = ?"),
            &[DataValue::Text(value.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.first().map(map_row).transpose()
}

/// Find a user by (already-normalized) email.
pub async fn find_by_email(db: &dyn AsyncQueryStore, email: &str) -> AppResult<Option<UserRow>> {
    find_one(db, "email", email).await
}

/// Find a user by uuid.
pub async fn find_by_uuid(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<Option<UserRow>> {
    find_one(db, "uuid", uuid).await
}

/// Update the mutable profile fields (name, master password hint).
pub async fn update_profile(
    db: &dyn AsyncQueryStore,
    uuid: &str,
    name: &Option<String>,
    hint: &Option<String>,
    updated_at: DateTime<Utc>,
) -> AppResult<()> {
    db.execute_async(
        "UPDATE vault_accounts SET name = ?, master_password_hint = ?, updated_at = ? WHERE uuid = ?",
        &[
            opt_text(name),
            opt_text(hint),
            DataValue::Text(updated_at.to_rfc3339()),
            DataValue::Text(uuid.to_string()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

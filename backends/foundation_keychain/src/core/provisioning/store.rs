//! App + SSH-key persistence (feature 009).

use chrono::{DateTime, Utc};

use foundation_db::core::storage_provider::{AsyncQueryStore, DataValue, SqlRow};

use crate::core::error::AppResult;
use crate::core::store::{parse_ts, store_err};

// ── Apps ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AppRow {
    pub uuid: String,
    pub name: String,
    pub description: Option<String>,
    pub secret_hash: String,
    pub secret_salt: String,
    pub created_at: DateTime<Utc>,
}

fn opt_text(o: &Option<String>) -> DataValue {
    match o {
        Some(s) => DataValue::Text(s.clone()),
        None => DataValue::Null,
    }
}

fn map_app(row: &SqlRow) -> AppResult<AppRow> {
    let created_at: String = row.get_by_name("created_at").map_err(store_err)?;
    Ok(AppRow {
        uuid: row.get_by_name("uuid").map_err(store_err)?,
        name: row.get_by_name("name").map_err(store_err)?,
        description: row.get_by_name("description").map_err(store_err)?,
        secret_hash: row.get_by_name("secret_hash").map_err(store_err)?,
        secret_salt: row.get_by_name("secret_salt").map_err(store_err)?,
        created_at: parse_ts(&created_at)?,
    })
}

pub async fn insert_app(db: &dyn AsyncQueryStore, row: &AppRow) -> AppResult<()> {
    db.execute_async(
        "INSERT INTO vault_apps (uuid, name, description, secret_hash, secret_salt, created_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
        &[
            DataValue::Text(row.uuid.clone()),
            DataValue::Text(row.name.clone()),
            opt_text(&row.description),
            DataValue::Text(row.secret_hash.clone()),
            DataValue::Text(row.secret_salt.clone()),
            DataValue::Text(row.created_at.to_rfc3339()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

pub async fn find_app(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<Option<AppRow>> {
    let rows = db
        .query_async(
            "SELECT uuid, name, description, secret_hash, secret_salt, created_at \
             FROM vault_apps WHERE uuid = ?",
            &[DataValue::Text(uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.first().map(map_app).transpose()
}

pub async fn delete_app(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<()> {
    for sql in [
        "DELETE FROM vault_ssh_keys WHERE app_uuid = ?",
        "DELETE FROM vault_apps WHERE uuid = ?",
    ] {
        db.execute_async(sql, &[DataValue::Text(uuid.to_string())])
            .await
            .map_err(store_err)?;
    }
    Ok(())
}

// ── SSH keys ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SshKeyRow {
    pub uuid: String,
    pub app_uuid: String,
    pub name: String,
    pub key_type: String,
    pub public_key: String,
    pub private_key_encrypted: String,
    pub comment: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

const KEY_COLS: &str = "uuid, app_uuid, name, key_type, public_key, private_key_encrypted, \
     comment, expires_at, created_at";

fn map_key(row: &SqlRow) -> AppResult<SshKeyRow> {
    let expires_at: Option<String> = row.get_by_name("expires_at").map_err(store_err)?;
    let created_at: String = row.get_by_name("created_at").map_err(store_err)?;
    Ok(SshKeyRow {
        uuid: row.get_by_name("uuid").map_err(store_err)?,
        app_uuid: row.get_by_name("app_uuid").map_err(store_err)?,
        name: row.get_by_name("name").map_err(store_err)?,
        key_type: row.get_by_name("key_type").map_err(store_err)?,
        public_key: row.get_by_name("public_key").map_err(store_err)?,
        private_key_encrypted: row.get_by_name("private_key_encrypted").map_err(store_err)?,
        comment: row.get_by_name("comment").map_err(store_err)?,
        expires_at: expires_at.as_deref().map(parse_ts).transpose()?,
        created_at: parse_ts(&created_at)?,
    })
}

pub async fn insert_key(db: &dyn AsyncQueryStore, row: &SshKeyRow) -> AppResult<()> {
    db.execute_async(
        "INSERT INTO vault_ssh_keys \
         (uuid, app_uuid, name, key_type, public_key, private_key_encrypted, comment, expires_at, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            DataValue::Text(row.uuid.clone()),
            DataValue::Text(row.app_uuid.clone()),
            DataValue::Text(row.name.clone()),
            DataValue::Text(row.key_type.clone()),
            DataValue::Text(row.public_key.clone()),
            DataValue::Text(row.private_key_encrypted.clone()),
            opt_text(&row.comment),
            match &row.expires_at {
                Some(d) => DataValue::Text(d.to_rfc3339()),
                None => DataValue::Null,
            },
            DataValue::Text(row.created_at.to_rfc3339()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

pub async fn find_keys_by_app(db: &dyn AsyncQueryStore, app_uuid: &str) -> AppResult<Vec<SshKeyRow>> {
    let rows = db
        .query_async(
            &format!("SELECT {KEY_COLS} FROM vault_ssh_keys WHERE app_uuid = ? ORDER BY created_at"),
            &[DataValue::Text(app_uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.iter().map(map_key).collect()
}

pub async fn find_key(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<Option<SshKeyRow>> {
    let rows = db
        .query_async(
            &format!("SELECT {KEY_COLS} FROM vault_ssh_keys WHERE uuid = ?"),
            &[DataValue::Text(uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.first().map(map_key).transpose()
}

pub async fn delete_key(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<()> {
    db.execute_async(
        "DELETE FROM vault_ssh_keys WHERE uuid = ?",
        &[DataValue::Text(uuid.to_string())],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

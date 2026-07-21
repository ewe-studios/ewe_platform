//! Organization persistence — orgs, memberships, collections (spec-57, F008 Stage 1).
//!
//! Ported from OrangeVault `db/queries.rs`. Membership `status`: 0 = invited,
//! 1 = accepted, 2 = confirmed.

use chrono::{DateTime, Utc};

use foundation_db::core::storage_provider::{AsyncQueryStore, DataValue, SqlRow};

use crate::core::error::AppResult;
use crate::core::store::{parse_ts, store_err};

fn opt_text(o: &Option<String>) -> DataValue {
    match o {
        Some(s) => DataValue::Text(s.clone()),
        None => DataValue::Null,
    }
}

// ── Organizations ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct OrgRow {
    pub uuid: String,
    pub name: String,
    pub billing_email: Option<String>,
    pub enabled: bool,
    pub akey: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn map_org(row: &SqlRow) -> AppResult<OrgRow> {
    let enabled: i64 = row.get_by_name("enabled").map_err(store_err)?;
    let created_at: String = row.get_by_name("created_at").map_err(store_err)?;
    let updated_at: String = row.get_by_name("updated_at").map_err(store_err)?;
    Ok(OrgRow {
        uuid: row.get_by_name("uuid").map_err(store_err)?,
        name: row.get_by_name("name").map_err(store_err)?,
        billing_email: row.get_by_name("billing_email").map_err(store_err)?,
        enabled: enabled != 0,
        akey: row.get_by_name("akey").map_err(store_err)?,
        created_at: parse_ts(&created_at)?,
        updated_at: parse_ts(&updated_at)?,
    })
}

pub async fn insert_org(db: &dyn AsyncQueryStore, row: &OrgRow) -> AppResult<()> {
    db.execute_async(
        "INSERT INTO vault_organizations (uuid, name, billing_email, enabled, akey, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        &[
            DataValue::Text(row.uuid.clone()),
            DataValue::Text(row.name.clone()),
            opt_text(&row.billing_email),
            DataValue::Integer(i64::from(row.enabled)),
            opt_text(&row.akey),
            DataValue::Text(row.created_at.to_rfc3339()),
            DataValue::Text(row.updated_at.to_rfc3339()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

pub async fn find_org(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<Option<OrgRow>> {
    let rows = db
        .query_async(
            "SELECT uuid, name, billing_email, enabled, akey, created_at, updated_at \
             FROM vault_organizations WHERE uuid = ?",
            &[DataValue::Text(uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.first().map(map_org).transpose()
}

pub async fn delete_org(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<()> {
    for sql in [
        "DELETE FROM vault_collections WHERE org_uuid = ?",
        "DELETE FROM vault_org_memberships WHERE org_uuid = ?",
        "DELETE FROM vault_organizations WHERE uuid = ?",
    ] {
        db.execute_async(sql, &[DataValue::Text(uuid.to_string())])
            .await
            .map_err(store_err)?;
    }
    Ok(())
}

// ── Memberships ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MembershipRow {
    pub uuid: String,
    pub org_uuid: String,
    pub user_uuid: Option<String>,
    pub email: String,
    pub atype: i64,
    pub status: i64,
    pub akey: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const M_COLS: &str = "uuid, org_uuid, user_uuid, email, atype, status, akey, created_at, updated_at";

fn map_membership(row: &SqlRow) -> AppResult<MembershipRow> {
    let created_at: String = row.get_by_name("created_at").map_err(store_err)?;
    let updated_at: String = row.get_by_name("updated_at").map_err(store_err)?;
    Ok(MembershipRow {
        uuid: row.get_by_name("uuid").map_err(store_err)?,
        org_uuid: row.get_by_name("org_uuid").map_err(store_err)?,
        user_uuid: row.get_by_name("user_uuid").map_err(store_err)?,
        email: row.get_by_name("email").map_err(store_err)?,
        atype: row.get_by_name("atype").map_err(store_err)?,
        status: row.get_by_name("status").map_err(store_err)?,
        akey: row.get_by_name("akey").map_err(store_err)?,
        created_at: parse_ts(&created_at)?,
        updated_at: parse_ts(&updated_at)?,
    })
}

pub async fn insert_membership(db: &dyn AsyncQueryStore, row: &MembershipRow) -> AppResult<()> {
    db.execute_async(
        "INSERT INTO vault_org_memberships \
         (uuid, org_uuid, user_uuid, email, atype, status, akey, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            DataValue::Text(row.uuid.clone()),
            DataValue::Text(row.org_uuid.clone()),
            opt_text(&row.user_uuid),
            DataValue::Text(row.email.clone()),
            DataValue::Integer(row.atype),
            DataValue::Integer(row.status),
            opt_text(&row.akey),
            DataValue::Text(row.created_at.to_rfc3339()),
            DataValue::Text(row.updated_at.to_rfc3339()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

pub async fn find_membership(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<Option<MembershipRow>> {
    query_memberships(db, "WHERE uuid = ?", uuid).await.map(|mut v| v.pop())
}

pub async fn find_membership_for_user(
    db: &dyn AsyncQueryStore,
    org_uuid: &str,
    user_uuid: &str,
) -> AppResult<Option<MembershipRow>> {
    let rows = db
        .query_async(
            &format!("SELECT {M_COLS} FROM vault_org_memberships WHERE org_uuid = ? AND user_uuid = ?"),
            &[
                DataValue::Text(org_uuid.to_string()),
                DataValue::Text(user_uuid.to_string()),
            ],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.first().map(map_membership).transpose()
}

pub async fn find_memberships_by_user(
    db: &dyn AsyncQueryStore,
    user_uuid: &str,
) -> AppResult<Vec<MembershipRow>> {
    query_memberships(db, "WHERE user_uuid = ?", user_uuid).await
}

pub async fn find_memberships_by_org(
    db: &dyn AsyncQueryStore,
    org_uuid: &str,
) -> AppResult<Vec<MembershipRow>> {
    query_memberships(db, "WHERE org_uuid = ?", org_uuid).await
}

async fn query_memberships(
    db: &dyn AsyncQueryStore,
    where_clause: &str,
    value: &str,
) -> AppResult<Vec<MembershipRow>> {
    let rows = db
        .query_async(
            &format!("SELECT {M_COLS} FROM vault_org_memberships {where_clause}"),
            &[DataValue::Text(value.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.iter().map(map_membership).collect()
}

pub async fn update_membership(db: &dyn AsyncQueryStore, row: &MembershipRow) -> AppResult<()> {
    db.execute_async(
        "UPDATE vault_org_memberships SET user_uuid = ?, atype = ?, status = ?, akey = ?, updated_at = ? \
         WHERE uuid = ?",
        &[
            opt_text(&row.user_uuid),
            DataValue::Integer(row.atype),
            DataValue::Integer(row.status),
            opt_text(&row.akey),
            DataValue::Text(row.updated_at.to_rfc3339()),
            DataValue::Text(row.uuid.clone()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

pub async fn delete_membership(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<()> {
    db.execute_async(
        "DELETE FROM vault_org_memberships WHERE uuid = ?",
        &[DataValue::Text(uuid.to_string())],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

// ── Collections ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CollectionRow {
    pub uuid: String,
    pub org_uuid: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn map_collection(row: &SqlRow) -> AppResult<CollectionRow> {
    let created_at: String = row.get_by_name("created_at").map_err(store_err)?;
    let updated_at: String = row.get_by_name("updated_at").map_err(store_err)?;
    Ok(CollectionRow {
        uuid: row.get_by_name("uuid").map_err(store_err)?,
        org_uuid: row.get_by_name("org_uuid").map_err(store_err)?,
        name: row.get_by_name("name").map_err(store_err)?,
        created_at: parse_ts(&created_at)?,
        updated_at: parse_ts(&updated_at)?,
    })
}

pub async fn insert_collection(db: &dyn AsyncQueryStore, row: &CollectionRow) -> AppResult<()> {
    db.execute_async(
        "INSERT INTO vault_collections (uuid, org_uuid, name, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?)",
        &[
            DataValue::Text(row.uuid.clone()),
            DataValue::Text(row.org_uuid.clone()),
            DataValue::Text(row.name.clone()),
            DataValue::Text(row.created_at.to_rfc3339()),
            DataValue::Text(row.updated_at.to_rfc3339()),
        ],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

pub async fn find_collections_by_org(
    db: &dyn AsyncQueryStore,
    org_uuid: &str,
) -> AppResult<Vec<CollectionRow>> {
    let rows = db
        .query_async(
            "SELECT uuid, org_uuid, name, created_at, updated_at FROM vault_collections \
             WHERE org_uuid = ? ORDER BY name",
            &[DataValue::Text(org_uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.iter().map(map_collection).collect()
}

pub async fn find_collection(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<Option<CollectionRow>> {
    let rows = db
        .query_async(
            "SELECT uuid, org_uuid, name, created_at, updated_at FROM vault_collections WHERE uuid = ?",
            &[DataValue::Text(uuid.to_string())],
        )
        .await
        .map_err(store_err)?
        .collect_all()
        .await
        .map_err(store_err)?;
    rows.first().map(map_collection).transpose()
}

pub async fn delete_collection(db: &dyn AsyncQueryStore, uuid: &str) -> AppResult<()> {
    db.execute_async(
        "DELETE FROM vault_collections WHERE uuid = ?",
        &[DataValue::Text(uuid.to_string())],
    )
    .await
    .map_err(store_err)?;
    Ok(())
}

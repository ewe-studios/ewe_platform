//! Organizations API — portable handlers (spec-57, F008 Stage 1, core subset).
//!
//! WHY: shared vaults — orgs own collections and have members. Ported from
//! OrangeVault `api/organizations.rs`, decoupled from any transport.
//!
//! WHAT (core): `create` (org + owner membership + default collection),
//! `get`/`list_for_user`/`delete`, collection `create_collection`/
//! `list_collections`/`delete_collection`, and membership `invite_member`/
//! `confirm_member`/`list_members`. Advanced org features (collection-user ACLs,
//! policies, groups, cipher-share) are deferred within Stage 1.
//!
//! HOW: permissions — any confirmed member may read; only Owner/Admin may manage.
//! Membership `status`: 0 invited, 1 accepted, 2 confirmed.

use chrono::Utc;
use uuid::Uuid;

use crate::core::context::KeychainContext;
use crate::core::error::{AppError, AppResult};
use crate::core::models::org::{
    Collection, CollectionCreateRequest, ConfirmMemberRequest, InviteMemberRequest, MemberResponse,
    Organization, OrganizationCreateRequest, OrganizationUserType, ProfileOrganization,
};
use crate::core::store::orgs as store;
use crate::core::store::orgs::{CollectionRow, MembershipRow, OrgRow};

const STATUS_INVITED: i64 = 0;
const STATUS_CONFIRMED: i64 = 2;

fn member_type_from_i64(v: i64) -> OrganizationUserType {
    match v {
        0 => OrganizationUserType::Owner,
        1 => OrganizationUserType::Admin,
        3 => OrganizationUserType::Manager,
        4 => OrganizationUserType::Custom,
        _ => OrganizationUserType::User,
    }
}

fn to_org(row: &OrgRow) -> Organization {
    Organization {
        id: row.uuid.clone(),
        name: row.name.clone(),
        billing_email: row.billing_email.clone(),
        enabled: row.enabled,
        max_collections: None,
        max_storage_gb: None,
        creation_date: row.created_at,
        revision_date: row.updated_at,
    }
}

fn to_collection(row: &CollectionRow) -> Collection {
    Collection {
        id: row.uuid.clone(),
        organization_id: row.org_uuid.clone(),
        name: row.name.clone(),
        external_id: None,
        read_only: false,
        object: "collectionDetails",
    }
}

fn to_member(row: &MembershipRow) -> MemberResponse {
    MemberResponse {
        id: row.uuid.clone(),
        user_id: row.user_uuid.clone(),
        organization_id: row.org_uuid.clone(),
        email: row.email.clone(),
        member_type: member_type_from_i64(row.atype),
        status: row.status as i32,
        object: "organizationUserUserDetails",
    }
}

/// The caller's membership in `org_id`, or an error if they are not a member.
async fn membership(
    ctx: &KeychainContext,
    user_uuid: &str,
    org_id: &str,
) -> AppResult<MembershipRow> {
    store::find_membership_for_user(ctx.db(), org_id, user_uuid)
        .await?
        .ok_or(AppError::Forbidden)
}

/// Require the caller to be an Owner or Admin of `org_id`.
async fn require_admin(
    ctx: &KeychainContext,
    user_uuid: &str,
    org_id: &str,
) -> AppResult<MembershipRow> {
    let m = membership(ctx, user_uuid, org_id).await?;
    let is_admin = matches!(
        member_type_from_i64(m.atype),
        OrganizationUserType::Owner | OrganizationUserType::Admin
    );
    if !is_admin || m.status != STATUS_CONFIRMED {
        return Err(AppError::Forbidden);
    }
    Ok(m)
}

/// `POST /api/organizations` — create an org with the caller as owner + a default collection.
pub async fn create(
    ctx: &KeychainContext,
    user_uuid: &str,
    email: &str,
    req: OrganizationCreateRequest,
) -> AppResult<Organization> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("organization name is required".into()));
    }
    let now = Utc::now();
    let org = OrgRow {
        uuid: Uuid::new_v4().to_string(),
        name: req.name,
        billing_email: req.billing_email.or_else(|| Some(email.to_string())),
        enabled: true,
        akey: req.key,
        created_at: now,
        updated_at: now,
    };
    store::insert_org(ctx.db(), &org).await?;

    store::insert_membership(
        ctx.db(),
        &MembershipRow {
            uuid: Uuid::new_v4().to_string(),
            org_uuid: org.uuid.clone(),
            user_uuid: Some(user_uuid.to_string()),
            email: email.to_string(),
            atype: OrganizationUserType::Owner as i64,
            status: STATUS_CONFIRMED,
            akey: None,
            created_at: now,
            updated_at: now,
        },
    )
    .await?;

    store::insert_collection(
        ctx.db(),
        &CollectionRow {
            uuid: Uuid::new_v4().to_string(),
            org_uuid: org.uuid.clone(),
            name: req.collection_name.unwrap_or_else(|| "Default Collection".into()),
            created_at: now,
            updated_at: now,
        },
    )
    .await?;

    Ok(to_org(&org))
}

/// `GET /api/organizations/:id` — any member may read.
pub async fn get(ctx: &KeychainContext, user_uuid: &str, org_id: &str) -> AppResult<Organization> {
    membership(ctx, user_uuid, org_id).await?;
    let org = store::find_org(ctx.db(), org_id)
        .await?
        .ok_or_else(|| AppError::NotFound("organization not found".into()))?;
    Ok(to_org(&org))
}

/// The caller's organization memberships (the `profile.organizations` block).
pub async fn list_for_user(
    ctx: &KeychainContext,
    user_uuid: &str,
) -> AppResult<Vec<ProfileOrganization>> {
    let mut out = Vec::new();
    for m in store::find_memberships_by_user(ctx.db(), user_uuid).await? {
        if let Some(org) = store::find_org(ctx.db(), &m.org_uuid).await? {
            out.push(ProfileOrganization {
                id: org.uuid,
                name: org.name,
                member_type: member_type_from_i64(m.atype),
                status: m.status as i32,
                enabled: org.enabled,
                key: m.akey,
                object: "profileOrganization",
            });
        }
    }
    Ok(out)
}

/// `DELETE /api/organizations/:id` — owner only.
pub async fn delete(ctx: &KeychainContext, user_uuid: &str, org_id: &str) -> AppResult<()> {
    let m = membership(ctx, user_uuid, org_id).await?;
    if member_type_from_i64(m.atype) != OrganizationUserType::Owner {
        return Err(AppError::Forbidden);
    }
    store::delete_org(ctx.db(), org_id).await
}

/// `POST /api/organizations/:id/collections` — owner/admin.
pub async fn create_collection(
    ctx: &KeychainContext,
    user_uuid: &str,
    org_id: &str,
    req: CollectionCreateRequest,
) -> AppResult<Collection> {
    require_admin(ctx, user_uuid, org_id).await?;
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("collection name is required".into()));
    }
    let now = Utc::now();
    let row = CollectionRow {
        uuid: Uuid::new_v4().to_string(),
        org_uuid: org_id.to_string(),
        name: req.name,
        created_at: now,
        updated_at: now,
    };
    store::insert_collection(ctx.db(), &row).await?;
    Ok(to_collection(&row))
}

/// `GET /api/organizations/:id/collections` — any member.
pub async fn list_collections(
    ctx: &KeychainContext,
    user_uuid: &str,
    org_id: &str,
) -> AppResult<Vec<Collection>> {
    membership(ctx, user_uuid, org_id).await?;
    Ok(store::find_collections_by_org(ctx.db(), org_id)
        .await?
        .iter()
        .map(to_collection)
        .collect())
}

/// `DELETE /api/organizations/:id/collections/:col_id` — owner/admin.
pub async fn delete_collection(
    ctx: &KeychainContext,
    user_uuid: &str,
    org_id: &str,
    col_id: &str,
) -> AppResult<()> {
    require_admin(ctx, user_uuid, org_id).await?;
    let col = store::find_collection(ctx.db(), col_id)
        .await?
        .ok_or_else(|| AppError::NotFound("collection not found".into()))?;
    if col.org_uuid != org_id {
        return Err(AppError::NotFound("collection not found".into()));
    }
    store::delete_collection(ctx.db(), col_id).await
}

/// `POST /api/organizations/:id/users/invite` — owner/admin invites by email.
pub async fn invite_member(
    ctx: &KeychainContext,
    user_uuid: &str,
    org_id: &str,
    req: InviteMemberRequest,
) -> AppResult<MemberResponse> {
    require_admin(ctx, user_uuid, org_id).await?;
    let email = req.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::BadRequest("a valid email is required".into()));
    }
    let now = Utc::now();
    let row = MembershipRow {
        uuid: Uuid::new_v4().to_string(),
        org_uuid: org_id.to_string(),
        user_uuid: None,
        email,
        atype: req.member_type as i64,
        status: STATUS_INVITED,
        akey: None,
        created_at: now,
        updated_at: now,
    };
    store::insert_membership(ctx.db(), &row).await?;
    Ok(to_member(&row))
}

/// `POST /api/organizations/:id/users/:member_id/confirm` — owner/admin confirms.
pub async fn confirm_member(
    ctx: &KeychainContext,
    user_uuid: &str,
    org_id: &str,
    member_id: &str,
    req: ConfirmMemberRequest,
) -> AppResult<()> {
    require_admin(ctx, user_uuid, org_id).await?;
    let mut m = store::find_membership(ctx.db(), member_id)
        .await?
        .ok_or_else(|| AppError::NotFound("member not found".into()))?;
    if m.org_uuid != org_id {
        return Err(AppError::NotFound("member not found".into()));
    }
    m.status = STATUS_CONFIRMED;
    if req.key.is_some() {
        m.akey = req.key;
    }
    m.updated_at = Utc::now();
    store::update_membership(ctx.db(), &m).await
}

/// `GET /api/organizations/:id/users` — owner/admin lists members.
pub async fn list_members(
    ctx: &KeychainContext,
    user_uuid: &str,
    org_id: &str,
) -> AppResult<Vec<MemberResponse>> {
    require_admin(ctx, user_uuid, org_id).await?;
    Ok(store::find_memberships_by_org(ctx.db(), org_id)
        .await?
        .iter()
        .map(to_member)
        .collect())
}

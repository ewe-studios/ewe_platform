//! Organization models (spec-57, F008).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OrganizationUserType {
    Owner = 0,
    Admin = 1,
    User = 2,
    Manager = 3,
    Custom = 4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub billing_email: Option<String>,
    pub enabled: bool,
    pub max_collections: Option<i32>,
    pub max_storage_gb: Option<i32>,
    pub creation_date: DateTime<Utc>,
    pub revision_date: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationCreateRequest {
    pub name: String,
    pub billing_email: Option<String>,
    pub key: Option<String>,
    pub collection_name: Option<String>,
}

// ── Collections + membership DTOs (F008 Stage 1) ────────────────────────────

/// A collection within an organization.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub organization_id: String,
    pub name: String,
    pub external_id: Option<String>,
    pub read_only: bool,
    pub object: &'static str,
}

/// An organization membership as returned to the owner/admin.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberResponse {
    pub id: String,
    pub user_id: Option<String>,
    pub organization_id: String,
    pub email: String,
    #[serde(rename = "type")]
    pub member_type: OrganizationUserType,
    pub status: i32,
    pub object: &'static str,
}

/// The `profile.organizations[*]` entry a client sees for its own memberships.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileOrganization {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub member_type: OrganizationUserType,
    pub status: i32,
    pub enabled: bool,
    pub key: Option<String>,
    pub object: &'static str,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionCreateRequest {
    pub name: String,
    pub external_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteMemberRequest {
    pub email: String,
    #[serde(rename = "type")]
    pub member_type: OrganizationUserType,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmMemberRequest {
    pub key: Option<String>,
}

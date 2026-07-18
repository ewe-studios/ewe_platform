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

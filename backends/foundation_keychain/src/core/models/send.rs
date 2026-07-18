//! Send models — time-limited, access-counted secure sharing (spec-57, F008).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SendType {
    Text = 0,
    File = 1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Send {
    pub id: String,
    pub name: String,
    pub notes: Option<String>,
    #[serde(rename = "type")]
    pub send_type: SendType,
    pub data: Option<String>,
    pub key: Option<String>,
    pub password: Option<String>,
    pub max_access_count: Option<i32>,
    pub access_count: i32,
    pub disabled: bool,
    pub hide_email: bool,
    pub deletion_date: DateTime<Utc>,
    pub expiration_date: Option<DateTime<Utc>>,
    pub revision_date: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendCreateRequest {
    pub name: String,
    pub notes: Option<String>,
    #[serde(rename = "type")]
    pub send_type: SendType,
    pub data: Option<String>,
    pub key: Option<String>,
    pub password: Option<String>,
    pub max_access_count: Option<i32>,
    pub deletion_date: Option<String>,
    pub expiration_date: Option<String>,
    pub disabled: Option<bool>,
    pub hide_email: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendUpdateRequest {
    pub name: Option<String>,
    pub notes: Option<String>,
    pub data: Option<String>,
    pub key: Option<String>,
    pub password: Option<String>,
    pub max_access_count: Option<i32>,
    pub disabled: Option<bool>,
    pub hide_email: Option<bool>,
}

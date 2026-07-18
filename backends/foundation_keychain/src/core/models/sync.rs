//! Sync data — the full vault snapshot returned by /sync (spec-57, F008).

use serde::Serialize;

use super::cipher::Cipher;
use super::folder::Folder;
use super::org::Organization;
use super::send::Send;
use super::user::UserAccount;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncData {
    pub profile: Option<UserAccount>,
    pub folders: Vec<Folder>,
    pub ciphers: Vec<Cipher>,
    pub collections: Vec<serde_json::Value>,
    pub sends: Vec<Send>,
    pub domains: Option<EquivalentDomains>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquivalentDomains {
    pub equivalent_domains: Option<Vec<Vec<String>>>,
    pub global_equivalent_domains: Option<Vec<GlobalEquivalentDomain>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalEquivalentDomain {
    #[serde(rename = "type")]
    pub domain_type: i32,
    pub domains: Vec<String>,
    pub excluded: bool,
}

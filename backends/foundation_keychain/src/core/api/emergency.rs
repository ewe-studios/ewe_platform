//! Emergency access API — placeholder (spec-57, F008 Stage 1).
//!
//! WHY: Bitwarden's emergency-access feature (trusted contacts who can request
//! vault access) is not implemented upstream in OrangeVault either — the endpoint
//! exists only so clients that probe it don't error. `list` returns an empty set;
//! grant/request/approve are intentionally deferred (not part of Stage 1 scope).

use serde::Serialize;

use crate::core::context::KeychainContext;
use crate::core::error::AppResult;

/// A trusted/granted emergency-access relationship (none yet).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmergencyAccess {
    pub id: String,
    pub status: i32,
    pub object: &'static str,
}

/// `GET /api/emergency-access/{trusted,granted}` — currently always empty.
pub async fn list(_ctx: &KeychainContext, _user_uuid: &str) -> AppResult<Vec<EmergencyAccess>> {
    Ok(Vec::new())
}

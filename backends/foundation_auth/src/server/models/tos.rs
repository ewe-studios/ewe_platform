//! Terms of Service model — versioned ToS with acceptance tracking.

use serde::{Deserialize, Serialize};

/// A version of the Terms of Service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TosVersion {
    pub version: String,
    pub content: String,
    pub created_at: i64,
    pub effective_from: i64,
}

/// A user's acceptance of a ToS version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TosAcceptance {
    pub user_id: String,
    pub tos_version: String,
    pub accepted_at: i64,
    pub ip_address: Option<String>,
}

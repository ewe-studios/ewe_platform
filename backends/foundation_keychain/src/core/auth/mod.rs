//! Bitwarden-specific auth adapters over foundation_auth (spec-57, F008).
//!
//! WHY: foundation_auth provides general JWT/TOTP/PBKDF2; Bitwarden adds
//! specific claims (`sstamp` for security stamp, `orgowner`/`orgadmin` for
//! org roles) and a distinctive login response shape (`Key`, `PrivateKey`,
//! `Kdf`, `UserDecryptionOptions`).
//!
//! WHAT: `BitwardenClaims` maps onto `VerifiedClaims::custom`. The security
//! stamp middleware checks `sstamp` against the DB after JWT verify. The
//! token response adapter builds the full `LoginResponse`.

use serde::{Deserialize, Serialize};

/// Bitwarden-specific claims carried inside the JWT `custom` map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitwardenClaims {
    /// Security stamp — changes on password rotation, invalidates old tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sstamp: Option<String>,
    /// Whether the user is an org owner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orgowner: Option<String>,
    /// Whether the user is an org admin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orgadmin: Option<String>,
    /// Whether the user is an org user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orguser: Option<String>,
    /// Whether the user is an org manager.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orgmanager: Option<String>,
    /// Premium flag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub premium: Option<bool>,
    /// Device identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

/// Verify that the JWT's security stamp matches the user's current stamp.
///
/// Returns `true` if the token is still valid (stamps match or no stamp claim).
#[must_use]
pub fn verify_security_stamp(token_stamp: Option<&str>, db_stamp: &str) -> bool {
    match token_stamp {
        Some(ts) => ts == db_stamp,
        None => true, // tokens without sstamp pre-date the mechanism — allow
    }
}

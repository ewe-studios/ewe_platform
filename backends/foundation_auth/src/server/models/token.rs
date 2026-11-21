//! Refresh token entity for the `IdP` server.

use crate::shared::clock::now_millis;
use crate::shared::constant_time_eq;
use foundation_context::Ctx;
use serde::{Deserialize, Serialize};

use super::client::hex_sha256;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshToken {
    pub id: String,
    pub user_id: String,
    pub client_id: String,
    pub token_hash: String,
    pub expires_at: i64,
    pub rotated_at: Option<i64>,
    pub created_at: i64,
}

impl RefreshToken {
    #[must_use]
    pub fn verify(&self, token: &str) -> bool {
        let hash = hex_sha256(token);
        constant_time_eq(hash.as_bytes(), self.token_hash.as_bytes())
    }

    #[must_use]
    pub fn is_expired(&self, ctx: &Ctx) -> bool {
        now_millis(ctx) >= self.expires_at
    }

    #[must_use]
    pub fn is_rotated(&self) -> bool {
        self.rotated_at.is_some()
    }
}

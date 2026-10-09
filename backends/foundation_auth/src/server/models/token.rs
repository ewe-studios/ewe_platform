//! Refresh token entity for the IdP server.

use chrono::Utc;
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
    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp_millis() >= self.expires_at
    }

    #[must_use]
    pub fn is_rotated(&self) -> bool {
        self.rotated_at.is_some()
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}


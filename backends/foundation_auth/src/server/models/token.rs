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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_token(plaintext: &str) -> RefreshToken {
        RefreshToken {
            id: "rt_1".into(),
            user_id: "u1".into(),
            client_id: "c1".into(),
            token_hash: hex_sha256(plaintext),
            expires_at: Utc::now().timestamp_millis() + 86_400_000,
            rotated_at: None,
            created_at: Utc::now().timestamp_millis(),
        }
    }

    #[test]
    fn test_verify_correct() {
        let rt = test_token("my_refresh_token");
        assert!(rt.verify("my_refresh_token"));
    }

    #[test]
    fn test_verify_wrong() {
        let rt = test_token("my_refresh_token");
        assert!(!rt.verify("wrong_token"));
    }

    #[test]
    fn test_not_expired() {
        let rt = test_token("tok");
        assert!(!rt.is_expired());
    }

    #[test]
    fn test_not_rotated() {
        let rt = test_token("tok");
        assert!(!rt.is_rotated());
    }

    #[test]
    fn test_rotated() {
        let mut rt = test_token("tok");
        rt.rotated_at = Some(Utc::now().timestamp_millis());
        assert!(rt.is_rotated());
    }
}

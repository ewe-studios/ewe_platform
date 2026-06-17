//! OAuth client entity for the IdP server.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthClient {
    pub id: String,
    pub name: String,
    pub client_secret_hash: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub scopes: Vec<String>,
    pub is_public: bool,
    pub created_at: i64,
}

impl OAuthClient {
    #[must_use]
    pub fn verify_secret(&self, secret: &str) -> bool {
        let hash = hex_sha256(secret);
        constant_time_eq(hash.as_bytes(), self.client_secret_hash.as_bytes())
    }

    #[must_use]
    pub fn allows_redirect(&self, uri: &str) -> bool {
        self.redirect_uris.iter().any(|r| r == uri)
    }

    #[must_use]
    pub fn allows_grant(&self, grant: &str) -> bool {
        self.grant_types.iter().any(|g| g == grant)
    }

    #[must_use]
    pub fn allows_scope(&self, scope: &str) -> bool {
        scope
            .split_whitespace()
            .all(|s| self.scopes.iter().any(|cs| cs == s))
    }
}

pub(crate) fn hex_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
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


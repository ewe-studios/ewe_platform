//! Authorization code and device code entities.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizationCode {
    pub code: String,
    pub user_id: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: Option<String>,
    pub scope: String,
    pub nonce: Option<String>,
    pub expires_at: i64,
    pub created_at: i64,
}

impl AuthorizationCode {
    #[must_use]
    pub fn new(
        user_id: String,
        client_id: String,
        redirect_uri: String,
        code_challenge: Option<String>,
        scope: String,
        nonce: Option<String>,
        ttl: Duration,
    ) -> Self {
        let now = Utc::now().timestamp_millis();
        Self {
            code: generate_random_code(64),
            user_id,
            client_id,
            redirect_uri,
            code_challenge,
            scope,
            nonce,
            expires_at: now + ttl.as_millis() as i64,
            created_at: now,
        }
    }

    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp_millis() >= self.expires_at
    }

    #[must_use]
    pub fn verify_pkce(&self, code_verifier: &str) -> bool {
        let Some(ref challenge) = self.code_challenge else {
            return true;
        };
        let mut hasher = Sha256::new();
        hasher.update(code_verifier.as_bytes());
        let computed = URL_SAFE_NO_PAD.encode(hasher.finalize());
        computed == *challenge
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub client_id: String,
    pub scope: String,
    pub expires_at: i64,
    pub interval: u32,
    pub user_id: Option<String>,
    pub created_at: i64,
}

impl DeviceCode {
    #[must_use]
    pub fn new(client_id: String, scope: String, ttl: Duration, interval: u32) -> Self {
        let now = Utc::now().timestamp_millis();
        Self {
            device_code: generate_random_code(64),
            user_code: generate_user_code(),
            client_id,
            scope,
            expires_at: now + ttl.as_millis() as i64,
            interval,
            user_id: None,
            created_at: now,
        }
    }

    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp_millis() >= self.expires_at
    }

    #[must_use]
    pub fn is_approved(&self) -> bool {
        self.user_id.is_some()
    }
}

fn generate_random_code(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(&buf)
}

fn generate_user_code() -> String {
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::thread_rng();
    let mut code = String::with_capacity(9);
    for i in 0..8 {
        if i == 4 {
            code.push('-');
        }
        let idx = (rng.next_u32() as usize) % CHARS.len();
        code.push(CHARS[idx] as char);
    }
    code
}


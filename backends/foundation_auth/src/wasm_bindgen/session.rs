//! Async session manager for wasm32 environments.
//!
//! WHY: The sync `SessionManager` routes storage through valtron iterators.
//! On miniflare, `schedule_future` → `drive_non_send_iterator` blocks the
//! JS event loop, preventing D1 Promises from resolving.
//! `WasmSessionManager` calls async storage methods directly, which use
//! `JsFuture::from(promise).await` and yield properly.
//!
//! WHAT: Lightweight session management with HMAC-SHA256 signed tokens,
//! storing sessions via `AsyncCredentialStore`.
//!
//! HOW: Generic over `S: AsyncCredentialStore`, delegates to its `*_async` methods.

use std::sync::Arc;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::shared::credential_store::AsyncCredentialStore;

const COOKIE_NAME: &str = "session";
const DEFAULT_SESSION_DURATION_SECS: i64 = 86_400;

/// Session payload stored via AsyncCredentialStore.
#[derive(Serialize, Deserialize)]
pub struct SessionPayload {
    pub id: String,
    pub uid: String,
    pub exp: i64,
}

/// Async session manager for wasm32.
pub struct WasmSessionManager<S: AsyncCredentialStore> {
    store: Arc<S>,
    signing_key: [u8; 32],
}

impl<S: AsyncCredentialStore> WasmSessionManager<S> {
    pub const COOKIE_NAME: &'static str = COOKIE_NAME;
    pub const DEFAULT_SESSION_DURATION_SECS: i64 = DEFAULT_SESSION_DURATION_SECS;

    pub fn new(store: Arc<S>, signing_key: [u8; 32]) -> Self {
        Self { store, signing_key }
    }

    /// Create a session and return the signed token.
    pub async fn create_session(&self, user_id: &str) -> Result<String, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let exp = chrono::Utc::now().timestamp() + DEFAULT_SESSION_DURATION_SECS;

        let payload = SessionPayload { id: id.clone(), uid: user_id.to_string(), exp };
        let json = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
        let token = Self::create_token(&self.signing_key, json.as_bytes());

        self.store
            .set_async(&format!("session:{id}"), payload)
            .await
            .map_err(|e| format!("D1 insert failed: {e:?}"))?;

        Ok(token)
    }

    /// Validate a session token and return the user ID.
    pub async fn validate_session(&self, token: &str) -> Result<String, String> {
        let payload = Self::verify_token(&self.signing_key, token)?;
        let stored: Option<SessionPayload> = self
            .store
            .get_async(&format!("session:{}", payload.id))
            .await
            .map_err(|e| format!("D1 query failed: {e:?}"))?;
        if stored.is_none() {
            return Err("session not found".to_string());
        }
        Ok(payload.uid)
    }

    /// Revoke a session.
    pub async fn revoke_session(&self, token: &str) -> Result<(), String> {
        let payload = match Self::verify_token(&self.signing_key, token) {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };
        self.store
            .delete_async(&format!("session:{}", payload.id))
            .await
            .map_err(|e| format!("D1 delete failed: {e:?}"))?;
        Ok(())
    }

    /// Format a Set-Cookie header value.
    pub fn session_cookie(token: &str) -> String {
        format!(
            "{}={}; Path=/; HttpOnly; Max-Age={}; SameSite=Lax",
            COOKIE_NAME,
            token,
            DEFAULT_SESSION_DURATION_SECS
        )
    }

    /// Clear the session cookie.
    pub fn clear_cookie() -> String {
        format!("{}=; Path=/; HttpOnly; Max-Age=0; SameSite=Lax", COOKIE_NAME)
    }

    // ========================================================================
    // Token signing (HMAC-SHA256)
    // ========================================================================

    fn create_token(key: &[u8; 32], payload_json: &[u8]) -> String {
        let encoded = base64_encode(payload_json);
        let sig = Self::sign_payload(key, payload_json);
        format!("{encoded}.{sig}")
    }

    fn sign_payload(key: &[u8; 32], payload_json: &[u8]) -> String {
        let sig = hmac_sha256(key, payload_json);
        base64_encode(&sig)
    }

    fn verify_token(key: &[u8; 32], token: &str) -> Result<SessionPayload, String> {
        let dot = token.rfind('.').ok_or_else(|| "invalid token format".to_string())?;
        let encoded = &token[..dot];
        let sig = &token[dot + 1..];
        let payload_bytes = base64_decode(encoded)?;
        let expected_sig = Self::sign_payload(key, &payload_bytes);
        if sig != expected_sig {
            return Err("invalid signature".to_string());
        }
        let payload: SessionPayload =
            serde_json::from_slice(&payload_bytes).map_err(|e| e.to_string())?;
        let now = chrono::Utc::now().timestamp();
        if payload.exp < now {
            return Err("session expired".to_string());
        }
        Ok(payload)
    }
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;
    let mut key_block = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let hashed = sha256(key);
        key_block[..32].copy_from_slice(&hashed);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK_SIZE];
    let mut opad = [0x5cu8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        ipad[i] ^= key_block[i];
        opad[i] ^= key_block[i];
    }
    let inner = {
        let mut h = Sha256::new();
        h.update(&ipad);
        h.update(message);
        h.finalize()
    };
    let mut h = Sha256::new();
    h.update(&opad);
    h.update(&inner);
    h.finalize().into()
}

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn base64_encode(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    URL_SAFE_NO_PAD.decode(s).map_err(|e| e.to_string())
}

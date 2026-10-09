//! Async session manager for wasm32.
//!
//! WHY: The native `SessionManager` is sync (calls `CredentialStore` sync
//! methods). On wasm32, D1 operations are JS Promises that require the
//! event loop to resolve. A sync blocking spin-loop deadlocks on
//! miniflare because D1 Promises go through the event loop.
//!
//! This async variant uses `WasmCredentialStore` (async-only) backed by
//! `D1WasmStorage`, so the wasm runtime can properly `.await` all storage
//! operations without blocking the JS event loop.
//!
//! NOTE: This file is NOT activated in mod.rs. It is here for review.
//! When the native `SessionManager` is ready for wasm (async
//! `CredentialStore` impl), this file can be deleted or merged.

use std::sync::Arc;

use crate::wasm::D1WasmStorage;
use crate::core::storage_provider::DataValue;

// ===========================================================================
// Session payload — the data we sign into cookies.
// ===========================================================================

#[derive(serde::Serialize, serde::Deserialize)]
struct SessionPayload {
    /// Unique session ID (UUID v4).
    id: String,
    /// User ID (typically email).
    uid: String,
    /// Unix timestamp when this session expires.
    exp: i64,
}

// ===========================================================================
// WasmSessionManager — async session management with signed cookies + D1.
//
// Cookie format: base64(payload).base64(hmac_sha256(payload))
// ===========================================================================

/// HMAC-SHA256 signing key — must be 32 bytes.
/// CHANGE THIS in real deployments.
const SIGNING_KEY: &[u8; 32] = b"CHANGE-ME-TO-32-RANDOM-BYTES!!!!";

pub struct WasmSessionManager {
    storage: Arc<D1WasmStorage>,
}

impl WasmSessionManager {
    /// Cookie name for the session token.
    pub const COOKIE_NAME: &'static str = "session";

    /// Default session duration in seconds (24 hours).
    pub const DEFAULT_SESSION_DURATION_SECS: i64 = 86_400;

    /// Create a new session manager.
    #[must_use]
    pub fn new(storage: Arc<D1WasmStorage>) -> Self {
        Self { storage }
    }

    /// Create a new session: store in D1 and return a signed cookie string.
    pub async fn create_session(&self, user_id: &str) -> Result<String, String> {
        let id = foundation_compact::ids::new_scru128_string();
        let exp = chrono::Utc::now().timestamp() + Self::DEFAULT_SESSION_DURATION_SECS;
        let now_ms = chrono::Utc::now().timestamp_millis();

        let payload = SessionPayload {
            id: id.clone(),
            uid: user_id.to_string(),
            exp,
        };

        let json = serde_json::to_string(&payload).map_err(|e| format!("serialize: {e}"))?;
        let token = Self::create_token(json.as_bytes());

        let session_json = serde_json::to_string(&payload).map_err(|e| format!("serialize: {e}"))?;
        self.storage
            .execute_async(
                "INSERT INTO kv_store (key, value, updated_at) VALUES (?, ?, ?)",
                &[
                    DataValue::Text(format!("session:{}", id)),
                    DataValue::Text(session_json),
                    DataValue::Integer(now_ms),
                ],
            )
            .await
            .map_err(|e| format!("D1 insert failed: {e:?}"))?;

        Ok(token)
    }

    /// Validate a session token and return the user_id.
    pub async fn validate_session(&self, token: &str) -> Result<String, String> {
        let payload = Self::verify_token(token)?;

        let rows = self
            .storage
            .query_async(
                "SELECT value FROM kv_store WHERE key = ?",
                &[DataValue::Text(format!("session:{}", payload.id))],
            )
            .await
            .map_err(|e| format!("D1 query failed: {e:?}"))?;

        if rows.is_empty() {
            return Err("session not found".to_string());
        }

        Ok(payload.uid)
    }

    /// Revoke a session.
    pub async fn revoke_session(&self, token: &str) -> Result<(), String> {
        let payload = match Self::verify_token(token) {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };

        self.storage
            .execute_async(
                "DELETE FROM kv_store WHERE key = ?",
                &[DataValue::Text(format!("session:{}", payload.id))],
            )
            .await
            .map_err(|e| format!("D1 delete failed: {e:?}"))?;

        Ok(())
    }

    /// Build a session cookie header value.
    #[must_use]
    pub fn session_cookie(token: &str) -> String {
        let max_age = Self::DEFAULT_SESSION_DURATION_SECS;
        format!(
            "{}={}; Path=/; HttpOnly; Max-Age={}; SameSite=Lax",
            Self::COOKIE_NAME,
            token,
            max_age
        )
    }

    /// Build a cookie-clearing header value.
    #[must_use]
    pub fn clear_cookie() -> String {
        format!(
            "{}=; Path=/; HttpOnly; Max-Age=0; SameSite=Lax",
            Self::COOKIE_NAME
        )
    }

    // ===========================================================================
    // Token signing internals.
    // ===========================================================================

    fn sign_payload(payload_json: &[u8]) -> String {
        let sig = hmac_sha256(SIGNING_KEY, payload_json);
        base64_encode(&sig)
    }

    fn create_token(payload_json: &[u8]) -> String {
        let encoded = base64_encode(payload_json);
        let sig = Self::sign_payload(payload_json);
        format!("{encoded}.{sig}")
    }

    fn verify_token(token: &str) -> Result<SessionPayload, String> {
        let dot = token
            .rfind('.')
            .ok_or_else(|| "invalid token format".to_string())?;
        let encoded = &token[..dot];
        let sig = &token[dot + 1..];

        let payload_bytes = base64_decode(encoded)?;
        let expected_sig = Self::sign_payload(&payload_bytes);

        if sig != expected_sig {
            return Err("invalid signature".to_string());
        }

        let payload: SessionPayload =
            serde_json::from_slice(&payload_bytes).map_err(|e| format!("deserialize: {e}"))?;

        let now = chrono::Utc::now().timestamp();
        if payload.exp < now {
            return Err("session expired".to_string());
        }

        Ok(payload)
    }
}

// ===========================================================================
// Crypto helpers.
// ===========================================================================

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};

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
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn base64_encode(data: &[u8]) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD.decode(s).map_err(|e| format!("base64 decode: {e}"))
}

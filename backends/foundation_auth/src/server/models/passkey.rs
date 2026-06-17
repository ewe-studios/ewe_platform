//! Passkey (WebAuthn credential) model.
//! Always compiled under `server` feature — needed by account management (F09)
//! for listing/deleting passkeys regardless of whether WebAuthn crypto is available.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Passkey {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub credential_id: Vec<u8>,
    pub credential_public_key: Vec<u8>,
    pub counter: u32,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

//! OAuth client management service.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use rand::RngCore;

use super::super::models::client::hex_sha256;
use super::super::models::OAuthClient;

#[derive(Debug)]
pub enum ClientServiceError {
    Storage(String),
    NotFound,
}

impl core::fmt::Display for ClientServiceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Storage(s) => write!(f, "Storage error: {s}"),
            Self::NotFound => write!(f, "Client not found"),
        }
    }
}

impl std::error::Error for ClientServiceError {}

pub fn create_client_record(
    name: &str,
    redirect_uris: Vec<String>,
    grant_types: Vec<String>,
    scopes: Vec<String>,
    is_public: bool,
) -> (OAuthClient, String) {
    let client_id = uuid::Uuid::new_v4().to_string();
    let secret = generate_client_secret();
    let secret_hash = hex_sha256(&secret);

    let client = OAuthClient {
        id: client_id,
        name: name.to_string(),
        client_secret_hash: secret_hash,
        redirect_uris,
        grant_types,
        scopes,
        is_public,
        created_at: Utc::now().timestamp_millis(),
    };

    (client, secret)
}

fn generate_client_secret() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}


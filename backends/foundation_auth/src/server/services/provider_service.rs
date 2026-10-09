//! Upstream provider management service.
//!
//! WHY: CRUD for `UpstreamProvider` rows plus encryption of the client secret
//! at rest. The IdP admin API (feature 06) and the social login flow (feature
//! 04) both go through this service.
//!
//! WHAT: `ProviderService<QS>` (storage-backed CRUD over `QueryStore`) and
//! `ProviderCrypto` (ChaCha20-Poly1305 AEAD, key derived from a secret via
//! Argon2id).
//!
//! HOW: the sync `QueryStore` is the persistence seam (matches the rest of the
//! IdP server storage layer). Secrets are stored as `nonce || ciphertext` in a
//! BLOB column. The 32-byte AEAD key is derived once from `PROVIDER_SECRET_KEY`
//! (or an explicit passphrase) via Argon2id with a fixed context salt, so the
//! same passphrase always yields the same key (decrypt must be deterministic).

use std::sync::Arc;

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use chrono::Utc;
use rand::RngCore;

use foundation_db::core::storage_provider::{DataValue, QueryStore, SqlRow, StorageItemStream};
use foundation_core::valtron::Stream;

use crate::shared::password_hash::argon2id_derive;

use super::super::models::provider::{
    ProviderMapping, ProviderType, ProviderUpdate, UpstreamProvider,
};

const NONCE_LEN: usize = 12;

/// Fixed context salt for deriving the AEAD key from the configured passphrase.
/// This is not secret — its role is to bind the derivation to this use, not to
/// add entropy. The passphrase is the secret. Argon2id makes brute-forcing a
/// weak passphrase expensive.
const KEY_DERIVATION_SALT: &[u8] = b"foundation_auth::provider_secret::v1";

/// Errors from provider CRUD and secret encryption.
#[derive(Debug)]
pub enum ProviderServiceError {
    /// A storage query failed.
    Storage(String),
    /// The provider id was not found.
    NotFound,
    /// The provider row/config failed validation.
    Validation(String),
    /// Encryption or decryption failed.
    Crypto(String),
    /// A stored row could not be parsed back into a model.
    Parse(String),
}

impl core::fmt::Display for ProviderServiceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Storage(s) => write!(f, "Storage error: {s}"),
            Self::NotFound => write!(f, "Provider not found"),
            Self::Validation(s) => write!(f, "Validation error: {s}"),
            Self::Crypto(s) => write!(f, "Crypto error: {s}"),
            Self::Parse(s) => write!(f, "Parse error: {s}"),
        }
    }
}

impl std::error::Error for ProviderServiceError {}

type Result<T> = std::result::Result<T, ProviderServiceError>;

/// Encrypts/decrypts provider secrets with ChaCha20-Poly1305.
///
/// The 32-byte key is derived from a passphrase via Argon2id (deterministic —
/// same passphrase → same key, required so decrypt works across restarts).
#[derive(Clone)]
pub struct ProviderCrypto {
    key: Key,
    key_id: String,
}

impl core::fmt::Debug for ProviderCrypto {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never print the key material.
        f.debug_struct("ProviderCrypto")
            .field("key_id", &self.key_id)
            .field("key", &"<redacted>")
            .finish()
    }
}

impl ProviderCrypto {
    /// Derive an encryption key from a passphrase. `key_id` labels the key for
    /// rotation (stored on each encrypted provider row).
    #[must_use]
    pub fn from_passphrase(passphrase: &str, key_id: impl Into<String>) -> Self {
        // Argon2id, 32-byte output, Bitwarden-ish params. Deterministic given
        // the fixed salt, so encrypt today / decrypt tomorrow both derive the
        // same key.
        let derived = argon2id_derive(passphrase.as_bytes(), KEY_DERIVATION_SALT, 3, 65536, 4, 32);
        let key = Key::clone_from_slice(derived.as_bytes());
        Self {
            key,
            key_id: key_id.into(),
        }
    }

    /// Read the passphrase from the `PROVIDER_SECRET_KEY` env var.
    ///
    /// # Errors
    ///
    /// Returns `Validation` if the env var is missing or empty.
    pub fn from_env() -> Result<Self> {
        let passphrase = std::env::var("PROVIDER_SECRET_KEY")
            .map_err(|_| ProviderServiceError::Validation("PROVIDER_SECRET_KEY not set".into()))?;
        if passphrase.is_empty() {
            return Err(ProviderServiceError::Validation(
                "PROVIDER_SECRET_KEY is empty".into(),
            ));
        }
        Ok(Self::from_passphrase(&passphrase, "default"))
    }

    /// The key id label stored alongside each encrypted secret.
    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Encrypt plaintext → `nonce || ciphertext`.
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = ChaCha20Poly1305::new(&self.key);
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| ProviderServiceError::Crypto(format!("encrypt: {e}")))?;
        let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Decrypt `nonce || ciphertext` → plaintext.
    fn decrypt(&self, blob: &[u8]) -> Result<Vec<u8>> {
        if blob.len() < NONCE_LEN {
            return Err(ProviderServiceError::Crypto(
                "ciphertext shorter than nonce".into(),
            ));
        }
        let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
        let cipher = ChaCha20Poly1305::new(&self.key);
        let nonce = Nonce::from_slice(nonce_bytes);
        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| ProviderServiceError::Crypto(format!("decrypt: {e}")))
    }
}

/// Storage-backed CRUD for upstream providers.
pub struct ProviderService<QS: QueryStore + 'static> {
    query_store: Arc<QS>,
    crypto: ProviderCrypto,
}

impl<QS: QueryStore + 'static> ProviderService<QS> {
    /// Create a service over a query store and a crypto wrapper.
    pub fn new(query_store: Arc<QS>, crypto: ProviderCrypto) -> Self {
        Self {
            query_store,
            crypto,
        }
    }

    /// Insert a new provider. `client_secret_ciphertext` is ignored here — set
    /// the secret separately with [`set_secret`](Self::set_secret) so it is
    /// always encrypted.
    ///
    /// # Errors
    ///
    /// - `Validation` if `id`/`client_id` is empty or endpoints are missing.
    /// - `Storage` on insert failure (including a duplicate id).
    pub fn create(&self, mut provider: UpstreamProvider) -> Result<UpstreamProvider> {
        self.validate(&provider)?;

        let now = Utc::now().timestamp_millis();
        provider.created_at = now;
        provider.updated_at = now;
        provider.encryption_key_id = self.crypto.key_id().to_string();
        // Secret set via set_secret, never on create.
        provider.client_secret_ciphertext = None;

        let scopes_json = serde_json::to_string(&provider.scopes)
            .map_err(|e| ProviderServiceError::Parse(e.to_string()))?;
        let mapping_json = serde_json::to_string(&provider.mapping_config)
            .map_err(|e| ProviderServiceError::Parse(e.to_string()))?;

        self.query_store
            .execute(
                "INSERT INTO upstream_providers \
                 (id, name, provider_type, client_id, client_secret_ciphertext, encryption_key_id, \
                  authorization_url, token_url, userinfo_url, discovery_url, scopes, is_active, \
                  mapping_config, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                &[
                    DataValue::Text(provider.id.clone()),
                    DataValue::Text(provider.name.clone()),
                    DataValue::Text(provider.provider_type.as_str().to_string()),
                    DataValue::Text(provider.client_id.clone()),
                    DataValue::Text(provider.encryption_key_id.clone()),
                    opt_text(&provider.authorization_url),
                    opt_text(&provider.token_url),
                    opt_text(&provider.userinfo_url),
                    opt_text(&provider.discovery_url),
                    DataValue::Text(scopes_json),
                    DataValue::Integer(i64::from(provider.is_active)),
                    DataValue::Text(mapping_json),
                    DataValue::Integer(provider.created_at),
                    DataValue::Integer(provider.updated_at),
                ],
            )
            .map_err(|e| ProviderServiceError::Storage(e.to_string()))?;

        Ok(provider)
    }

    /// Find a provider by id. Returns `None` if not present.
    ///
    /// # Errors
    ///
    /// `Storage` on query failure, `Parse` if a row can't be decoded.
    pub fn find_by_id(&self, id: &str) -> Result<Option<UpstreamProvider>> {
        let mut stream = self
            .query_store
            .query(
                "SELECT id, name, provider_type, client_id, client_secret_ciphertext, \
                 encryption_key_id, authorization_url, token_url, userinfo_url, discovery_url, \
                 scopes, is_active, mapping_config, created_at, updated_at \
                 FROM upstream_providers WHERE id = ?",
                &[DataValue::Text(id.to_string())],
            )
            .map_err(|e| ProviderServiceError::Storage(e.to_string()))?;

        match first_row(&mut stream)? {
            Some(row) => Ok(Some(parse_provider_row(&row)?)),
            None => Ok(None),
        }
    }

    /// List all active providers, ordered by id.
    ///
    /// # Errors
    ///
    /// `Storage` on query failure, `Parse` if a row can't be decoded.
    pub fn find_active(&self) -> Result<Vec<UpstreamProvider>> {
        Self::drain_providers(self.query_store.query(
            "SELECT id, name, provider_type, client_id, client_secret_ciphertext, \
             encryption_key_id, authorization_url, token_url, userinfo_url, discovery_url, \
             scopes, is_active, mapping_config, created_at, updated_at \
             FROM upstream_providers WHERE is_active = 1 ORDER BY id",
            &[],
        ))
    }

    /// List all providers (active and inactive), ordered by id.
    ///
    /// # Errors
    ///
    /// `Storage` on query failure, `Parse` if a row can't be decoded.
    pub fn list_all(&self) -> Result<Vec<UpstreamProvider>> {
        Self::drain_providers(self.query_store.query(
            "SELECT id, name, provider_type, client_id, client_secret_ciphertext, \
             encryption_key_id, authorization_url, token_url, userinfo_url, discovery_url, \
             scopes, is_active, mapping_config, created_at, updated_at \
             FROM upstream_providers ORDER BY id",
            &[],
        ))
    }

    fn drain_providers(
        result: std::result::Result<StorageItemStream<'_, SqlRow>, foundation_db::StorageError>,
    ) -> Result<Vec<UpstreamProvider>> {
        let stream = result.map_err(|e| ProviderServiceError::Storage(e.to_string()))?;
        let mut out = Vec::new();
        for item in stream {
            match item {
                Stream::Next(Ok(row)) => out.push(parse_provider_row(&row)?),
                Stream::Next(Err(e)) => return Err(ProviderServiceError::Storage(e.to_string())),
                _ => {}
            }
        }
        Ok(out)
    }

    /// Apply partial updates to an existing provider. Returns the updated row.
    ///
    /// # Errors
    ///
    /// `NotFound` if the id doesn't exist; `Validation` if the result would be
    /// invalid; `Storage`/`Parse` on failure.
    pub fn update(&self, id: &str, updates: ProviderUpdate) -> Result<UpstreamProvider> {
        let mut provider = self.find_by_id(id)?.ok_or(ProviderServiceError::NotFound)?;

        if let Some(v) = updates.name {
            provider.name = v;
        }
        if let Some(v) = updates.provider_type {
            provider.provider_type = v;
        }
        if let Some(v) = updates.client_id {
            provider.client_id = v;
        }
        if let Some(v) = updates.authorization_url {
            provider.authorization_url = v;
        }
        if let Some(v) = updates.token_url {
            provider.token_url = v;
        }
        if let Some(v) = updates.userinfo_url {
            provider.userinfo_url = v;
        }
        if let Some(v) = updates.discovery_url {
            provider.discovery_url = v;
        }
        if let Some(v) = updates.scopes {
            provider.scopes = v;
        }
        if let Some(v) = updates.is_active {
            provider.is_active = v;
        }
        if let Some(v) = updates.mapping_config {
            provider.mapping_config = v;
        }

        self.validate(&provider)?;
        provider.updated_at = Utc::now().timestamp_millis();

        let scopes_json = serde_json::to_string(&provider.scopes)
            .map_err(|e| ProviderServiceError::Parse(e.to_string()))?;
        let mapping_json = serde_json::to_string(&provider.mapping_config)
            .map_err(|e| ProviderServiceError::Parse(e.to_string()))?;

        self.query_store
            .execute(
                "UPDATE upstream_providers SET name = ?, provider_type = ?, client_id = ?, \
                 authorization_url = ?, token_url = ?, userinfo_url = ?, discovery_url = ?, \
                 scopes = ?, is_active = ?, mapping_config = ?, updated_at = ? WHERE id = ?",
                &[
                    DataValue::Text(provider.name.clone()),
                    DataValue::Text(provider.provider_type.as_str().to_string()),
                    DataValue::Text(provider.client_id.clone()),
                    opt_text(&provider.authorization_url),
                    opt_text(&provider.token_url),
                    opt_text(&provider.userinfo_url),
                    opt_text(&provider.discovery_url),
                    DataValue::Text(scopes_json),
                    DataValue::Integer(i64::from(provider.is_active)),
                    DataValue::Text(mapping_json),
                    DataValue::Integer(provider.updated_at),
                    DataValue::Text(provider.id.clone()),
                ],
            )
            .map_err(|e| ProviderServiceError::Storage(e.to_string()))?;

        Ok(provider)
    }

    /// Delete a provider by id. Idempotent — deleting a missing id is `Ok`.
    ///
    /// # Errors
    ///
    /// `Storage` on failure.
    pub fn delete(&self, id: &str) -> Result<()> {
        self.query_store
            .execute(
                "DELETE FROM upstream_providers WHERE id = ?",
                &[DataValue::Text(id.to_string())],
            )
            .map_err(|e| ProviderServiceError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Encrypt and store the client secret for a provider.
    ///
    /// # Errors
    ///
    /// `NotFound` if the id doesn't exist; `Crypto`/`Storage` on failure.
    pub fn set_secret(&self, id: &str, plaintext_secret: &str) -> Result<()> {
        // Ensure the provider exists first (so we return NotFound, not a silent
        // no-op update).
        if self.find_by_id(id)?.is_none() {
            return Err(ProviderServiceError::NotFound);
        }
        let ciphertext = self.crypto.encrypt(plaintext_secret.as_bytes())?;
        self.query_store
            .execute(
                "UPDATE upstream_providers SET client_secret_ciphertext = ?, encryption_key_id = ?, \
                 updated_at = ? WHERE id = ?",
                &[
                    DataValue::Blob(ciphertext),
                    DataValue::Text(self.crypto.key_id().to_string()),
                    DataValue::Integer(Utc::now().timestamp_millis()),
                    DataValue::Text(id.to_string()),
                ],
            )
            .map_err(|e| ProviderServiceError::Storage(e.to_string()))?;
        Ok(())
    }

    /// Decrypt and return the client secret. Only called during the auth flow;
    /// never logged.
    ///
    /// # Errors
    ///
    /// `NotFound` if the id doesn't exist; `Validation` if no secret is set;
    /// `Crypto` if decryption fails.
    pub fn get_secret(&self, id: &str) -> Result<String> {
        let provider = self.find_by_id(id)?.ok_or(ProviderServiceError::NotFound)?;
        let blob = provider
            .client_secret_ciphertext
            .ok_or_else(|| ProviderServiceError::Validation("no secret set".into()))?;
        let plaintext = self.crypto.decrypt(&blob)?;
        String::from_utf8(plaintext)
            .map_err(|e| ProviderServiceError::Crypto(format!("secret not utf8: {e}")))
    }

    fn validate(&self, provider: &UpstreamProvider) -> Result<()> {
        if provider.id.trim().is_empty() {
            return Err(ProviderServiceError::Validation("id is empty".into()));
        }
        if provider.client_id.trim().is_empty() {
            return Err(ProviderServiceError::Validation("client_id is empty".into()));
        }
        if !provider.has_endpoints() {
            return Err(ProviderServiceError::Validation(
                "must set discovery_url or both authorization_url and token_url".into(),
            ));
        }
        Ok(())
    }
}

/// SQL `NULL` for `None`, else `Text`.
fn opt_text(val: &Option<String>) -> DataValue {
    match val {
        Some(s) => DataValue::Text(s.clone()),
        None => DataValue::Null,
    }
}

/// Pull the first row from a lazy query stream (skipping readiness markers).
fn first_row(
    stream: &mut foundation_db::core::storage_provider::StorageItemStream<'_, SqlRow>,
) -> Result<Option<SqlRow>> {
    for item in stream {
        match item {
            Stream::Next(Ok(row)) => return Ok(Some(row)),
            Stream::Next(Err(e)) => return Err(ProviderServiceError::Storage(e.to_string())),
            _ => {}
        }
    }
    Ok(None)
}

/// Decode a full provider row from the SELECT column order used above.
fn parse_provider_row(row: &SqlRow) -> Result<UpstreamProvider> {
    let parse = |e: foundation_db::StorageError| ProviderServiceError::Parse(e.to_string());

    let provider_type: String = row.get_by_name("provider_type").map_err(parse)?;
    let secret: Option<Vec<u8>> = row.get_by_name("client_secret_ciphertext").map_err(parse)?;
    let auth_url: Option<String> = row.get_by_name("authorization_url").map_err(parse)?;
    let token_url: Option<String> = row.get_by_name("token_url").map_err(parse)?;
    let userinfo_url: Option<String> = row.get_by_name("userinfo_url").map_err(parse)?;
    let discovery_url: Option<String> = row.get_by_name("discovery_url").map_err(parse)?;
    let scopes_json: String = row.get_by_name("scopes").map_err(parse)?;
    let is_active: i64 = row.get_by_name("is_active").map_err(parse)?;
    let mapping_json: String = row.get_by_name("mapping_config").map_err(parse)?;

    let scopes: Vec<String> = serde_json::from_str(&scopes_json)
        .map_err(|e| ProviderServiceError::Parse(format!("scopes: {e}")))?;
    let mapping_config: ProviderMapping = serde_json::from_str(&mapping_json)
        .map_err(|e| ProviderServiceError::Parse(format!("mapping_config: {e}")))?;

    Ok(UpstreamProvider {
        id: row.get_by_name("id").map_err(parse)?,
        name: row.get_by_name("name").map_err(parse)?,
        provider_type: ProviderType::from_str_lenient(&provider_type),
        client_id: row.get_by_name("client_id").map_err(parse)?,
        client_secret_ciphertext: secret,
        encryption_key_id: row.get_by_name("encryption_key_id").map_err(parse)?,
        authorization_url: auth_url,
        token_url,
        userinfo_url,
        discovery_url,
        scopes,
        is_active: is_active != 0,
        mapping_config,
        created_at: row.get_by_name("created_at").map_err(parse)?,
        updated_at: row.get_by_name("updated_at").map_err(parse)?,
    })
}

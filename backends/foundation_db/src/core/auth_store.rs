//! Auth storage traits — abstract interface for passkey, ToS, and user storage.
//!
//! WHY: The auth server should not depend on specific storage implementations.
//! These traits allow swapping between Turso/SQLite, CF D1, in-memory, etc.
//! while keeping the auth service logic identical.
//!
//! WHAT: Traits for passkey CRUD, ToS acceptance tracking, and user management
//! that any storage backend can implement.

use std::string::String;
use std::vec::Vec;

use crate::core::storage_provider::AsyncStorageItemStream;
use crate::core::errors::StorageResult;

// ─── Stored Types ────────────────────────────────────────────────────────────

/// A stored WebAuthn passkey credential.
///
/// The `credential_public_key` is a CBOR-serialized `webauthn-rs::Passkey`
/// (when `server-native` is enabled) so it can be deserialized back for
/// cryptographic verification.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredPasskey {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub credential_id: Vec<u8>,
    pub credential_public_key: Vec<u8>,
    pub counter: u32,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

/// A record of a user accepting a Terms of Service version.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredTosAcceptance {
    pub user_id: String,
    pub tos_version: String,
    pub accepted_at: i64,
    pub ip_address: Option<String>,
}

// ─── Sync Auth Store ─────────────────────────────────────────────────────────

/// Storage operations for WebAuthn passkeys.
pub trait PasskeyStore {
    fn store_passkey(&self, passkey: &StoredPasskey) -> Result<(), String>;
    fn find_passkeys_by_user(&self, user_id: &str) -> Result<Vec<StoredPasskey>, String>;
    fn find_passkey_by_id(&self, passkey_id: &str) -> Result<Option<StoredPasskey>, String>;
    fn find_passkey_by_credential_id(&self, credential_id: &[u8]) -> Result<Option<StoredPasskey>, String>;
    fn update_passkey_counter(&self, passkey_id: &str, counter: u32) -> Result<(), String>;
    fn update_passkey_name(&self, passkey_id: &str, name: &str) -> Result<(), String>;
    fn delete_passkey(&self, passkey_id: &str) -> Result<(), String>;
}

/// Storage operations for Terms of Service acceptances.
pub trait TosStore {
    fn store_tos_acceptance(&self, acceptance: &StoredTosAcceptance) -> Result<(), String>;
    fn find_tos_acceptance(&self, user_id: &str, tos_version: &str) -> Result<Option<StoredTosAcceptance>, String>;
    fn find_latest_tos_acceptance(&self, user_id: &str) -> Result<Option<StoredTosAcceptance>, String>;
}

/// Combined trait that provides all auth-related storage operations.
pub trait AuthStore: PasskeyStore + TosStore {
    /// Find a user by email (used by passkey login flow).
    /// Returns (user_id, has_password).
    fn find_user_by_email(&self, email: &str) -> Result<Option<(String, bool)>, String>;
}

// ─── Async Auth Store ────────────────────────────────────────────────────────

/// Async storage operations for WebAuthn passkeys.
///
/// Mirrors `PasskeyStore` with `*_async` methods for use with `AsyncQueryStore`
/// backends (Turso/Libsql, D1). Multi-value methods return streams to avoid
/// loading all passkeys into memory at once.
#[async_trait::async_trait]
pub trait AsyncPasskeyStore {
    async fn store_passkey_async(&self, passkey: &StoredPasskey) -> Result<(), String>;
    async fn find_passkeys_by_user_async<'a>(&'a self, user_id: &'a str) -> StorageResult<AsyncStorageItemStream<'a, StoredPasskey>>;
    async fn find_passkey_by_id_async<'a>(&'a self, passkey_id: &'a str) -> StorageResult<Option<StoredPasskey>>;
    async fn find_passkey_by_credential_id_async<'a>(&'a self, credential_id: &'a [u8]) -> StorageResult<Option<StoredPasskey>>;
    async fn update_passkey_counter_async(&self, passkey_id: &str, counter: u32) -> Result<(), String>;
    async fn update_passkey_name_async(&self, passkey_id: &str, name: &str) -> Result<(), String>;
    async fn delete_passkey_async(&self, passkey_id: &str) -> Result<(), String>;
}

/// Async storage operations for Terms of Service acceptances.
#[async_trait::async_trait]
pub trait AsyncTosStore {
    async fn store_tos_acceptance_async(&self, acceptance: &StoredTosAcceptance) -> Result<(), String>;
    async fn find_tos_acceptance_async<'a>(&'a self, user_id: &'a str, tos_version: &'a str) -> StorageResult<Option<StoredTosAcceptance>>;
    async fn find_latest_tos_acceptance_async<'a>(&'a self, user_id: &'a str) -> StorageResult<Option<StoredTosAcceptance>>;
}

/// Combined async auth store — the primary trait for async auth handlers.
#[async_trait::async_trait]
pub trait AsyncAuthStore: AsyncPasskeyStore + AsyncTosStore {
    /// Find a user by email (used by passkey login flow).
    /// Returns (user_id, has_password).
    async fn find_user_by_email_async(&self, email: &str) -> StorageResult<Option<(String, bool)>>;
}

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

// ─── Passkey Storage ─────────────────────────────────────────────────────────

/// A stored WebAuthn passkey credential.
///
/// The `credential_public_key` is a CBOR-serialized `webauthn-rs::Passkey`
/// (when `server-native` is enabled) so it can be deserialized back for
/// cryptographic verification. When `server-native` is not enabled, it
/// stores the raw credential ID bytes.
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

/// Storage operations for WebAuthn passkeys.
///
/// Implement this trait to provide passkey persistence for the auth server.
/// The default implementation uses a `QueryStore` (SQL-based).
pub trait PasskeyStore {
    /// Store a new passkey credential.
    fn store_passkey(&self, passkey: &StoredPasskey) -> Result<(), String>;

    /// Find all passkeys for a user.
    fn find_passkeys_by_user(&self, user_id: &str) -> Result<Vec<StoredPasskey>, String>;

    /// Find a specific passkey by its ID.
    fn find_passkey_by_id(&self, passkey_id: &str) -> Result<Option<StoredPasskey>, String>;

    /// Find a passkey by its credential ID (the raw bytes sent by the authenticator).
    fn find_passkey_by_credential_id(&self, credential_id: &[u8]) -> Result<Option<StoredPasskey>, String>;

    /// Update the authenticator counter after successful authentication.
    fn update_passkey_counter(&self, passkey_id: &str, counter: u32) -> Result<(), String>;

    /// Rename a passkey (user-facing label).
    fn update_passkey_name(&self, passkey_id: &str, name: &str) -> Result<(), String>;

    /// Delete a passkey.
    fn delete_passkey(&self, passkey_id: &str) -> Result<(), String>;
}

// ─── ToS Acceptance Storage ──────────────────────────────────────────────────

/// A record of a user accepting a Terms of Service version.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredTosAcceptance {
    pub user_id: String,
    pub tos_version: String,
    pub accepted_at: i64,
    pub ip_address: Option<String>,
}

/// Storage operations for Terms of Service acceptances.
pub trait TosStore {
    /// Record a user's acceptance of a ToS version.
    fn store_tos_acceptance(&self, acceptance: &StoredTosAcceptance) -> Result<(), String>;

    /// Check if a user has accepted a specific ToS version.
    fn find_tos_acceptance(&self, user_id: &str, tos_version: &str) -> Result<Option<StoredTosAcceptance>, String>;

    /// Get the user's most recent ToS acceptance (any version).
    fn find_latest_tos_acceptance(&self, user_id: &str) -> Result<Option<StoredTosAcceptance>, String>;
}

// ─── Combined Auth Store (convenience trait) ─────────────────────────────────

/// Combined trait that provides all auth-related storage operations.
///
/// This is the primary trait the auth server handlers use. A single
/// implementation typically backs both passkeys and ToS acceptances.
pub trait AuthStore: PasskeyStore + TosStore {}

// Blanket impl: anything implementing both sub-traits gets AuthStore
impl<T: PasskeyStore + TosStore> AuthStore for T {}

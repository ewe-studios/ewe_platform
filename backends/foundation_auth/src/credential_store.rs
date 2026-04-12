//! Credential storage wrapping `foundation_db::StorageProvider`.
//!
//! `foundation_db` already selects the backend (Turso, libsql, D1, R2, JSON
//! file, in-memory) through `StorageBackend`. There is no reason to expose
//! one credential-store wrapper per backend — the wrapper only needs to
//! drain Valtron streams into sync `Result`s. A single [`CredentialStorage`]
//! therefore covers every backend `foundation_db` supports.

use foundation_core::valtron::Stream;
use foundation_db::{KeyValueStore, StorageBackend, StorageError, StorageProvider};
use serde::{Deserialize, Serialize};

use crate::oauth::OAuthToken;

/// Synchronous credential storage API.
///
/// Implementations drain Valtron streams from `foundation_db` into plain
/// `Result` values so that auth code can treat credential IO as blocking.
pub trait CredentialStore: Send + Sync {
    /// Get a credential by key.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError`] if the storage operation or
    /// deserialization fails.
    fn get<V: for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError>;

    /// Set a credential.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError`] if the storage operation or
    /// serialization fails.
    fn set<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError>;

    /// Delete a credential.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError`] if the storage operation fails.
    fn delete(&self, key: &str) -> Result<(), CredentialStoreError>;

    /// Check whether a credential exists.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError`] if the storage operation fails.
    fn exists(&self, key: &str) -> Result<bool, CredentialStoreError>;

    /// List credential keys, optionally filtered by prefix.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError`] if the storage operation fails.
    fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError>;
}

/// Credential store error type.
#[derive(derive_more::From, Debug)]
pub enum CredentialStoreError {
    /// Storage backend error.
    Storage(StorageError),
    /// Serialization error.
    #[from(ignore)]
    Serialization(String),
    /// Credential not found.
    #[from(ignore)]
    NotFound(String),
    /// Generic error.
    #[from(ignore)]
    Generic(String),
}

impl core::fmt::Display for CredentialStoreError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CredentialStoreError::Storage(e) => write!(f, "Storage error: {e}"),
            CredentialStoreError::Serialization(s) => write!(f, "Serialization error: {s}"),
            CredentialStoreError::NotFound(s) => write!(f, "Credential not found: {s}"),
            CredentialStoreError::Generic(s) => write!(f, "Credential store error: {s}"),
        }
    }
}

impl std::error::Error for CredentialStoreError {}

/// Credential storage backed by a [`foundation_db::StorageProvider`].
///
/// The backend (Turso, libsql, D1, R2, JSON file, in-memory) is chosen when
/// the underlying [`StorageProvider`] is constructed. This type just adapts
/// its streaming API to the synchronous [`CredentialStore`] trait.
pub struct CredentialStorage {
    storage: StorageProvider,
}

impl CredentialStorage {
    /// Wrap an existing [`StorageProvider`].
    #[must_use]
    pub fn new(storage: StorageProvider) -> Self {
        Self { storage }
    }

    /// Build credential storage against a Turso database URL.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError`] if the Turso backend cannot be
    /// initialized.
    pub fn turso(url: &str) -> Result<Self, CredentialStoreError> {
        let storage = StorageProvider::new(StorageBackend::Turso {
            url: url.to_string(),
        })?;
        Ok(Self { storage })
    }

    /// Build in-memory credential storage. Intended for tests and local
    /// development.
    #[must_use]
    pub fn memory() -> Self {
        Self {
            storage: StorageProvider::memory(),
        }
    }

    /// Borrow the underlying [`StorageProvider`].
    #[must_use]
    pub fn provider(&self) -> &StorageProvider {
        &self.storage
    }
}

impl CredentialStore for CredentialStorage {
    fn get<V: for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError> {
        let stream = self.storage.get(key).map_err(|e| match e {
            StorageError::NotFound(_) => CredentialStoreError::NotFound(key.to_string()),
            other => CredentialStoreError::Storage(other),
        })?;

        stream
            .flat_map(|stream_item| match stream_item {
                Stream::Next(result) => vec![result],
                _ => vec![],
            })
            .next()
            .ok_or_else(|| CredentialStoreError::NotFound(key.to_string()))?
            .map_err(CredentialStoreError::Storage)
    }

    fn set<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError> {
        let stream = self
            .storage
            .set(key, value)
            .map_err(CredentialStoreError::Storage)?;

        stream
            .flat_map(|stream_item| match stream_item {
                Stream::Next(result) => vec![result],
                _ => vec![],
            })
            .next()
            .ok_or_else(|| {
                CredentialStoreError::Generic("Stream ended without result".to_string())
            })??;
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), CredentialStoreError> {
        let stream = self
            .storage
            .delete(key)
            .map_err(CredentialStoreError::Storage)?;

        stream
            .flat_map(|stream_item| match stream_item {
                Stream::Next(result) => vec![result],
                _ => vec![],
            })
            .next()
            .ok_or_else(|| {
                CredentialStoreError::Generic("Stream ended without result".to_string())
            })??;
        Ok(())
    }

    fn exists(&self, key: &str) -> Result<bool, CredentialStoreError> {
        let stream = self
            .storage
            .exists(key)
            .map_err(CredentialStoreError::Storage)?;

        stream
            .flat_map(|stream_item| match stream_item {
                Stream::Next(result) => vec![result],
                _ => vec![],
            })
            .next()
            .ok_or_else(|| {
                CredentialStoreError::Generic("Stream ended without result".to_string())
            })?
            .map_err(CredentialStoreError::Storage)
    }

    fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError> {
        let stream = self
            .storage
            .list_keys(prefix)
            .map_err(CredentialStoreError::Storage)?;

        stream
            .flat_map(|stream_item| match stream_item {
                Stream::Next(Ok(result)) => vec![Ok(result)],
                Stream::Next(Err(e)) => vec![Err(CredentialStoreError::Storage(e))],
                _ => vec![],
            })
            .collect::<Result<Vec<_>, _>>()
    }
}

/// Helper methods for OAuth token storage.
pub trait OAuthTokenStore: CredentialStore {
    /// Store OAuth tokens for a provider.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError`] if the storage operation fails.
    fn store_oauth_token(
        &self,
        provider: &str,
        token: &OAuthToken,
    ) -> Result<(), CredentialStoreError> {
        let key = format!("oauth:token:{provider}");
        self.set(&key, token.clone())
    }

    /// Get OAuth tokens for a provider.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError`] if the storage operation or
    /// deserialization fails.
    fn get_oauth_token(&self, provider: &str) -> Result<Option<OAuthToken>, CredentialStoreError> {
        let key = format!("oauth:token:{provider}");
        self.get(&key)
    }

    /// Delete OAuth tokens for a provider.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError`] if the storage operation fails.
    fn delete_oauth_token(&self, provider: &str) -> Result<(), CredentialStoreError> {
        let key = format!("oauth:token:{provider}");
        self.delete(&key)
    }

    /// Store OAuth state (for PKCE flow).
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError`] if the storage operation fails.
    fn store_oauth_state(
        &self,
        state: &str,
        code_verifier: &str,
        expires_at: i64,
    ) -> Result<(), CredentialStoreError> {
        let key = format!("oauth:state:{state}");
        let value = OAuthState {
            code_verifier: code_verifier.to_string(),
            expires_at,
        };
        self.set(&key, value)
    }

    /// Get and validate OAuth state.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError`] if the storage operation or
    /// deserialization fails.
    fn get_oauth_state(&self, state: &str) -> Result<Option<OAuthState>, CredentialStoreError> {
        let key = format!("oauth:state:{state}");
        self.get(&key)
    }

    /// Delete OAuth state.
    ///
    /// # Errors
    ///
    /// Returns [`CredentialStoreError`] if the storage operation fails.
    fn delete_oauth_state(&self, state: &str) -> Result<(), CredentialStoreError> {
        let key = format!("oauth:state:{state}");
        self.delete(&key)
    }
}

/// OAuth state for PKCE flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthState {
    /// The PKCE code verifier.
    pub code_verifier: String,
    /// When the state expires (Unix timestamp).
    pub expires_at: i64,
}

impl OAuthState {
    /// Check if the state is expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now >= self.expires_at
    }
}

// Blanket impl: any CredentialStore picks up OAuth helpers for free.
impl<T: CredentialStore> OAuthTokenStore for T {}

/// Credential wrapper for secure storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredential<T> {
    /// The credential data.
    pub data: T,
    /// When the credential was created.
    pub created_at: i64,
    /// When the credential was last accessed.
    pub last_accessed_at: Option<i64>,
    /// Metadata (encrypted if sensitive).
    pub metadata: Option<serde_json::Value>,
}

impl<T> StoredCredential<T> {
    /// Create a new stored credential.
    #[must_use]
    pub fn new(data: T) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            data,
            created_at: now,
            last_accessed_at: None,
            metadata: None,
        }
    }

    /// Mark the credential as accessed.
    pub fn mark_accessed(&mut self) {
        self.last_accessed_at = Some(chrono::Utc::now().timestamp());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Initialise the Valtron single-thread executor for the current test
    /// worker thread. The executor lives in a `thread_local!` `OnceCell`,
    /// so every test thread needs its own call. `initialize_pool` is
    /// idempotent per thread (internally `get_or_init`), so repeat calls
    /// are safe.
    fn init_valtron() {
        foundation_core::valtron::single::initialize_pool(42);
    }

    /// Build a fresh [`CredentialStorage`] backed by a temporary `SQLite`
    /// file via the Turso backend. Local tests exercise the real SQL path
    /// instead of the in-memory shim so migrations, streaming, and
    /// serialization are all validated end-to-end.
    fn sqlite_store() -> (CredentialStorage, tempfile::TempDir) {
        init_valtron();
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("auth.sqlite");
        let store = CredentialStorage::turso(
            db_path.to_str().expect("non-utf8 tempdir path"),
        )
        .expect("init turso credential storage");
        (store, dir)
    }

    #[test]
    fn sqlite_store_basic_roundtrip() {
        let (store, _dir) = sqlite_store();

        store
            .set("test_key", "test_value")
            .expect("Failed to set credential");

        let value: String = store
            .get("test_key")
            .expect("Failed to get credential")
            .expect("Credential not found");
        assert_eq!(value, "test_value");

        assert!(store.exists("test_key").expect("Failed to check existence"));
        assert!(!store
            .exists("nonexistent")
            .expect("Failed to check existence"));

        store
            .delete("test_key")
            .expect("Failed to delete credential");
        assert!(!store.exists("test_key").expect("Failed to check existence"));
    }

    #[test]
    fn sqlite_store_oauth_helpers() {
        let (store, _dir) = sqlite_store();

        let token = OAuthToken {
            access_token: "access_123".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: Some("refresh_456".to_string()),
            scope: Some("openid profile".to_string()),
            id_token: None,
        };

        store
            .store_oauth_token("test_provider", &token)
            .expect("Failed to store OAuth token");

        let retrieved = store
            .get_oauth_token("test_provider")
            .expect("Failed to get OAuth token")
            .expect("OAuth token not found");
        assert_eq!(retrieved.access_token, "access_123");
        assert_eq!(retrieved.refresh_token, Some("refresh_456".to_string()));

        store
            .store_oauth_state("test_state", "verifier_abc", 9_999_999_999)
            .expect("Failed to store OAuth state");

        let state = store
            .get_oauth_state("test_state")
            .expect("Failed to get OAuth state")
            .expect("OAuth state not found");
        assert_eq!(state.code_verifier, "verifier_abc");
        assert!(!state.is_expired());
    }

    #[test]
    fn sqlite_store_list_keys_filters_by_prefix() {
        let (store, _dir) = sqlite_store();

        store.set("oauth:provider1", "value1").unwrap();
        store.set("oauth:provider2", "value2").unwrap();
        store.set("jwt:token", "value3").unwrap();

        let keys = store.list_keys(None).unwrap();
        assert_eq!(keys.len(), 3);

        let keys = store.list_keys(Some("oauth:")).unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"oauth:provider1".to_string()));
        assert!(keys.contains(&"oauth:provider2".to_string()));
    }

    #[test]
    fn stored_credential_tracks_access() {
        let mut stored = StoredCredential::new("secret_data");
        assert!(stored.created_at > 0);
        assert!(stored.last_accessed_at.is_none());

        stored.mark_accessed();
        assert!(stored.last_accessed_at.is_some());
    }

    #[test]
    fn credential_storage_wraps_arbitrary_provider() {
        init_valtron();
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("wrap.sqlite");
        let provider = StorageProvider::new(StorageBackend::Turso {
            url: db_path.to_string_lossy().into_owned(),
        })
        .expect("init turso provider");
        let store = CredentialStorage::new(provider);
        store.set("k", "v").unwrap();
        let got: String = store.get("k").unwrap().unwrap();
        assert_eq!(got, "v");
    }
}

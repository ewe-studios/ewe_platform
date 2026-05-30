//! Credential storage wrapping `foundation_db::StorageProvider`.
//!
//! `foundation_db` already selects the backend (Turso, libsql, D1, R2, JSON
//! file, in-memory) through `StorageBackend`. There is no reason to expose
//! one credential-store wrapper per backend — the wrapper only needs to
//! drain Valtron streams into sync `Result`s. A single [`CredentialStorage`]
//! therefore covers every backend `foundation_db` supports.

use foundation_core::valtron::Stream;
#[cfg(feature = "turso")]
use foundation_db::StorageBackend;
use foundation_db::{core::AsyncKeyValueStore, KeyValueStore, StorageError, StorageProvider};
use serde::{Deserialize, Serialize};

use crate::shared::oauth_token::OAuthToken;

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

/// Asynchronous credential storage API.
///
/// Implementations use native async/await for I/O (e.g. D1 Promises,
/// HTTP requests). Specific impls exist for each storage type to avoid
/// orphan/coherence conflicts.
#[async_trait::async_trait(?Send)]
pub trait AsyncCredentialStore: Sync {
    /// Get a credential by key.
    async fn get_async<V: for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError>;

    /// Set a credential.
    async fn set_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError>;

    /// Delete a credential.
    async fn delete_async(&self, key: &str) -> Result<(), CredentialStoreError>;

    /// Check whether a credential exists.
    async fn exists_async(&self, key: &str) -> Result<bool, CredentialStoreError>;

    /// List credential keys, optionally filtered by prefix.
    async fn list_keys_async(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError>;
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
    #[cfg(feature = "turso")]
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

#[async_trait::async_trait(?Send)]
impl AsyncCredentialStore for CredentialStorage {
    async fn get_async<V: for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError> {
        self.storage.get_async(key)
            .await
            .map_err(|e| match e {
                StorageError::NotFound(_) => CredentialStoreError::NotFound(key.to_string()),
                other => CredentialStoreError::Storage(other),
            })
    }

    async fn set_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError> {
        self.storage.set_async(key, value)
            .await
            .map_err(CredentialStoreError::Storage)
    }

    async fn delete_async(&self, key: &str) -> Result<(), CredentialStoreError> {
        self.storage.delete_async(key)
            .await
            .map_err(CredentialStoreError::Storage)
    }

    async fn exists_async(&self, key: &str) -> Result<bool, CredentialStoreError> {
        self.storage.exists_async(key)
            .await
            .map_err(CredentialStoreError::Storage)
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError> {
        self.storage.list_keys_async(prefix)
            .await
            .map_err(CredentialStoreError::Storage)
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

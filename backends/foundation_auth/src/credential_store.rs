//! Credential storage module wrapping `foundation_db`.

use foundation_core::valtron::Stream;
use foundation_db::{KeyValueStore, StorageBackend, StorageProvider, StorageError};
use serde::{Deserialize, Serialize};

use crate::oauth::OAuthToken;

/// Credential store trait for persistent credential storage.
pub trait CredentialStore: Send + Sync {
    /// Get a credential by key.
    ///
    /// # Errors
    ///
    /// Returns a `CredentialStoreError` if the storage operation fails or if deserialization fails.
    fn get<V: for<'de> Deserialize<'de> + Send + 'static>(&self, key: &str) -> Result<Option<V>, CredentialStoreError>;

    /// Set a credential.
    ///
    /// # Errors
    ///
    /// Returns a `CredentialStoreError` if the storage operation fails or if serialization fails.
    fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> Result<(), CredentialStoreError>;

    /// Delete a credential.
    ///
    /// # Errors
    ///
    /// Returns a `CredentialStoreError` if the storage operation fails.
    fn delete(&self, key: &str) -> Result<(), CredentialStoreError>;

    /// Check if a credential exists.
    ///
    /// # Errors
    ///
    /// Returns a `CredentialStoreError` if the storage operation fails.
    fn exists(&self, key: &str) -> Result<bool, CredentialStoreError>;

    /// List credentials with optional prefix.
    ///
    /// # Errors
    ///
    /// Returns a `CredentialStoreError` if the storage operation fails.
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

/// Turso-backed credential store.
pub struct TursoCredentialStore {
    storage: StorageProvider,
}

impl TursoCredentialStore {
    /// Create a new Turso credential store.
    ///
    /// # Errors
    ///
    /// Returns a `CredentialStoreError` if the storage backend initialization fails.
    pub fn new(url: &str) -> Result<Self, CredentialStoreError> {
        let storage = StorageProvider::new(StorageBackend::Turso {
            url: url.to_string(),
        })?;

        Ok(Self { storage })
    }

    /// Create from an existing [`StorageProvider`].
    #[must_use]
    pub fn from_storage(storage: StorageProvider) -> Self {
        Self { storage }
    }

    /// Initialize the database schema.
    ///
    /// # Errors
    ///
    /// Returns a `CredentialStoreError` if schema initialization fails.
    pub fn init_schema(&self) -> Result<(), CredentialStoreError> {
        // The Turso backend already creates the kv_store table
        // Additional auth-specific tables can be added via migrations
        Ok(())
    }
}

impl CredentialStore for TursoCredentialStore {
    fn get<V: for<'de> Deserialize<'de> + Send + 'static>(&self, key: &str) -> Result<Option<V>, CredentialStoreError> {
        let stream = self.storage.get(key).map_err(|e| match e {
            StorageError::NotFound(_) => CredentialStoreError::NotFound(key.to_string()),
            _ => CredentialStoreError::Storage(e),
        })?;

        // Consume the stream to get the value
        stream
            .flat_map(|stream_item| match stream_item {
                Stream::Next(result) => vec![result],
                _ => vec![],
            })
            .next()
            .ok_or_else(|| CredentialStoreError::NotFound(key.to_string()))?
            .map_err(CredentialStoreError::Storage)
    }

    fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> Result<(), CredentialStoreError> {
        let stream = self.storage.set(key, value).map_err(CredentialStoreError::Storage)?;

        stream
            .flat_map(|stream_item| match stream_item {
                Stream::Next(result) => vec![result],
                _ => vec![],
            })
            .next()
            .ok_or_else(|| CredentialStoreError::Generic("Stream ended without result".to_string()))??;
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), CredentialStoreError> {
        let stream = self.storage.delete(key).map_err(CredentialStoreError::Storage)?;

        stream
            .flat_map(|stream_item| match stream_item {
                Stream::Next(result) => vec![result],
                _ => vec![],
            })
            .next()
            .ok_or_else(|| CredentialStoreError::Generic("Stream ended without result".to_string()))??;
        Ok(())
    }

    fn exists(&self, key: &str) -> Result<bool, CredentialStoreError> {
        let stream = self.storage.exists(key).map_err(CredentialStoreError::Storage)?;

        stream
            .flat_map(|stream_item| match stream_item {
                Stream::Next(result) => vec![result],
                _ => vec![],
            })
            .next()
            .ok_or_else(|| CredentialStoreError::Generic("Stream ended without result".to_string()))?
            .map_err(CredentialStoreError::Storage)
    }

    fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError> {
        let stream = self.storage.list_keys(prefix).map_err(CredentialStoreError::Storage)?;

        stream
            .flat_map(|stream_item| match stream_item {
                Stream::Next(Ok(result)) => vec![Ok(result)],
                Stream::Next(Err(e)) => vec![Err(CredentialStoreError::Storage(e))],
                _ => vec![],
            })
            .collect::<Result<Vec<_>, _>>()
    }
}

/// In-memory credential store for development/testing.
pub struct MemoryCredentialStore {
    storage: StorageProvider,
}

impl MemoryCredentialStore {
    /// Create a new in-memory credential store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage: StorageProvider::memory(),
        }
    }
}

impl Default for MemoryCredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for MemoryCredentialStore {
    fn get<V: for<'de> Deserialize<'de> + Send + 'static>(&self, key: &str) -> Result<Option<V>, CredentialStoreError> {
        let stream = self.storage.get(key).map_err(|e| match e {
            StorageError::NotFound(_) => CredentialStoreError::NotFound(key.to_string()),
            _ => CredentialStoreError::Storage(e),
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

    fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> Result<(), CredentialStoreError> {
        let stream = self.storage.set(key, value).map_err(CredentialStoreError::Storage)?;

        stream
            .flat_map(|stream_item| match stream_item {
                Stream::Next(result) => vec![result],
                _ => vec![],
            })
            .next()
            .ok_or_else(|| CredentialStoreError::Generic("Stream ended without result".to_string()))??;
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), CredentialStoreError> {
        let stream = self.storage.delete(key).map_err(CredentialStoreError::Storage)?;

        stream
            .flat_map(|stream_item| match stream_item {
                Stream::Next(result) => vec![result],
                _ => vec![],
            })
            .next()
            .ok_or_else(|| CredentialStoreError::Generic("Stream ended without result".to_string()))??;
        Ok(())
    }

    fn exists(&self, key: &str) -> Result<bool, CredentialStoreError> {
        let stream = self.storage.exists(key).map_err(CredentialStoreError::Storage)?;

        stream
            .flat_map(|stream_item| match stream_item {
                Stream::Next(result) => vec![result],
                _ => vec![],
            })
            .next()
            .ok_or_else(|| CredentialStoreError::Generic("Stream ended without result".to_string()))?
            .map_err(CredentialStoreError::Storage)
    }

    fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError> {
        let stream = self.storage.list_keys(prefix).map_err(CredentialStoreError::Storage)?;

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
    /// Returns a `CredentialStoreError` if the storage operation fails.
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
    /// Returns a `CredentialStoreError` if the storage operation fails or if deserialization fails.
    fn get_oauth_token(&self, provider: &str) -> Result<Option<OAuthToken>, CredentialStoreError> {
        let key = format!("oauth:token:{provider}");
        self.get(&key)
    }

    /// Delete OAuth tokens for a provider.
    ///
    /// # Errors
    ///
    /// Returns a `CredentialStoreError` if the storage operation fails.
    fn delete_oauth_token(&self, provider: &str) -> Result<(), CredentialStoreError> {
        let key = format!("oauth:token:{provider}");
        self.delete(&key)
    }

    /// Store OAuth state (for PKCE flow).
    ///
    /// # Errors
    ///
    /// Returns a `CredentialStoreError` if the storage operation fails.
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
    /// Returns a `CredentialStoreError` if the storage operation fails or if deserialization fails.
    fn get_oauth_state(&self, state: &str) -> Result<Option<OAuthState>, CredentialStoreError> {
        let key = format!("oauth:state:{state}");
        self.get(&key)
    }

    /// Delete OAuth state.
    ///
    /// # Errors
    ///
    /// Returns a `CredentialStoreError` if the storage operation fails.
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

// Implement OAuthTokenStore for all types that implement CredentialStore
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

    #[test]
    fn test_memory_credential_store_basic() {
        let store = MemoryCredentialStore::new();

        // Test set and get
        store.set("test_key", "test_value").expect("Failed to set credential");

        let value: String = store
            .get("test_key")
            .expect("Failed to get credential")
            .expect("Credential not found");

        assert_eq!(value, "test_value");

        // Test exists
        assert!(store
            .exists("test_key")
            .expect("Failed to check existence"));
        assert!(!store
            .exists("nonexistent")
            .expect("Failed to check existence"));

        // Test delete
        store
            .delete("test_key")
            .expect("Failed to delete credential");
        assert!(!store
            .exists("test_key")
            .expect("Failed to check existence"));
    }

    #[test]
    fn test_memory_credential_store_oauth() {
        let store = MemoryCredentialStore::new();

        let token = OAuthToken {
            access_token: "access_123".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: Some(3600),
            refresh_token: Some("refresh_456".to_string()),
            scope: Some("openid profile".to_string()),
            id_token: None,
        };

        // Store OAuth token
        store
            .store_oauth_token("test_provider", &token)
            .expect("Failed to store OAuth token");

        // Retrieve OAuth token
        let retrieved = store
            .get_oauth_token("test_provider")
            .expect("Failed to get OAuth token")
            .expect("OAuth token not found");

        assert_eq!(retrieved.access_token, "access_123");
        assert_eq!(retrieved.refresh_token, Some("refresh_456".to_string()));

        // Store and retrieve OAuth state
        store
            .store_oauth_state("test_state", "verifier_abc", 9999999999)
            .expect("Failed to store OAuth state");

        let state = store
            .get_oauth_state("test_state")
            .expect("Failed to get OAuth state")
            .expect("OAuth state not found");

        assert_eq!(state.code_verifier, "verifier_abc");
        assert!(!state.is_expired());
    }

    #[test]
    fn test_memory_credential_store_list_keys() {
        let store = MemoryCredentialStore::new();

        store.set("oauth:provider1", "value1").unwrap();
        store.set("oauth:provider2", "value2").unwrap();
        store.set("jwt:token", "value3").unwrap();

        // List all keys
        let keys = store.list_keys(None).unwrap();
        assert_eq!(keys.len(), 3);

        // List keys with prefix
        let keys = store.list_keys(Some("oauth:")).unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"oauth:provider1".to_string()));
        assert!(keys.contains(&"oauth:provider2".to_string()));
    }

    #[test]
    fn test_stored_credential() {
        let mut stored = StoredCredential::new("secret_data");

        assert!(stored.created_at > 0);
        assert!(stored.last_accessed_at.is_none());

        stored.mark_accessed();
        assert!(stored.last_accessed_at.is_some());
    }
}

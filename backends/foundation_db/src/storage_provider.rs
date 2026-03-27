//! Core storage provider traits and abstractions.

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

use crate::backends::{MemoryStorage, TursoStorage};
use crate::errors::StorageResult;

/// Key-value store operations available on all backends.
#[async_trait]
pub trait KeyValueStore: Send + Sync {
    /// Get a value by key.
    async fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>>;

    /// Set a key-value pair.
    async fn set<V: Serialize + Send>(&self, key: &str, value: V) -> StorageResult<()>;

    /// Delete a key.
    async fn delete(&self, key: &str) -> StorageResult<()>;

    /// Check if a key exists.
    async fn exists(&self, key: &str) -> StorageResult<bool>;

    /// List all keys with optional prefix filter.
    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<Vec<String>>;
}

/// Blob storage operations for binary large objects.
#[async_trait]
pub trait BlobStore: Send + Sync {
    /// Put a blob into storage.
    async fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()>;

    /// Get a blob from storage.
    async fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>>;

    /// Delete a blob.
    async fn delete_blob(&self, key: &str) -> StorageResult<()>;

    /// Check if a blob exists.
    async fn blob_exists(&self, key: &str) -> StorageResult<bool>;
}

/// SQL query operations for relational backends (Turso, D1).
#[async_trait]
pub trait QueryStore: Send + Sync {
    /// Execute a query that returns rows.
    async fn query(
        &self,
        sql: &str,
        params: Vec<libsql::Value>,
    ) -> StorageResult<Vec<libsql::Row>>;

    /// Execute a statement that returns number of rows affected.
    async fn execute(&self, sql: &str, params: Vec<libsql::Value>) -> StorageResult<u64>;
}

/// Rate limiting operations.
#[async_trait]
pub trait RateLimiterStore: Send + Sync {
    /// Check if a rate limit key is allowed.
    async fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool>;

    /// Record a rate-limited action.
    async fn record_rate_limit(&self, key: &str) -> StorageResult<u32>;

    /// Reset a rate limit key.
    async fn reset_rate_limit(&self, key: &str) -> StorageResult<()>;
}

/// Storage backend enumeration for runtime selection.
#[derive(Debug, Clone)]
pub enum StorageBackend {
    /// Turso/libsql backend with database URL.
    Turso { url: String },
    /// Cloudflare D1 backend.
    D1,
    /// Cloudflare R2 backend with bucket configuration.
    R2 { bucket: String },
    /// In-memory backend for development/testing.
    Memory,
}

/// Unified storage provider that wraps all backends.
pub struct StorageProvider {
    inner: StorageProviderInner,
}

enum StorageProviderInner {
    Turso(Box<TursoStorage>),
    D1,      // TODO: Implement D1 backend
    R2,      // TODO: Implement R2 backend
    Memory(MemoryStorage),
}

impl StorageProvider {
    /// Create a new storage provider with the specified backend.
    pub async fn new(backend: StorageBackend) -> StorageResult<Self> {
        match backend {
            StorageBackend::Turso { url } => {
                let storage = TursoStorage::new(&url).await?;
                Ok(Self {
                    inner: StorageProviderInner::Turso(Box::new(storage)),
                })
            }
            StorageBackend::D1 => {
                // TODO: Implement D1 backend
                Err(crate::errors::StorageError::Generic(
                    "D1 backend not yet implemented".to_string(),
                ))
            }
            StorageBackend::R2 { .. } => {
                // TODO: Implement R2 backend
                Err(crate::errors::StorageError::Generic(
                    "R2 backend not yet implemented".to_string(),
                ))
            }
            StorageBackend::Memory => {
                let storage = MemoryStorage::new();
                Ok(Self {
                    inner: StorageProviderInner::Memory(storage),
                })
            }
        }
    }

    /// Create an in-memory storage provider (useful for testing).
    #[must_use]
    pub fn memory() -> Self {
        Self {
            inner: StorageProviderInner::Memory(MemoryStorage::new()),
        }
    }
}

#[async_trait]
impl KeyValueStore for StorageProvider {
    async fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
        match &self.inner {
            StorageProviderInner::Turso(storage) => storage.as_ref().get(key).await,
            StorageProviderInner::Memory(storage) => storage.get(key).await,
            _ => Err(crate::errors::StorageError::Generic(
                "Backend not implemented".to_string(),
            )),
        }
    }

    async fn set<V: Serialize + Send>(&self, key: &str, value: V) -> StorageResult<()> {
        match &self.inner {
            StorageProviderInner::Turso(storage) => storage.as_ref().set(key, value).await,
            StorageProviderInner::Memory(storage) => storage.set(key, value).await,
            _ => Err(crate::errors::StorageError::Generic(
                "Backend not implemented".to_string(),
            )),
        }
    }

    async fn delete(&self, key: &str) -> StorageResult<()> {
        match &self.inner {
            StorageProviderInner::Turso(storage) => storage.as_ref().delete(key).await,
            StorageProviderInner::Memory(storage) => storage.delete(key).await,
            _ => Err(crate::errors::StorageError::Generic(
                "Backend not implemented".to_string(),
            )),
        }
    }

    async fn exists(&self, key: &str) -> StorageResult<bool> {
        match &self.inner {
            StorageProviderInner::Turso(storage) => storage.as_ref().exists(key).await,
            StorageProviderInner::Memory(storage) => storage.exists(key).await,
            _ => Err(crate::errors::StorageError::Generic(
                "Backend not implemented".to_string(),
            )),
        }
    }

    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<Vec<String>> {
        match &self.inner {
            StorageProviderInner::Turso(storage) => storage.as_ref().list_keys(prefix).await,
            StorageProviderInner::Memory(storage) => storage.list_keys(prefix).await,
            _ => Err(crate::errors::StorageError::Generic(
                "Backend not implemented".to_string(),
            )),
        }
    }
}

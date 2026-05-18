//! StorageProvider — unified runtime backend selector (native only).

use serde::{de::DeserializeOwned, Serialize};

#[cfg(feature = "d1")]
use crate::core::backends::d1_kvstore::D1KeyValueStore;
use super::json_file::JsonFileStorage;
use crate::core::backends::memory::MemoryStorage;
#[cfg(feature = "r2")]
use crate::core::backends::r2_blobstore::R2BlobStore;
#[cfg(feature = "turso")]
use super::turso_backend::TursoStorage;
#[cfg(feature = "libsql")]
use super::libsql_backend::LibsqlStorage;
use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{
    BlobStore, DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};

/// Storage backend enumeration for runtime selection.
#[derive(Debug, Clone)]
pub enum StorageBackend {
    /// Turso backend with database URL.
    #[cfg(feature = "turso")]
    Turso { url: String },
    /// libsql backend with database URL.
    #[cfg(feature = "libsql")]
    Libsql { url: String },
    /// Cloudflare D1 backend.
    #[cfg(feature = "d1")]
    D1,
    /// Cloudflare R2 backend with bucket configuration.
    #[cfg(feature = "r2")]
    R2 { bucket: String },
    /// JSON file backend with file path.
    JsonFile { path: String },
    /// In-memory backend for development/testing.
    Memory,
}

/// Unified storage provider that wraps all backends.
pub struct StorageProvider {
    inner: StorageProviderInner,
}

enum StorageProviderInner {
    #[cfg(feature = "turso")]
    Turso(Box<TursoStorage>),
    #[cfg(feature = "libsql")]
    Libsql(Box<LibsqlStorage>),
    JsonFile(JsonFileStorage),
    Memory(MemoryStorage),
    #[cfg(feature = "d1")]
    D1(D1KeyValueStore),
    #[cfg(feature = "r2")]
    R2(R2BlobStore),
}

impl StorageProvider {
    /// Create a new storage provider with the specified backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend initialization fails.
    pub fn new(backend: StorageBackend) -> StorageResult<Self> {
        match backend {
            #[cfg(feature = "turso")]
            StorageBackend::Turso { url } => {
                let storage = TursoStorage::new(&url)?;
                storage.init_schema()?;
                Ok(Self {
                    inner: StorageProviderInner::Turso(Box::new(storage)),
                })
            }
            #[cfg(feature = "libsql")]
            StorageBackend::Libsql { url } => {
                let storage = LibsqlStorage::new(&url)?;
                storage.init_schema()?;
                Ok(Self {
                    inner: StorageProviderInner::Libsql(Box::new(storage)),
                })
            }
            #[cfg(feature = "d1")]
            StorageBackend::D1 => {
                let storage = D1KeyValueStore::from_env()?;
                Ok(Self {
                    inner: StorageProviderInner::D1(storage),
                })
            }
            #[cfg(feature = "r2")]
            StorageBackend::R2 { bucket } => {
                let storage = R2BlobStore::from_env()?;
                let storage = if bucket.is_empty() {
                    storage
                } else {
                    R2BlobStore::new(
                        &std::env::var("CLOUDFLARE_API_TOKEN").unwrap_or_default(),
                        &std::env::var("CLOUDFLARE_ACCOUNT_ID").unwrap_or_default(),
                        &bucket,
                        "blobs",
                    )
                };
                Ok(Self {
                    inner: StorageProviderInner::R2(storage),
                })
            }
            StorageBackend::JsonFile { path } => {
                let storage = JsonFileStorage::new(&path)?;
                Ok(Self {
                    inner: StorageProviderInner::JsonFile(storage),
                })
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

    /// Create a JSON file storage provider.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn json_file<P: AsRef<std::path::Path>>(path: P) -> StorageResult<Self> {
        let storage = JsonFileStorage::new(path)?;
        Ok(Self {
            inner: StorageProviderInner::JsonFile(storage),
        })
    }
}

impl KeyValueStore for StorageProvider {
    fn get<'a, V: DeserializeOwned + Send + 'static>(
        &'a self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'a, Option<V>>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.get(key),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.get(key),
            StorageProviderInner::JsonFile(storage) => storage.get(key),
            StorageProviderInner::Memory(storage) => storage.get(key),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.get(key),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
        }
    }

    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.set(key, value),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.set(key, value),
            StorageProviderInner::JsonFile(storage) => storage.set(key, value),
            StorageProviderInner::Memory(storage) => storage.set(key, value),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.set(key, value),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
        }
    }

    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.delete(key),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.delete(key),
            StorageProviderInner::JsonFile(storage) => storage.delete(key),
            StorageProviderInner::Memory(storage) => storage.delete(key),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.delete(key),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
        }
    }

    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.exists(key),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.exists(key),
            StorageProviderInner::JsonFile(storage) => storage.exists(key),
            StorageProviderInner::Memory(storage) => storage.exists(key),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.exists(key),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
        }
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.list_keys(prefix),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.list_keys(prefix),
            StorageProviderInner::JsonFile(storage) => storage.list_keys(prefix),
            StorageProviderInner::Memory(storage) => storage.list_keys(prefix),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.list_keys(prefix),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
        }
    }
}

impl QueryStore for StorageProvider {
    fn query(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.query(sql, params),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.query(sql, params),
            StorageProviderInner::JsonFile(storage) => storage.query(sql, params),
            StorageProviderInner::Memory(storage) => storage.query(sql, params),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.query(sql, params),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore - object storage is not SQL".to_string(),
            )),
        }
    }

    fn execute(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, u64>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.execute(sql, params),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.execute(sql, params),
            StorageProviderInner::JsonFile(storage) => storage.execute(sql, params),
            StorageProviderInner::Memory(storage) => storage.execute(sql, params),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.execute(sql, params),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore - object storage is not SQL".to_string(),
            )),
        }
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.execute_batch(sql),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.execute_batch(sql),
            StorageProviderInner::JsonFile(storage) => storage.execute_batch(sql),
            StorageProviderInner::Memory(storage) => storage.execute_batch(sql),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.execute_batch(sql),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore - object storage is not SQL".to_string(),
            )),
        }
    }
}

impl RateLimiterStore for StorageProvider {
    fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<StorageItemStream<'_, bool>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => {
                storage.check_rate_limit(key, max_count, window_seconds)
            }
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => {
                storage.check_rate_limit(key, max_count, window_seconds)
            }
            StorageProviderInner::JsonFile(storage) => {
                storage.check_rate_limit(key, max_count, window_seconds)
            }
            StorageProviderInner::Memory(storage) => {
                storage.check_rate_limit(key, max_count, window_seconds)
            }
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => {
                storage.check_rate_limit(key, max_count, window_seconds)
            }
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore - object storage is not suitable for rate limiting".to_string(),
            )),
        }
    }

    fn record_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, u32>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.record_rate_limit(key),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.record_rate_limit(key),
            StorageProviderInner::JsonFile(storage) => storage.record_rate_limit(key),
            StorageProviderInner::Memory(storage) => storage.record_rate_limit(key),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.record_rate_limit(key),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore - object storage is not suitable for rate limiting".to_string(),
            )),
        }
    }

    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.reset_rate_limit(key),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.reset_rate_limit(key),
            StorageProviderInner::JsonFile(storage) => storage.reset_rate_limit(key),
            StorageProviderInner::Memory(storage) => storage.reset_rate_limit(key),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.reset_rate_limit(key),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore - object storage is not suitable for rate limiting".to_string(),
            )),
        }
    }
}

impl BlobStore for StorageProvider {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<StorageItemStream<'_, ()>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.put_blob(key, data),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.put_blob(key, data),
            StorageProviderInner::JsonFile(storage) => storage.put_blob(key, data),
            StorageProviderInner::Memory(storage) => storage.put_blob(key, data),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.put_blob(key, data),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(storage) => storage.put_blob(key, data),
        }
    }

    fn get_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<Vec<u8>>>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.get_blob(key),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.get_blob(key),
            StorageProviderInner::JsonFile(storage) => storage.get_blob(key),
            StorageProviderInner::Memory(storage) => storage.get_blob(key),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.get_blob(key),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(storage) => storage.get_blob(key),
        }
    }

    fn delete_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.delete_blob(key),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.delete_blob(key),
            StorageProviderInner::JsonFile(storage) => storage.delete_blob(key),
            StorageProviderInner::Memory(storage) => storage.delete_blob(key),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.delete_blob(key),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(storage) => storage.delete_blob(key),
        }
    }

    fn blob_exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        match &self.inner {
            #[cfg(feature = "turso")]
            StorageProviderInner::Turso(storage) => storage.blob_exists(key),
            #[cfg(feature = "libsql")]
            StorageProviderInner::Libsql(storage) => storage.blob_exists(key),
            StorageProviderInner::JsonFile(storage) => storage.blob_exists(key),
            StorageProviderInner::Memory(storage) => storage.blob_exists(key),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.blob_exists(key),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(storage) => storage.blob_exists(key),
        }
    }
}

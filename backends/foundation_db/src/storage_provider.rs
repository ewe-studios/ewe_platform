//! StorageProvider — unified runtime backend selector (both native + wasm).
//!
//! This is the central export of `foundation_db`. It wraps all available
//! backends behind feature gates and provides a single `StorageResult` /
//! `StorageItemStream` interface.

use serde::{de::DeserializeOwned, Serialize};

// Core backends (shared — both targets)
use crate::core::backends::memory::MemoryStorage;
use crate::core::backends::memory_json::MemoryJsonStore;

#[cfg(feature = "d1")]
use crate::core::backends::d1_kvstore::D1KeyValueStore;

#[cfg(feature = "r2")]
use crate::core::backends::r2_blobstore::R2BlobStore;

// Native-only backends
#[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
use crate::native::turso_backend::TursoStorage;

#[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
use crate::native::libsql_backend::LibsqlStorage;

#[cfg(not(target_arch = "wasm32"))]
use crate::native::json_file::JsonFileStorage;

use crate::core::errors::StorageResult;
#[allow(unused_imports)] // used only in R2 match arms
use crate::core::errors::StorageError;
use crate::core::storage_provider::{
    BlobStore, DataValue, KeyValueStore, QueryStore, RateLimiterStore, SqlRow, StorageItemStream,
};

/// Storage backend enumeration for runtime selection.
#[derive(Debug, Clone)]
pub enum StorageBackend {
    /// Turso backend with database URL (native only).
    #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
    Turso { url: String },
    /// libsql backend with database URL (native only).
    #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
    Libsql { url: String },
    /// Cloudflare D1 backend (both targets).
    #[cfg(feature = "d1")]
    D1,
    /// Cloudflare R2 backend with bucket configuration (both targets).
    #[cfg(feature = "r2")]
    R2 { bucket: String },
    /// JSON file backend with file path (native only).
    #[cfg(not(target_arch = "wasm32"))]
    JsonFile { path: String },
    /// In-memory byte-buffer backend (both targets).
    Memory,
    /// In-memory JSON backend — values as JSON strings, inspectable (both targets).
    MemoryJson,
}

/// Unified storage provider that wraps all backends.
pub struct StorageProvider {
    inner: StorageProviderInner,
}

enum StorageProviderInner {
    #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
    Turso(Box<TursoStorage>),
    #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
    Libsql(Box<LibsqlStorage>),
    #[cfg(not(target_arch = "wasm32"))]
    JsonFile(JsonFileStorage),
    Memory(MemoryStorage),
    MemoryJson(MemoryJsonStore),
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
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageBackend::Turso { url } => {
                let storage = TursoStorage::new(&url)?;
                storage.init_schema()?;
                Ok(Self {
                    inner: StorageProviderInner::Turso(Box::new(storage)),
                })
            }
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
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
            #[cfg(not(target_arch = "wasm32"))]
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
            StorageBackend::MemoryJson => {
                let storage = MemoryJsonStore::new();
                Ok(Self {
                    inner: StorageProviderInner::MemoryJson(storage),
                })
            }
        }
    }

    /// Create an in-memory byte-buffer storage provider.
    #[must_use]
    pub fn memory() -> Self {
        Self {
            inner: StorageProviderInner::Memory(MemoryStorage::new()),
        }
    }

    /// Create an in-memory JSON storage provider (inspectable).
    #[must_use]
    pub fn memory_json() -> Self {
        Self {
            inner: StorageProviderInner::MemoryJson(MemoryJsonStore::new()),
        }
    }

    /// Create a JSON file storage provider (native only).
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    #[cfg(not(target_arch = "wasm32"))]
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
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.get(key),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.get(key),
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.get(key),
            StorageProviderInner::Memory(storage) => storage.get(key),
            StorageProviderInner::MemoryJson(storage) => storage.get(key),
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
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.set(key, value),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.set(key, value),
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.set(key, value),
            StorageProviderInner::Memory(storage) => storage.set(key, value),
            StorageProviderInner::MemoryJson(storage) => storage.set(key, value),
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
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.delete(key),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.delete(key),
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.delete(key),
            StorageProviderInner::Memory(storage) => storage.delete(key),
            StorageProviderInner::MemoryJson(storage) => storage.delete(key),
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
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.exists(key),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.exists(key),
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.exists(key),
            StorageProviderInner::Memory(storage) => storage.exists(key),
            StorageProviderInner::MemoryJson(storage) => storage.exists(key),
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
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.list_keys(prefix),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.list_keys(prefix),
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.list_keys(prefix),
            StorageProviderInner::Memory(storage) => storage.list_keys(prefix),
            StorageProviderInner::MemoryJson(storage) => storage.list_keys(prefix),
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
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.query(sql, params),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.query(sql, params),
            #[cfg(not(target_arch = "wasm32"))]
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.query(sql, params),
            StorageProviderInner::Memory(storage) => storage.query(sql, params),
            StorageProviderInner::MemoryJson(storage) => storage.query(sql, params),
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
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.execute(sql, params),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.execute(sql, params),
            #[cfg(not(target_arch = "wasm32"))]
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.execute(sql, params),
            StorageProviderInner::Memory(storage) => storage.execute(sql, params),
            StorageProviderInner::MemoryJson(storage) => storage.execute(sql, params),
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
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.execute_batch(sql),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.execute_batch(sql),
            #[cfg(not(target_arch = "wasm32"))]
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.execute_batch(sql),
            StorageProviderInner::Memory(storage) => storage.execute_batch(sql),
            StorageProviderInner::MemoryJson(storage) => storage.execute_batch(sql),
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
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => {
                storage.check_rate_limit(key, max_count, window_seconds)
            }
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => {
                storage.check_rate_limit(key, max_count, window_seconds)
            }
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => {
                storage.check_rate_limit(key, max_count, window_seconds)
            }
            StorageProviderInner::Memory(storage) => {
                storage.check_rate_limit(key, max_count, window_seconds)
            }
            StorageProviderInner::MemoryJson(storage) => {
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
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.record_rate_limit(key),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.record_rate_limit(key),
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.record_rate_limit(key),
            StorageProviderInner::Memory(storage) => storage.record_rate_limit(key),
            StorageProviderInner::MemoryJson(storage) => storage.record_rate_limit(key),
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
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.reset_rate_limit(key),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.reset_rate_limit(key),
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.reset_rate_limit(key),
            StorageProviderInner::Memory(storage) => storage.reset_rate_limit(key),
            StorageProviderInner::MemoryJson(storage) => storage.reset_rate_limit(key),
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
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.put_blob(key, data),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.put_blob(key, data),
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.put_blob(key, data),
            StorageProviderInner::Memory(storage) => storage.put_blob(key, data),
            StorageProviderInner::MemoryJson(storage) => storage.put_blob(key, data),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.put_blob(key, data),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(storage) => storage.put_blob(key, data),
        }
    }

    fn get_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<Vec<u8>>>> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.get_blob(key),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.get_blob(key),
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.get_blob(key),
            StorageProviderInner::Memory(storage) => storage.get_blob(key),
            StorageProviderInner::MemoryJson(storage) => storage.get_blob(key),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.get_blob(key),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(storage) => storage.get_blob(key),
        }
    }

    fn delete_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.delete_blob(key),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.delete_blob(key),
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.delete_blob(key),
            StorageProviderInner::Memory(storage) => storage.delete_blob(key),
            StorageProviderInner::MemoryJson(storage) => storage.delete_blob(key),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.delete_blob(key),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(storage) => storage.delete_blob(key),
        }
    }

    fn blob_exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.blob_exists(key),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.blob_exists(key),
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.blob_exists(key),
            StorageProviderInner::Memory(storage) => storage.blob_exists(key),
            StorageProviderInner::MemoryJson(storage) => storage.blob_exists(key),
            #[cfg(feature = "d1")]
            StorageProviderInner::D1(storage) => storage.blob_exists(key),
            #[cfg(feature = "r2")]
            StorageProviderInner::R2(storage) => storage.blob_exists(key),
        }
    }
}

//! StorageProvider — unified runtime backend selector (both native + wasm).
//!
//! This is the central export of `foundation_db`. It wraps all available
//! backends behind feature gates and provides a single `StorageResult` /
//! `StorageItemStream` interface.

use serde::{de::DeserializeOwned, Serialize};

// Core backends (shared — both targets)
use crate::core::backends::memory::MemoryStorage;
use crate::core::backends::memory_json::MemoryJsonStore;

#[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
use crate::native::d1_kvstore::D1Store;

#[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
use crate::native::r2_blobstore::R2Store;

// Native-only backends
#[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
use crate::native::turso_backend::TursoStorage;

#[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
use crate::native::libsql_backend::LibsqlStorage;

#[cfg(not(target_arch = "wasm32"))]
use crate::native::json_file::JsonFileStorage;

// Wasm-bindgen storage backends (wasm32 only)
#[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
use crate::wasm::wasm_storage::{D1WasmStorage, KVWasmStorage, R2WasmStorage};

use crate::core::errors::StorageResult;
#[allow(unused_imports)] // used only in R2 match arms
use crate::core::errors::StorageError;
use crate::core::storage_provider::{
    AsyncBlobStore, AsyncKeyValueStore, AsyncQueryStore, AsyncRateLimiterStore,
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
    #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
    D1,
    /// Cloudflare R2 backend with bucket configuration (native only).
    #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
    R2 { bucket: String },
    /// JSON file backend with file path (native only).
    #[cfg(not(target_arch = "wasm32"))]
    JsonFile { path: String },
    /// In-memory byte-buffer backend (both targets).
    Memory,
    /// In-memory JSON backend — values as JSON strings, inspectable (both targets).
    MemoryJson,
    /// D1 via wasm-bindgen — calls Cloudflare D1 JS API directly (wasm32 only).
    #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
    D1Wasm {
        db: std::sync::Arc<crate::wasm::bindgen::D1Database>,
        table_prefix: String,
    },
    /// R2 via wasm-bindgen — calls Cloudflare R2 JS API directly (wasm32 only).
    #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
    R2Wasm {
        bucket: crate::wasm::bindgen::R2Bucket,
        prefix: String,
    },
    /// KV via wasm-bindgen — calls Cloudflare Workers KV JS API directly (wasm32 only).
    #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
    KVWasm {
        kv: crate::wasm::bindgen::KVNamespace,
        prefix: String,
    },
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
    #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
    D1(D1Store),
    #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
    R2(R2Store),
    #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
    D1Wasm(D1WasmStorage),
    #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
    R2Wasm(R2WasmStorage),
    #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
    KVWasm(KVWasmStorage),
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
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageBackend::D1 => {
                let storage = D1Store::from_env()?;
                Ok(Self {
                    inner: StorageProviderInner::D1(storage),
                })
            }
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageBackend::R2 { bucket } => {
                let storage = if bucket.is_empty() {
                    R2Store::from_env()?
                } else {
                    R2Store::new_blob(
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
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageBackend::D1Wasm { db, table_prefix } => {
                let storage = D1WasmStorage::new(db, &table_prefix);
                // Init schema via valtron single-threaded executor.
                let storage_ref = storage.clone();
                let mut driven = foundation_core::valtron::drive_future(async move {
                    storage_ref.init_schema_async().await
                });
                let mut init_result: Option<Result<(), StorageError>> = None;
                for status in driven.by_ref() {
                    if let foundation_core::valtron::TaskStatus::Ready(v) = status {
                        init_result = Some(v.map_err(|e| StorageError::Backend(format!("Init failed: {e:?}"))));
                        break;
                    }
                }
                init_result.ok_or_else(|| StorageError::Generic("No result from init".into()))??;
                Ok(Self {
                    inner: StorageProviderInner::D1Wasm(storage),
                })
            }
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageBackend::R2Wasm { bucket, prefix } => {
                let storage = R2WasmStorage::new(bucket, &prefix);
                Ok(Self {
                    inner: StorageProviderInner::R2Wasm(storage),
                })
            }
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageBackend::KVWasm { kv, prefix } => {
                let storage = KVWasmStorage::new(kv, &prefix);
                Ok(Self {
                    inner: StorageProviderInner::KVWasm(storage),
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
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.get(key),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.get(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.get(key),
        }
    }

    fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.set(key, value),
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.set(key, value),
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.set(key, value),
            StorageProviderInner::Memory(storage) => storage.set(key, value),
            StorageProviderInner::MemoryJson(storage) => storage.set(key, value),
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.set(key, value),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.set(key, value),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.set(key, value),
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
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.delete(key),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.delete(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.delete(key),
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
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.exists(key),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.exists(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.exists(key),
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
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.list_keys(prefix),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.list_keys(prefix),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore - use BlobStore instead".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.list_keys(prefix),
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
            StorageProviderInner::JsonFile(storage) => storage.query(sql, params),
            StorageProviderInner::Memory(storage) => storage.query(sql, params),
            StorageProviderInner::MemoryJson(storage) => storage.query(sql, params),
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.query(sql, params),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore - object storage is not SQL".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.query(sql, params),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore - object storage is not SQL".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(_) => Err(StorageError::Generic(
                "KV does not support QueryStore - KV is not SQL".to_string(),
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
            StorageProviderInner::Memory(storage) => storage.execute(sql, params),
            StorageProviderInner::MemoryJson(storage) => storage.execute(sql, params),
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.execute(sql, params),
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.execute(sql, params),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore - object storage is not SQL".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.execute(sql, params),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore - object storage is not SQL".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(_) => Err(StorageError::Generic(
                "KV does not support QueryStore - KV is not SQL".to_string(),
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
            StorageProviderInner::JsonFile(storage) => storage.execute_batch(sql),
            StorageProviderInner::Memory(storage) => storage.execute_batch(sql),
            StorageProviderInner::MemoryJson(storage) => storage.execute_batch(sql),
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.execute_batch(sql),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore - object storage is not SQL".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.execute_batch(sql),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore - object storage is not SQL".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(_) => Err(StorageError::Generic(
                "KV does not support QueryStore - KV is not SQL".to_string(),
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
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => {
                storage.check_rate_limit(key, max_count, window_seconds)
            }
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore - object storage is not suitable for rate limiting".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => {
                storage.check_rate_limit(key, max_count, window_seconds)
            }
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore - object storage is not suitable for rate limiting".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => {
                storage.check_rate_limit(key, max_count, window_seconds)
            },
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
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.record_rate_limit(key),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore - object storage is not suitable for rate limiting".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.record_rate_limit(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore - object storage is not suitable for rate limiting".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.record_rate_limit(key),
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
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.reset_rate_limit(key),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore - object storage is not suitable for rate limiting".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.reset_rate_limit(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore - object storage is not suitable for rate limiting".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.reset_rate_limit(key),
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
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.put_blob(key, data),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(storage) => storage.put_blob(key, data),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.put_blob(key, data),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(storage) => storage.put_blob(key, data),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.put_blob(key, data),
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
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.get_blob(key),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(storage) => storage.get_blob(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.get_blob(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(storage) => storage.get_blob(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.get_blob(key),
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
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.delete_blob(key),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(storage) => storage.delete_blob(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.delete_blob(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(storage) => storage.delete_blob(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.delete_blob(key),
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
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.blob_exists(key),
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(storage) => storage.blob_exists(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.blob_exists(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(storage) => storage.blob_exists(key),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.blob_exists(key),
        }
    }
}

// ===========================================================================
// Async trait implementations — delegate to inner backend.
// ===========================================================================

#[async_trait::async_trait(?Send)]
impl AsyncKeyValueStore for StorageProvider {
    async fn get_async<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.get_async(key).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.get_async(key).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.get_async(key).await,
            StorageProviderInner::Memory(storage) => storage.get_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.get_async(key).await,
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.get_async(key).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.get_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.get_async(key).await,
        }
    }

    async fn set_async<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.set_async(key, value).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.set_async(key, value).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.set_async(key, value).await,
            StorageProviderInner::Memory(storage) => storage.set_async(key, value).await,
            StorageProviderInner::MemoryJson(storage) => storage.set_async(key, value).await,
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.set_async(key, value).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.set_async(key, value).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.set_async(key, value).await,
        }
    }

    async fn delete_async(&self, key: &str) -> StorageResult<()> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.delete_async(key).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.delete_async(key).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.delete_async(key).await,
            StorageProviderInner::Memory(storage) => storage.delete_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.delete_async(key).await,
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.delete_async(key).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.delete_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.delete_async(key).await,
        }
    }

    async fn exists_async(&self, key: &str) -> StorageResult<bool> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.exists_async(key).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.exists_async(key).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.exists_async(key).await,
            StorageProviderInner::Memory(storage) => storage.exists_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.exists_async(key).await,
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.exists_async(key).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.exists_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.exists_async(key).await,
        }
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<Vec<String>> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.list_keys_async(prefix).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.list_keys_async(prefix).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.list_keys_async(prefix).await,
            StorageProviderInner::Memory(storage) => storage.list_keys_async(prefix).await,
            StorageProviderInner::MemoryJson(storage) => storage.list_keys_async(prefix).await,
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.list_keys_async(prefix).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.list_keys_async(prefix).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.list_keys_async(prefix).await,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncQueryStore for StorageProvider {
    async fn query_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<Vec<SqlRow>> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.query_async(sql, params).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.query_async(sql, params).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(_) => Err(StorageError::Generic(
                "QueryStore not supported for JsonFileStorage".to_string(),
            )),
            StorageProviderInner::Memory(_) => Err(StorageError::Generic(
                "QueryStore not supported for MemoryStorage".to_string(),
            )),
            StorageProviderInner::MemoryJson(_) => Err(StorageError::Generic(
                "QueryStore not supported for MemoryJsonStore".to_string(),
            )),
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.query_async(sql, params).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.query_async(sql, params).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(_) => Err(StorageError::Generic(
                "KV does not support QueryStore".to_string(),
            )),
        }
    }

    async fn execute_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.execute_async(sql, params).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.execute_async(sql, params).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(_) => Err(StorageError::Generic(
                "QueryStore not supported for JsonFileStorage".to_string(),
            )),
            StorageProviderInner::Memory(_) => Err(StorageError::Generic(
                "QueryStore not supported for MemoryStorage".to_string(),
            )),
            StorageProviderInner::MemoryJson(_) => Err(StorageError::Generic(
                "QueryStore not supported for MemoryJsonStore".to_string(),
            )),
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.execute_async(sql, params).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.execute_async(sql, params).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(_) => Err(StorageError::Generic(
                "KV does not support QueryStore".to_string(),
            )),
        }
    }

    async fn execute_batch_async(&self, sql: &str) -> StorageResult<()> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.execute_batch_async(sql).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.execute_batch_async(sql).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(_) => Err(StorageError::Generic(
                "QueryStore not supported for JsonFileStorage".to_string(),
            )),
            StorageProviderInner::Memory(_) => Err(StorageError::Generic(
                "QueryStore not supported for MemoryStorage".to_string(),
            )),
            StorageProviderInner::MemoryJson(_) => Err(StorageError::Generic(
                "QueryStore not supported for MemoryJsonStore".to_string(),
            )),
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.execute_batch_async(sql).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.execute_batch_async(sql).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(_) => Err(StorageError::Generic(
                "KV does not support QueryStore".to_string(),
            )),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncRateLimiterStore for StorageProvider {
    async fn check_rate_limit_async(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(_) => Err(StorageError::Generic(
                "RateLimiterStore not supported for JsonFileStorage".to_string(),
            )),
            StorageProviderInner::Memory(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
            StorageProviderInner::MemoryJson(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
        }
    }

    async fn record_rate_limit_async(&self, key: &str) -> StorageResult<u32> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.record_rate_limit_async(key).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.record_rate_limit_async(key).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(_) => Err(StorageError::Generic(
                "RateLimiterStore not supported for JsonFileStorage".to_string(),
            )),
            StorageProviderInner::Memory(storage) => storage.record_rate_limit_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.record_rate_limit_async(key).await,
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.record_rate_limit_async(key).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.record_rate_limit_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.record_rate_limit_async(key).await,
        }
    }

    async fn reset_rate_limit_async(&self, key: &str) -> StorageResult<()> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.reset_rate_limit_async(key).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.reset_rate_limit_async(key).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(_) => Err(StorageError::Generic(
                "RateLimiterStore not supported for JsonFileStorage".to_string(),
            )),
            StorageProviderInner::Memory(storage) => storage.reset_rate_limit_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.reset_rate_limit_async(key).await,
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.reset_rate_limit_async(key).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.reset_rate_limit_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore".to_string(),
            )),
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.reset_rate_limit_async(key).await,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl AsyncBlobStore for StorageProvider {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.put_blob_async(key, data).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.put_blob_async(key, data).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.put_blob_async(key, data).await,
            StorageProviderInner::Memory(storage) => storage.put_blob_async(key, data).await,
            StorageProviderInner::MemoryJson(storage) => storage.put_blob_async(key, data).await,
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.put_blob_async(key, data).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(storage) => storage.put_blob_async(key, data).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.put_blob_async(key, data).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(storage) => storage.put_blob_async(key, data).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.put_blob_async(key, data).await,
        }
    }

    async fn get_blob_async(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.get_blob_async(key).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.get_blob_async(key).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.get_blob_async(key).await,
            StorageProviderInner::Memory(storage) => storage.get_blob_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.get_blob_async(key).await,
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.get_blob_async(key).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(storage) => storage.get_blob_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.get_blob_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(storage) => storage.get_blob_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.get_blob_async(key).await,
        }
    }

    async fn delete_blob_async(&self, key: &str) -> StorageResult<()> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.delete_blob_async(key).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.delete_blob_async(key).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.delete_blob_async(key).await,
            StorageProviderInner::Memory(storage) => storage.delete_blob_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.delete_blob_async(key).await,
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.delete_blob_async(key).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(storage) => storage.delete_blob_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.delete_blob_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(storage) => storage.delete_blob_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.delete_blob_async(key).await,
        }
    }

    async fn blob_exists_async(&self, key: &str) -> StorageResult<bool> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_arch = "wasm32")))]
            StorageProviderInner::Turso(storage) => storage.blob_exists_async(key).await,
            #[cfg(all(feature = "libsql", not(target_arch = "wasm32")))]
            StorageProviderInner::Libsql(storage) => storage.blob_exists_async(key).await,
            #[cfg(not(target_arch = "wasm32"))]
            StorageProviderInner::JsonFile(storage) => storage.blob_exists_async(key).await,
            StorageProviderInner::Memory(storage) => storage.blob_exists_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.blob_exists_async(key).await,
            #[cfg(all(feature = "d1", not(target_arch = "wasm32")))]
            StorageProviderInner::D1(storage) => storage.blob_exists_async(key).await,
            #[cfg(all(feature = "r2", not(target_arch = "wasm32")))]
            StorageProviderInner::R2(storage) => storage.blob_exists_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.blob_exists_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(storage) => storage.blob_exists_async(key).await,
            #[cfg(all(target_arch = "wasm32", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.blob_exists_async(key).await,
        }
    }
}

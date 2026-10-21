//! `StorageProvider` — unified runtime backend selector (both native + wasm).
//!
//! This is the central export of `foundation_db`. It wraps all available
//! backends behind feature gates and implements the `Async*` storage traits
//! over whichever one is selected. Sync callers wrap it in
//! `foundation_db::Blocking`.

use serde::{de::DeserializeOwned, Serialize};

// Core backends (shared — both targets)
use crate::core::backends::memory::MemoryStorage;
use crate::core::backends::memory_json::MemoryJsonStore;

#[cfg(all(feature = "d1", not(target_family = "wasm")))]
use crate::native::d1_kvstore::D1Store;

#[cfg(all(feature = "r2", not(target_family = "wasm")))]
use crate::native::r2_blobstore::R2Store;

// Native-only backends
#[cfg(all(feature = "turso", not(target_family = "wasm")))]
use crate::native::turso_backend::TursoStorage;

#[cfg(all(feature = "libsql", not(target_family = "wasm")))]
use crate::native::libsql_store::LibsqlStore;

#[cfg(not(target_family = "wasm"))]
use crate::native::json_file::JsonFileStorage;

// Wasm-bindgen storage backends (wasm32 only)
#[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
use crate::wasm::wasm_storage::{D1WasmStorage, KVWasmStorage, R2WasmStorage};

use crate::core::errors::StorageResult;
#[allow(unused_imports)] // used only in R2 match arms
use crate::core::errors::StorageError;
use crate::core::storage_provider::{
    AsyncBlobStore, AsyncKeyValueStore, AsyncListStream, AsyncQueryStream, AsyncQueryStore,
    AsyncRateLimiterStore, DataValue,
};

/// Storage backend enumeration for runtime selection.
#[derive(Debug, Clone)]
pub enum StorageBackend {
    /// Turso backend with database URL (native only).
    #[cfg(all(feature = "turso", not(target_family = "wasm")))]
    Turso { url: String },
    /// libsql backend with database URL (native only).
    #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
    Libsql { url: String },
    #[cfg(all(feature = "d1", not(target_family = "wasm")))]
    D1,
    /// Cloudflare R2 backend with bucket configuration (native only).
    #[cfg(all(feature = "r2", not(target_family = "wasm")))]
    R2 { bucket: String },
    /// JSON file backend with file path (native only).
    #[cfg(not(target_family = "wasm"))]
    JsonFile { path: String },
    /// In-memory byte-buffer backend (both targets).
    Memory,
    /// In-memory JSON backend — values as JSON strings, inspectable (both targets).
    MemoryJson,
    /// D1 via wasm-bindgen — calls Cloudflare D1 JS API directly (wasm32 only).
    #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
    D1Wasm {
        db: std::sync::Arc<crate::wasm::bindgen::D1Database>,
        table_prefix: String,
    },
    /// R2 via wasm-bindgen — calls Cloudflare R2 JS API directly (wasm32 only).
    #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
    R2Wasm {
        bucket: crate::wasm::bindgen::R2Bucket,
        prefix: String,
    },
    /// KV via wasm-bindgen — calls Cloudflare Workers KV JS API directly (wasm32 only).
    #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
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
    #[cfg(all(feature = "turso", not(target_family = "wasm")))]
    Turso(Box<TursoStorage>),
    #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
    Libsql(Box<LibsqlStore>),
    #[cfg(not(target_family = "wasm"))]
    JsonFile(JsonFileStorage),
    Memory(MemoryStorage),
    MemoryJson(MemoryJsonStore),
    #[cfg(all(feature = "d1", not(target_family = "wasm")))]
    D1(D1Store),
    #[cfg(all(feature = "r2", not(target_family = "wasm")))]
    R2(R2Store),
    #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
    D1Wasm(D1WasmStorage),
    #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
    R2Wasm(R2WasmStorage),
    #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
    KVWasm(KVWasmStorage),
}

/// The in-memory byte-buffer backend, same as [`StorageProvider::memory`].
impl Default for StorageProvider {
    fn default() -> Self {
        Self::memory()
    }
}

impl StorageProvider {
    /// Create a new storage provider with the specified backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend initialization fails.
    pub async fn new(backend: StorageBackend) -> StorageResult<Self> {
        match backend {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageBackend::Turso { url } => {
                let storage = TursoStorage::new(&url).await?;
                storage.init_schema().await?;
                Ok(Self {
                    inner: StorageProviderInner::Turso(Box::new(storage)),
                })
            }
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageBackend::Libsql { url } => {
                let storage = LibsqlStore::new_kv(&url, None).await?;
                storage.init_kv().await?;
                Ok(Self {
                    inner: StorageProviderInner::Libsql(Box::new(storage)),
                })
            }
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageBackend::D1 => {
                let storage = D1Store::from_env()?;
                Ok(Self {
                    inner: StorageProviderInner::D1(storage),
                })
            }
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
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
            #[cfg(not(target_family = "wasm"))]
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
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageBackend::D1Wasm { db, table_prefix } => {
                let storage = D1WasmStorage::new(db, &table_prefix);
                // Schema init is the caller's responsibility — the Workers
                // entry point drives `init_schema_async` with `.await`.
                Ok(Self {
                    inner: StorageProviderInner::D1Wasm(storage),
                })
            }
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageBackend::R2Wasm { bucket, prefix } => {
                let storage = R2WasmStorage::new(bucket, &prefix);
                Ok(Self {
                    inner: StorageProviderInner::R2Wasm(storage),
                })
            }
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
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
    #[cfg(not(target_family = "wasm"))]
    pub fn json_file<P: AsRef<std::path::Path>>(path: P) -> StorageResult<Self> {
        let storage = JsonFileStorage::new(path)?;
        Ok(Self {
            inner: StorageProviderInner::JsonFile(storage),
        })
    }
    /// Initialize the D1-backed key-value schema (creates the `kv_store`
    /// table if it does not exist). Must be called once before any
    /// key-value operations on wasm Workers, where the constructor can't
    /// block.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::Backend` if the D1 statement fails.
    #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
    pub async fn init_schema_async(&self) -> StorageResult<()> {
        match &self.inner {
            StorageProviderInner::D1Wasm(storage) => storage.init_schema_async().await,
            _ => Ok(()),
        }
    }
}

#[async_trait::async_trait]
impl AsyncKeyValueStore for StorageProvider {
    async fn get_async<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.get_async(key).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.get_async(key).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(storage) => storage.get_async(key).await,
            StorageProviderInner::Memory(storage) => storage.get_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.get_async(key).await,
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.get_async(key).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.get_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.get_async(key).await,
        }
    }

    async fn set_async<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.set_async(key, value).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.set_async(key, value).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(storage) => storage.set_async(key, value).await,
            StorageProviderInner::Memory(storage) => storage.set_async(key, value).await,
            StorageProviderInner::MemoryJson(storage) => storage.set_async(key, value).await,
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.set_async(key, value).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.set_async(key, value).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.set_async(key, value).await,
        }
    }

    async fn delete_async(&self, key: &str) -> StorageResult<()> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.delete_async(key).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.delete_async(key).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(storage) => storage.delete_async(key).await,
            StorageProviderInner::Memory(storage) => storage.delete_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.delete_async(key).await,
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.delete_async(key).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.delete_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.delete_async(key).await,
        }
    }

    async fn exists_async(&self, key: &str) -> StorageResult<bool> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.exists_async(key).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.exists_async(key).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(storage) => storage.exists_async(key).await,
            StorageProviderInner::Memory(storage) => storage.exists_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.exists_async(key).await,
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.exists_async(key).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.exists_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.exists_async(key).await,
        }
    }

    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.list_keys_async(prefix).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.list_keys_async(prefix).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(storage) => storage.list_keys_async(prefix).await,
            StorageProviderInner::Memory(storage) => storage.list_keys_async(prefix).await,
            StorageProviderInner::MemoryJson(storage) => storage.list_keys_async(prefix).await,
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.list_keys_async(prefix).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => {
                let keys = storage.list_keys_async(prefix).await?;
                Ok(AsyncListStream::new(futures_lite::stream::iter(keys.into_iter().map(Ok))))
            }
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support KeyValueStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => {
                let keys = storage.list_keys_async(prefix).await?;
                Ok(AsyncListStream::new(futures_lite::stream::iter(keys.into_iter().map(Ok))))
            }
        }
    }
}

#[async_trait::async_trait]
impl AsyncQueryStore for StorageProvider {
    async fn query_async(&self, _sql: &str, _params: &[DataValue]) -> StorageResult<AsyncQueryStream> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.query_async(_sql, _params).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.query_async(_sql, _params).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(_) => Err(StorageError::Generic(
                "QueryStore not supported for JsonFileStorage".to_string(),
            )),
            StorageProviderInner::Memory(_) => Err(StorageError::Generic(
                "QueryStore not supported for MemoryStorage".to_string(),
            )),
            StorageProviderInner::MemoryJson(_) => Err(StorageError::Generic(
                "QueryStore not supported for MemoryJsonStore".to_string(),
            )),
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.query_async(_sql, _params).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => {
                let rows = storage.query_async(_sql, _params).await?;
                Ok(AsyncQueryStream::new(futures_lite::stream::iter(rows.into_iter().map(Ok))))
            }
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(_) => Err(StorageError::Generic(
                "KV does not support QueryStore".to_string(),
            )),
        }
    }

    async fn execute_async(&self, _sql: &str, _params: &[DataValue]) -> StorageResult<u64> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.execute_async(_sql, _params).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.execute_async(_sql, _params).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(_) => Err(StorageError::Generic(
                "QueryStore not supported for JsonFileStorage".to_string(),
            )),
            StorageProviderInner::Memory(_) => Err(StorageError::Generic(
                "QueryStore not supported for MemoryStorage".to_string(),
            )),
            StorageProviderInner::MemoryJson(_) => Err(StorageError::Generic(
                "QueryStore not supported for MemoryJsonStore".to_string(),
            )),
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.execute_async(_sql, _params).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.execute_async(_sql, _params).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(_) => Err(StorageError::Generic(
                "KV does not support QueryStore".to_string(),
            )),
        }
    }

    async fn execute_batch_async(&self, _sql: &str) -> StorageResult<()> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.execute_batch_async(_sql).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.execute_batch_async(_sql).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(_) => Err(StorageError::Generic(
                "QueryStore not supported for JsonFileStorage".to_string(),
            )),
            StorageProviderInner::Memory(_) => Err(StorageError::Generic(
                "QueryStore not supported for MemoryStorage".to_string(),
            )),
            StorageProviderInner::MemoryJson(_) => Err(StorageError::Generic(
                "QueryStore not supported for MemoryJsonStore".to_string(),
            )),
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.execute_batch_async(_sql).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.execute_batch_async(_sql).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support QueryStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(_) => Err(StorageError::Generic(
                "KV does not support QueryStore".to_string(),
            )),
        }
    }
}

#[async_trait::async_trait]
impl AsyncRateLimiterStore for StorageProvider {
    async fn check_rate_limit_async(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(_) => Err(StorageError::Generic(
                "RateLimiterStore not supported for JsonFileStorage".to_string(),
            )),
            StorageProviderInner::Memory(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
            StorageProviderInner::MemoryJson(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => {
                storage.check_rate_limit_async(key, max_count, window_seconds).await
            }
        }
    }

    async fn record_rate_limit_async(&self, key: &str) -> StorageResult<u32> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.record_rate_limit_async(key).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.record_rate_limit_async(key).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(_) => Err(StorageError::Generic(
                "RateLimiterStore not supported for JsonFileStorage".to_string(),
            )),
            StorageProviderInner::Memory(storage) => storage.record_rate_limit_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.record_rate_limit_async(key).await,
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.record_rate_limit_async(key).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.record_rate_limit_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.record_rate_limit_async(key).await,
        }
    }

    async fn reset_rate_limit_async(&self, key: &str) -> StorageResult<()> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.reset_rate_limit_async(key).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.reset_rate_limit_async(key).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(_) => Err(StorageError::Generic(
                "RateLimiterStore not supported for JsonFileStorage".to_string(),
            )),
            StorageProviderInner::Memory(storage) => storage.reset_rate_limit_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.reset_rate_limit_async(key).await,
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.reset_rate_limit_async(key).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.reset_rate_limit_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(_) => Err(StorageError::Generic(
                "R2 does not support RateLimiterStore".to_string(),
            )),
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.reset_rate_limit_async(key).await,
        }
    }
}

#[async_trait::async_trait]
impl AsyncBlobStore for StorageProvider {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.put_blob_async(key, data).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.put_blob_async(key, data).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(storage) => storage.put_blob_async(key, data).await,
            StorageProviderInner::Memory(storage) => storage.put_blob_async(key, data).await,
            StorageProviderInner::MemoryJson(storage) => storage.put_blob_async(key, data).await,
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.put_blob_async(key, data).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(storage) => storage.put_blob_async(key, data).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.put_blob_async(key, data).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(storage) => storage.put_blob_async(key, data).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.put_blob_async(key, data).await,
        }
    }

    async fn get_blob_async(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.get_blob_async(key).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.get_blob_async(key).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(storage) => storage.get_blob_async(key).await,
            StorageProviderInner::Memory(storage) => storage.get_blob_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.get_blob_async(key).await,
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.get_blob_async(key).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(storage) => storage.get_blob_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.get_blob_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(storage) => storage.get_blob_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.get_blob_async(key).await,
        }
    }

    async fn delete_blob_async(&self, key: &str) -> StorageResult<()> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.delete_blob_async(key).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.delete_blob_async(key).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(storage) => storage.delete_blob_async(key).await,
            StorageProviderInner::Memory(storage) => storage.delete_blob_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.delete_blob_async(key).await,
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.delete_blob_async(key).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(storage) => storage.delete_blob_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.delete_blob_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(storage) => storage.delete_blob_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.delete_blob_async(key).await,
        }
    }

    async fn blob_exists_async(&self, key: &str) -> StorageResult<bool> {
        match &self.inner {
            #[cfg(all(feature = "turso", not(target_family = "wasm")))]
            StorageProviderInner::Turso(storage) => storage.blob_exists_async(key).await,
            #[cfg(all(feature = "libsql", not(target_family = "wasm")))]
            StorageProviderInner::Libsql(storage) => storage.blob_exists_async(key).await,
            #[cfg(not(target_family = "wasm"))]
            StorageProviderInner::JsonFile(storage) => storage.blob_exists_async(key).await,
            StorageProviderInner::Memory(storage) => storage.blob_exists_async(key).await,
            StorageProviderInner::MemoryJson(storage) => storage.blob_exists_async(key).await,
            #[cfg(all(feature = "d1", not(target_family = "wasm")))]
            StorageProviderInner::D1(storage) => storage.blob_exists_async(key).await,
            #[cfg(all(feature = "r2", not(target_family = "wasm")))]
            StorageProviderInner::R2(storage) => storage.blob_exists_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::D1Wasm(storage) => storage.blob_exists_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::R2Wasm(storage) => storage.blob_exists_async(key).await,
            #[cfg(all(target_family = "wasm", feature = "wasm-bindgen-storage"))]
            StorageProviderInner::KVWasm(storage) => storage.blob_exists_async(key).await,
        }
    }
}

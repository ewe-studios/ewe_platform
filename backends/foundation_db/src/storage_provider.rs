//! Core storage provider traits and abstractions.
//!
//! All storage operations return `StorageItemStream` (a Valtron Stream-based
//! lazy iterator) wrapped in `StorageResult`. Single-value operations yield
//! exactly one `Stream::Next` item; multi-value operations yield many.
//!
//! Callers collect at sync boundaries using Valtron helpers:
//! - `collect_one(stream)` — extract first `Next` value
//! - `collect_result(stream)` — drain all `Next` values into `Vec<T>`

use serde::{de::DeserializeOwned, Serialize};

use crate::backends::json_file::JsonFileStorage;
#[cfg(feature = "libsql")]
use crate::backends::libsql_backend::LibsqlStorage;
use crate::backends::memory::MemoryStorage;
#[cfg(feature = "turso")]
use crate::backends::turso_backend::TursoStorage;
use crate::backends::d1_kvstore::D1KeyValueStore;
use crate::backends::r2_blobstore::R2BlobStore;
pub use crate::errors::StorageError;
use crate::errors::StorageResult;
use foundation_core::valtron::Stream;

/// Type alias for streamed storage items.
/// This is a Valtron Stream-based lazy iterator that yields items one at a time.
/// Errors are yielded in the stream as `Stream::Next(Err(e))` - callers can use
/// `.flatten()` to extract only successful values or collect into `Result` to propagate errors.
pub type StorageItemStream<'a, T> =
    Box<dyn Iterator<Item = Stream<Result<T, StorageError>, ()>> + Send + 'a>;

/// A single SQL parameter value (crate-owned, backend-agnostic).
#[derive(Debug, Clone)]
pub enum DataValue {
    /// SQL NULL value.
    Null,
    /// Integer value (64-bit).
    Integer(i64),
    /// Real/floating-point value.
    Real(f64),
    /// Text/string value.
    Text(String),
    /// Binary blob value.
    Blob(Vec<u8>),
}

/// A single row from a SQL query result.
#[derive(Debug, Clone)]
pub struct SqlRow {
    columns: Vec<(String, DataValue)>,
}

impl SqlRow {
    /// Create a new [`SqlRow`] from column data.
    #[must_use]
    pub fn new(columns: Vec<(String, DataValue)>) -> Self {
        Self { columns }
    }

    /// Get a value by column index.
    ///
    /// # Errors
    ///
    /// Returns an error if the column index is out of bounds or conversion fails.
    pub fn get<T: FromDataValue>(&self, index: usize) -> StorageResult<T> {
        self.columns
            .get(index)
            .ok_or_else(|| {
                crate::errors::StorageError::SqlConversion(format!(
                    "Column index {index} out of bounds"
                ))
            })
            .and_then(|(_, v)| T::from_data_value(v))
    }

    /// Get a value by column name.
    ///
    /// # Errors
    ///
    /// Returns an error if the column name is not found or conversion fails.
    pub fn get_by_name<T: FromDataValue>(&self, name: &str) -> StorageResult<T> {
        self.columns
            .iter()
            .find(|(col_name, _)| col_name == name)
            .ok_or_else(|| {
                crate::errors::StorageError::SqlConversion(format!("Column '{name}' not found"))
            })
            .and_then(|(_, v)| T::from_data_value(v))
    }

    /// Get the number of columns in this row.
    #[must_use]
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
}

/// Trait for extracting typed values from [`DataValue`].
pub trait FromDataValue: Sized {
    /// Convert from a [`DataValue`].
    ///
    /// # Errors
    ///
    /// Returns an error if the conversion fails.
    fn from_data_value(value: &DataValue) -> StorageResult<Self>;
}

impl FromDataValue for String {
    fn from_data_value(value: &DataValue) -> StorageResult<Self> {
        match value {
            DataValue::Text(s) => Ok(s.clone()),
            DataValue::Null => Ok(String::new()),
            _ => Err(crate::errors::StorageError::SqlConversion(
                "Cannot convert to String".to_string(),
            )),
        }
    }
}

impl FromDataValue for i64 {
    fn from_data_value(value: &DataValue) -> StorageResult<Self> {
        match value {
            DataValue::Integer(i) => Ok(*i),
            DataValue::Null => Ok(0),
            _ => Err(crate::errors::StorageError::SqlConversion(
                "Cannot convert to i64".to_string(),
            )),
        }
    }
}

impl FromDataValue for i32 {
    fn from_data_value(value: &DataValue) -> StorageResult<Self> {
        match value {
            DataValue::Integer(i) => {
                // Allow truncation - caller is responsible for ensuring value fits
                #[allow(clippy::cast_possible_truncation)]
                return Ok(*i as i32);
            }
            DataValue::Null => Ok(0),
            _ => Err(crate::errors::StorageError::SqlConversion(
                "Cannot convert to i32".to_string(),
            )),
        }
    }
}

impl FromDataValue for f64 {
    fn from_data_value(value: &DataValue) -> StorageResult<Self> {
        match value {
            DataValue::Real(r) => Ok(*r),
            DataValue::Null => Ok(0.0),
            _ => Err(crate::errors::StorageError::SqlConversion(
                "Cannot convert to f64".to_string(),
            )),
        }
    }
}

impl FromDataValue for Vec<u8> {
    fn from_data_value(value: &DataValue) -> StorageResult<Self> {
        match value {
            DataValue::Blob(b) => Ok(b.clone()),
            DataValue::Null => Ok(Vec::new()),
            _ => Err(crate::errors::StorageError::SqlConversion(
                "Cannot convert to Vec<u8>".to_string(),
            )),
        }
    }
}

impl FromDataValue for bool {
    fn from_data_value(value: &DataValue) -> StorageResult<Self> {
        match value {
            DataValue::Integer(i) => Ok(*i != 0),
            DataValue::Null => Ok(false),
            _ => Err(crate::errors::StorageError::SqlConversion(
                "Cannot convert to bool".to_string(),
            )),
        }
    }
}

/// Key-value store operations available on all backends.
///
/// All methods return `StorageItemStream` for composable, non-blocking I/O.
/// Single-value operations yield exactly one `Stream::Next` item.
/// Use `collect_one` / `collect_result` at sync boundaries to extract values.
pub trait KeyValueStore: Send + Sync {
    /// Get a value by key. Yields one `Next(Option<V>)`.
    ///
    /// # Errors
    ///
    /// Returns an error if scheduling fails or deserialization fails.
    fn get<'a, V: DeserializeOwned + Send + 'static>(
        &'a self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'a, Option<V>>>;

    /// Set a key-value pair. Yields one `Next(())`.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or scheduling fails.
    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>>;

    /// Delete a key. Yields one `Next(())`.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend encounters an error.
    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>>;

    /// Check if a key exists. Yields one `Next(bool)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend encounters an error.
    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>>;

    /// List all keys with optional prefix filter.
    /// Yields multiple `Next(String)` items.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend encounters an error.
    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>>;
}

/// Blob storage operations for binary large objects.
pub trait BlobStore: Send + Sync {
    /// Put a blob into storage. Yields one `Next(())`.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend encounters an error.
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<StorageItemStream<'_, ()>>;

    /// Get a blob from storage. Yields one `Next(Option<Vec<u8>>)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend encounters an error.
    fn get_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<Vec<u8>>>>;

    /// Delete a blob. Yields one `Next(())`.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend encounters an error.
    fn delete_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>>;

    /// Check if a blob exists. Yields one `Next(bool)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend encounters an error.
    fn blob_exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>>;
}

/// SQL query operations for relational backends (Turso, D1).
pub trait QueryStore: Send + Sync {
    /// Execute a query that returns rows.
    /// Returns a stream of `Next(SqlRow)` items.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails or parameter conversion fails.
    fn query(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, SqlRow>>;

    /// Execute a statement that returns number of rows affected.
    /// Yields one `Next(u64)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the statement fails or parameter conversion fails.
    fn execute(&self, sql: &str, params: &[DataValue])
        -> StorageResult<StorageItemStream<'_, u64>>;

    /// Execute a batch of SQL statements. Yields one `Next(())`.
    ///
    /// # Errors
    ///
    /// Returns an error if any statement in the batch fails.
    fn execute_batch(&self, sql: &str) -> StorageResult<StorageItemStream<'_, ()>>;
}

/// Rate limiting operations.
pub trait RateLimiterStore: Send + Sync {
    /// Check if a rate limit key is allowed. Yields one `Next(bool)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend encounters an error.
    fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<StorageItemStream<'_, bool>>;

    /// Record a rate-limited action. Yields one `Next(u32)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend encounters an error.
    fn record_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, u32>>;

    /// Reset a rate limit key. Yields one `Next(())`.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend encounters an error.
    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>>;
}

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
    D1,
    /// Cloudflare R2 backend with bucket configuration.
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
    D1(D1KeyValueStore),
    R2(R2BlobStore),
}

impl StorageProvider {
    /// Create a new storage provider with the specified backend.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend initialization fails or if the requested
    /// backend is not yet implemented.
    pub fn new(backend: StorageBackend) -> StorageResult<Self> {
        match backend {
            #[cfg(feature = "turso")]
            StorageBackend::Turso { url } => {
                let storage = TursoStorage::new(&url)?;
                Ok(Self {
                    inner: StorageProviderInner::Turso(Box::new(storage)),
                })
            }
            #[cfg(feature = "libsql")]
            StorageBackend::Libsql { url } => {
                let storage = LibsqlStorage::new(&url)?;
                Ok(Self {
                    inner: StorageProviderInner::Libsql(Box::new(storage)),
                })
            }
            StorageBackend::D1 => {
                let storage = D1KeyValueStore::from_env()?;
                Ok(Self {
                    inner: StorageProviderInner::D1(storage),
                })
            }
            StorageBackend::R2 { bucket } => {
                let storage = R2BlobStore::from_env()?;
                // Override prefix with bucket if specified
                let storage = if !bucket.is_empty() {
                    R2BlobStore::new(
                        &std::env::var("CLOUDFLARE_API_TOKEN").unwrap_or_default(),
                        &std::env::var("CLOUDFLARE_ACCOUNT_ID").unwrap_or_default(),
                        &bucket,
                        "blobs",
                    )
                } else {
                    storage
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
            StorageProviderInner::D1(storage) => storage.get(key),
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
            StorageProviderInner::D1(storage) => storage.set(key, value),
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
            StorageProviderInner::D1(storage) => storage.delete(key),
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
            StorageProviderInner::D1(storage) => storage.exists(key),
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
            StorageProviderInner::D1(storage) => storage.list_keys(prefix),
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
            StorageProviderInner::D1(storage) => storage.query(sql, params),
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
            StorageProviderInner::D1(storage) => storage.execute(sql, params),
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
            StorageProviderInner::D1(storage) => storage.execute_batch(sql),
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
            StorageProviderInner::D1(storage) => {
                storage.check_rate_limit(key, max_count, window_seconds)
            }
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
            StorageProviderInner::D1(storage) => storage.record_rate_limit(key),
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
            StorageProviderInner::D1(storage) => storage.reset_rate_limit(key),
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
            StorageProviderInner::D1(storage) => storage.put_blob(key, data),
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
            StorageProviderInner::D1(storage) => storage.get_blob(key),
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
            StorageProviderInner::D1(storage) => storage.delete_blob(key),
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
            StorageProviderInner::D1(storage) => storage.blob_exists(key),
            StorageProviderInner::R2(storage) => storage.blob_exists(key),
        }
    }
}

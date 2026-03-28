//! Core storage provider traits and abstractions.
//!
//! All storage traits are synchronous. Multi-value operations return
//! `StorageItemStream` (a Valtron Stream-based lazy iterator) instead
//! of allocating Vecs. Single-value operations return `StorageResult<T>` directly.

use serde::{de::DeserializeOwned, Serialize};

use crate::backends::{MemoryStorage, TursoStorage};
use crate::errors::StorageResult;

/// Type alias for streamed storage items.
/// This is a Valtron Stream-based lazy iterator that yields items one at a time.
pub type StorageItemStream<'a, T> = Box<dyn Iterator<Item = T> + Send + 'a>;

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
    /// Create a new SqlRow from column data.
    #[must_use]
    pub fn new(columns: Vec<(String, DataValue)>) -> Self {
        Self { columns }
    }

    /// Get a value by column index.
    pub fn get<T: FromDataValue>(&self, index: usize) -> StorageResult<T> {
        self.columns
            .get(index)
            .map(|(_, v)| T::from_data_value(v))
            .unwrap_or_else(|| {
                Err(crate::errors::StorageError::SqlConversion(format!(
                    "Column index {index} out of bounds"
                )))
            })
    }

    /// Get a value by column name.
    pub fn get_by_name<T: FromDataValue>(&self, name: &str) -> StorageResult<T> {
        self.columns
            .iter()
            .find(|(col_name, _)| col_name == name)
            .map(|(_, v)| T::from_data_value(v))
            .unwrap_or_else(|| {
                Err(crate::errors::StorageError::SqlConversion(format!(
                    "Column '{name}' not found"
                )))
            })
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
            DataValue::Integer(i) => Ok(*i as i32),
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
/// All methods are synchronous. Multi-value operations return
/// `StorageItemStream` for lazy iteration.
pub trait KeyValueStore: Send + Sync {
    /// Get a value by key.
    fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>>;

    /// Set a key-value pair.
    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<()>;

    /// Delete a key.
    fn delete(&self, key: &str) -> StorageResult<()>;

    /// Check if a key exists.
    fn exists(&self, key: &str) -> StorageResult<bool>;

    /// List all keys with optional prefix filter.
    /// Returns a stream for lazy iteration over keys.
    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>>;
}

/// Blob storage operations for binary large objects.
pub trait BlobStore: Send + Sync {
    /// Put a blob into storage.
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()>;

    /// Get a blob from storage.
    fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>>;

    /// Delete a blob.
    fn delete_blob(&self, key: &str) -> StorageResult<()>;

    /// Check if a blob exists.
    fn blob_exists(&self, key: &str) -> StorageResult<bool>;
}

/// SQL query operations for relational backends (Turso, D1).
pub trait QueryStore: Send + Sync {
    /// Execute a query that returns rows.
    /// Returns a stream for lazy iteration over rows.
    fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, SqlRow>>;

    /// Execute a statement that returns number of rows affected.
    fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64>;

    /// Execute a batch of SQL statements.
    fn execute_batch(&self, sql: &str) -> StorageResult<()>;
}

/// Rate limiting operations.
pub trait RateLimiterStore: Send + Sync {
    /// Check if a rate limit key is allowed.
    fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool>;

    /// Record a rate-limited action.
    fn record_rate_limit(&self, key: &str) -> StorageResult<u32>;

    /// Reset a rate limit key.
    fn reset_rate_limit(&self, key: &str) -> StorageResult<()>;
}

/// Storage backend enumeration for runtime selection.
#[derive(Debug, Clone)]
pub enum StorageBackend {
    /// Turso backend with database URL.
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
    #[allow(dead_code)] // TODO: Implement D1 backend
    D1,
    #[allow(dead_code)] // TODO: Implement R2 backend
    R2,
    Memory(MemoryStorage),
}

impl StorageProvider {
    /// Create a new storage provider with the specified backend.
    pub fn new(backend: StorageBackend) -> StorageResult<Self> {
        match backend {
            StorageBackend::Turso { url } => {
                let storage = TursoStorage::new(&url)?;
                Ok(Self {
                    inner: StorageProviderInner::Turso(Box::new(storage)),
                })
            }
            StorageBackend::D1 => {
                Err(crate::errors::StorageError::Generic(
                    "D1 backend not yet implemented".to_string(),
                ))
            }
            StorageBackend::R2 { .. } => {
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

impl KeyValueStore for StorageProvider {
    fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
        match &self.inner {
            StorageProviderInner::Turso(storage) => storage.get(key),
            StorageProviderInner::Memory(storage) => storage.get(key),
            _ => Err(crate::errors::StorageError::Generic(
                "Backend not implemented".to_string(),
            )),
        }
    }

    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<()> {
        match &self.inner {
            StorageProviderInner::Turso(storage) => storage.set(key, value),
            StorageProviderInner::Memory(storage) => storage.set(key, value),
            _ => Err(crate::errors::StorageError::Generic(
                "Backend not implemented".to_string(),
            )),
        }
    }

    fn delete(&self, key: &str) -> StorageResult<()> {
        match &self.inner {
            StorageProviderInner::Turso(storage) => storage.delete(key),
            StorageProviderInner::Memory(storage) => storage.delete(key),
            _ => Err(crate::errors::StorageError::Generic(
                "Backend not implemented".to_string(),
            )),
        }
    }

    fn exists(&self, key: &str) -> StorageResult<bool> {
        match &self.inner {
            StorageProviderInner::Turso(storage) => storage.exists(key),
            StorageProviderInner::Memory(storage) => storage.exists(key),
            _ => Err(crate::errors::StorageError::Generic(
                "Backend not implemented".to_string(),
            )),
        }
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        match &self.inner {
            StorageProviderInner::Turso(storage) => storage.list_keys(prefix),
            StorageProviderInner::Memory(storage) => storage.list_keys(prefix),
            _ => Err(crate::errors::StorageError::Generic(
                "Backend not implemented".to_string(),
            )),
        }
    }
}

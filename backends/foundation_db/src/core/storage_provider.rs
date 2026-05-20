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

pub use crate::core::errors::StorageError;
use crate::core::errors::StorageResult;
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
                crate::core::errors::StorageError::SqlConversion(format!(
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
                crate::core::errors::StorageError::SqlConversion(format!("Column '{name}' not found"))
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
            _ => Err(crate::core::errors::StorageError::SqlConversion(
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
            _ => Err(crate::core::errors::StorageError::SqlConversion(
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
            _ => Err(crate::core::errors::StorageError::SqlConversion(
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
            _ => Err(crate::core::errors::StorageError::SqlConversion(
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
            _ => Err(crate::core::errors::StorageError::SqlConversion(
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
            _ => Err(crate::core::errors::StorageError::SqlConversion(
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

/// Async SQL query operations — for wasm backends (D1) where the underlying
/// JS APIs are Promise-based and cannot be called synchronously.
#[async_trait::async_trait(?Send)]
pub trait AsyncQueryStore {
    /// Execute a query that returns rows.
    async fn query_async(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<Vec<SqlRow>>;

    /// Execute a statement that returns number of rows affected.
    async fn execute_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64>;

    /// Execute a batch of SQL statements.
    async fn execute_batch_async(&self, sql: &str) -> StorageResult<()>;
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

//! Core storage provider traits and abstractions.
//!
//! All storage operations return `StorageItemStream` (a Valtron Stream-based
//! lazy iterator) wrapped in `StorageResult`. Single-value operations yield
//! exactly one `Stream::Next` item; multi-value operations yield many.
//!
//! Callers collect at sync boundaries using Valtron helpers:
//! - `collect_one(stream)` — extract first `Next` value
//! - `collect_result(stream)` — drain all `Next` values into `Vec<T>`

use futures_lite::StreamExt;
use serde::{de::DeserializeOwned, Serialize};
use std::pin::Pin;
use std::task::{Context, Poll};

pub use crate::core::errors::StorageError;
use crate::core::errors::StorageResult;
use foundation_core::valtron::Stream;

// Alias for futures_core::Stream to avoid name collision with valtron::Stream
type AsyncStream<T> = Pin<Box<dyn futures_core::Stream<Item = T> + Send>>;

/// Type alias for streamed storage items.
/// This is a Valtron Stream-based lazy iterator that yields items one at a time.
/// Errors are yielded in the stream as `Stream::Next(Err(e))` - callers can use
/// `.flatten()` to extract only successful values or collect into `Result` to propagate errors.
#[cfg(target_arch = "wasm32")]
pub type StorageItemStream<'a, T> =
    Box<dyn Iterator<Item = Stream<Result<T, StorageError>, ()>> + 'a>;

#[cfg(not(target_arch = "wasm32"))]
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

// ===========================================================================
// Async stream types — returned by AsyncQueryStore and AsyncKeyValueStore
// ===========================================================================

/// An async stream of SQL rows from a query.
///
/// For Turso/Libsql: yields rows one at a time as they're fetched from the DB.
/// For D1 backends: buffered (the HTTP/JS API returns all rows at once).
///
/// Callers can iterate row-by-row via `next().await` or collect all rows
/// at once with `collect_all().await`.
pub struct AsyncQueryStream {
    inner: AsyncStream<StorageResult<SqlRow>>,
}

impl AsyncQueryStream {
    /// Wrap any `futures_core::Stream<Item = StorageResult<SqlRow>>` as an AsyncQueryStream.
    pub fn new<S>(stream: S) -> Self
    where
        S: futures_core::Stream<Item = StorageResult<SqlRow>> + Send + 'static,
    {
        Self { inner: Box::pin(stream) }
    }

    /// Collect all remaining rows into a Vec.
    pub async fn collect_all(self) -> StorageResult<Vec<SqlRow>> {
        self.inner.collect::<Vec<_>>().await.into_iter().collect()
    }
}

impl futures_core::Stream for AsyncQueryStream {
    type Item = StorageResult<SqlRow>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_next(cx)
    }
}

/// An async stream of keys from a list operation.
///
/// For Turso/Libsql: yields keys one at a time.
/// For D1 backends: buffered (all keys arrive at once).
pub struct AsyncListStream {
    inner: AsyncStream<StorageResult<String>>,
}

impl AsyncListStream {
    /// Wrap any `futures_core::Stream<Item = StorageResult<String>>` as an AsyncListStream.
    pub fn new<S>(stream: S) -> Self
    where
        S: futures_core::Stream<Item = StorageResult<String>> + Send + 'static,
    {
        Self { inner: Box::pin(stream) }
    }

    /// Collect all remaining keys into a Vec.
    pub async fn collect_all(self) -> StorageResult<Vec<String>> {
        self.inner.collect::<Vec<_>>().await.into_iter().collect()
    }
}

impl futures_core::Stream for AsyncListStream {
    type Item = StorageResult<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_next(cx)
    }
}

// ===========================================================================
// Sync iterator bridges — used inside `run_future_iter` worker threads
// to bridge async streams to sync Iterators.
// ===========================================================================

/// Bridges `AsyncQueryStream` to a sync `Iterator`.
/// Used inside `run_future_iter` worker threads where `block_on` is available.
pub struct AsyncQueryStreamIterator {
    stream: AsyncQueryStream,
}

impl AsyncQueryStreamIterator {
    pub fn new(stream: AsyncQueryStream) -> Self {
        Self { stream }
    }
}

impl Iterator for AsyncQueryStreamIterator {
    type Item = StorageResult<SqlRow>;

    fn next(&mut self) -> Option<Self::Item> {
        use futures_lite::future::block_on;
        block_on(self.stream.next())
    }
}

/// Bridges `AsyncListStream` to a sync `Iterator`.
/// Used inside `run_future_iter` worker threads where `block_on` is available.
pub struct AsyncListStreamIterator {
    stream: AsyncListStream,
}

impl AsyncListStreamIterator {
    pub fn new(stream: AsyncListStream) -> Self {
        Self { stream }
    }
}

impl Iterator for AsyncListStreamIterator {
    type Item = StorageResult<String>;

    fn next(&mut self) -> Option<Self::Item> {
        use futures_lite::future::block_on;
        block_on(self.stream.next())
    }
}

/// Key-value store operations available on all backends.
///
/// Single-value operations return `StorageResult<T>` directly.
/// Multi-value operations return `StorageItemStream<T>`.
pub trait KeyValueStore: Send + Sync {
    /// Get a value by key. Returns `None` if key doesn't exist.
    fn get<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>>;

    /// Set a key-value pair.
    fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()>;

    /// Delete a key.
    fn delete(&self, key: &str) -> StorageResult<()>;

    /// Check if a key exists.
    fn exists(&self, key: &str) -> StorageResult<bool>;

    /// List all keys with optional prefix filter.
    /// Returns a stream of keys.
    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>>;
}

/// Blob storage operations for binary large objects.
pub trait BlobStore: Send + Sync {
    /// Put a blob into storage.
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()>;

    /// Get a blob from storage. Returns `None` if key doesn't exist.
    fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>>;

    /// Delete a blob.
    fn delete_blob(&self, key: &str) -> StorageResult<()>;

    /// Check if a blob exists.
    fn blob_exists(&self, key: &str) -> StorageResult<bool>;
}

/// SQL query operations for relational backends (Turso, D1).
pub trait QueryStore: Send + Sync {
    /// Execute a query that returns rows.
    /// Returns a stream of `SqlRow` items.
    fn query(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<StorageItemStream<'_, SqlRow>>;

    /// Execute a statement that returns number of rows affected.
    fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64>;

    /// Execute a batch of SQL statements.
    fn execute_batch(&self, sql: &str) -> StorageResult<()>;
}

/// Async SQL query operations — for wasm backends (D1) where the underlying
/// JS APIs are Promise-based and cannot be called synchronously.
#[async_trait::async_trait(?Send)]
pub trait AsyncQueryStore {
    /// Execute a query that returns rows as an async stream.
    ///
    /// For Turso/Libsql: yields rows one at a time as they're fetched.
    /// For D1 backends: buffered (API returns all rows at once).
    ///
    /// Use `.collect_all().await` to get all rows, or iterate row-by-row
    /// via `.next().await`.
    async fn query_async(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<AsyncQueryStream>;

    /// Execute a statement that returns number of rows affected.
    async fn execute_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64>;

    /// Execute a batch of SQL statements.
    async fn execute_batch_async(&self, sql: &str) -> StorageResult<()>;
}

/// Async key-value store operations — for wasm backends where the underlying
/// JS APIs are Promise-based and cannot be called synchronously.
#[async_trait::async_trait(?Send)]
pub trait AsyncKeyValueStore {
    async fn get_async<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>>;
    async fn set_async<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()>;
    async fn delete_async(&self, key: &str) -> StorageResult<()>;
    async fn exists_async(&self, key: &str) -> StorageResult<bool>;
    /// List all keys with optional prefix filter, returning an async stream.
    ///
    /// For Turso/Libsql: yields keys one at a time.
    /// For D1 backends: buffered (API returns all keys at once).
    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream>;
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

    /// Record a rate-limited action. Returns the current count.
    fn record_rate_limit(&self, key: &str) -> StorageResult<u32>;

    /// Reset a rate limit key.
    fn reset_rate_limit(&self, key: &str) -> StorageResult<()>;
}

/// Async blob store operations — for wasm backends where the underlying
/// JS APIs are Promise-based and cannot be called synchronously.
#[async_trait::async_trait(?Send)]
pub trait AsyncBlobStore {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> StorageResult<()>;
    async fn get_blob_async(&self, key: &str) -> StorageResult<Option<Vec<u8>>>;
    async fn delete_blob_async(&self, key: &str) -> StorageResult<()>;
    async fn blob_exists_async(&self, key: &str) -> StorageResult<bool>;
}

/// Async rate limiter store operations — for wasm backends where the underlying
/// JS APIs are Promise-based and cannot be called synchronously.
#[async_trait::async_trait(?Send)]
pub trait AsyncRateLimiterStore {
    async fn check_rate_limit_async(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool>;

    async fn record_rate_limit_async(&self, key: &str) -> StorageResult<u32>;

    async fn reset_rate_limit_async(&self, key: &str) -> StorageResult<()>;
}

// ===========================================================================
// StoreResponse — reserved for future use
//
// If an operation ever needs to support both single-value and streaming
// returns from the same method, use:
//
//   pub enum StoreResponse<T, S> {
//       One(T),
//       Stream(StorageItemStream<S>),
//   }
//
// Currently not needed — each method has a clear single vs multi-value intent.
// ===========================================================================

/// A single document in a document store.
#[derive(Debug, Clone)]
pub struct Document {
    /// Unique document ID within the collection.
    pub id: String,
    /// The document content as a JSON string.
    pub content: String,
    /// Optional metadata (created_at, updated_at, etc.).
    pub metadata: serde_json::Value,
}

/// Document append operations — available on all backends.
///
/// Documents are stored as JSON strings in an append-only collection.
/// Each document gets a unique ID (scru128 or similar).
/// Collections are identified by a key (e.g., `session:{id}:messages`).
pub trait DocumentStore: Send + Sync {
    /// Append a document to a collection. Returns the stored document with assigned ID.
    ///
    /// The backend assigns a unique ID to the document.
    fn append<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document>;

    /// Scan the last N documents from a collection.
    /// Returns a stream of documents, newest-first.
    fn scan<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<StorageItemStream<'_, V>>;

    /// Scan all documents from a collection, oldest-first.
    /// Returns a stream of all documents.
    fn scan_all<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<StorageItemStream<'_, V>>;

    /// Delete a specific document from a collection by its ID.
    fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()>;

    /// Delete all documents in a collection. Returns count of deleted documents.
    fn delete_all(&self, key: &str) -> StorageResult<u64>;

    /// Count documents in a collection.
    fn count(&self, key: &str) -> StorageResult<u64>;
}

/// Async document store operations — for wasm backends where the underlying
/// JS APIs are Promise-based and cannot be called synchronously.
#[async_trait::async_trait(?Send)]
pub trait AsyncDocumentStore {
    /// Append a document to a collection.
    async fn append_async<V: Serialize + Send + 'static>(&self, key: &str, content: V) -> StorageResult<Document>;

    /// Scan the last N documents from a collection.
    async fn scan_async<V: DeserializeOwned + Send + 'static>(&self, key: &str, limit: usize) -> StorageResult<Vec<V>>;

    /// Scan all documents from a collection, oldest-first.
    async fn scan_all_async<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Vec<V>>;

    /// Delete a specific document.
    async fn delete_async(&self, key: &str, doc_id: &str) -> StorageResult<()>;

    /// Delete all documents in a collection.
    async fn delete_all_async(&self, key: &str) -> StorageResult<u64>;

    /// Count documents in a collection.
    async fn count_async(&self, key: &str) -> StorageResult<u64>;
}

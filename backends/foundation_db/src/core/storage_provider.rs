//! Core storage traits and types.
//!
//! Every storage trait is async (`KeyValueStore`, `BlobStore`,
//! `QueryStore`, `RateLimiterStore`, `DocumentStore`).
//! Single values are awaited directly; multi-row results are lazily pulled
//! async streams ([`AsyncStorageItemStream`], [`AsyncQueryStream`],
//! [`AsyncListStream`]), never collected into memory by the store. A sync
//! entry point awaits them with valtron's `block_on_future` / `.block()`
//! behind `blocking_boundary!`.

use futures_lite::StreamExt;
use serde::{de::DeserializeOwned, Serialize};
use std::pin::Pin;
use std::task::{Context, Poll};

pub use crate::core::errors::StorageError;
use crate::core::errors::StorageResult;

// A boxed `futures_core::Stream` (the inner stream of the query/list wrappers).
type AsyncStream<T> = Pin<Box<dyn futures_core::Stream<Item = T> + Send>>;

/// A lazily-pulled async stream of deserialized items, each a
/// `StorageResult<T>`.
///
/// Consumers pull one item at a time with `.next().await`
/// (`while let Some(item) = stream.next().await { ... }`), so even an unbounded
/// scan never materializes a `Vec` — back-pressure is natural and large
/// collections can't OOM.
#[cfg(target_family = "wasm")]
pub type AsyncStorageItemStream<'a, T> =
    Pin<Box<dyn futures_core::Stream<Item = StorageResult<T>> + 'a>>;

#[cfg(not(target_family = "wasm"))]
pub type AsyncStorageItemStream<'a, T> =
    Pin<Box<dyn futures_core::Stream<Item = StorageResult<T>> + Send + 'a>>;

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
                crate::core::errors::StorageError::SqlConversion(format!(
                    "Column '{name}' not found"
                ))
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

/// SQL `NULL` maps to `None`; any other value is converted via `T` and wrapped
/// in `Some`. Use this (not `String`) to distinguish a NULL column from an empty
/// string — `String::from_data_value` deliberately coerces NULL to `""`.
impl<T: FromDataValue> FromDataValue for Option<T> {
    fn from_data_value(value: &DataValue) -> StorageResult<Self> {
        match value {
            DataValue::Null => Ok(None),
            other => T::from_data_value(other).map(Some),
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
// Async stream types — returned by QueryStore and KeyValueStore
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
    /// Wrap any `futures_core::Stream<Item = StorageResult<SqlRow>>` as an `AsyncQueryStream`.
    pub fn new<S>(stream: S) -> Self
    where
        S: futures_core::Stream<Item = StorageResult<SqlRow>> + Send + 'static,
    {
        Self {
            inner: Box::pin(stream),
        }
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
    /// Wrap any `futures_core::Stream<Item = StorageResult<String>>` as an `AsyncListStream`.
    pub fn new<S>(stream: S) -> Self
    where
        S: futures_core::Stream<Item = StorageResult<String>> + Send + 'static,
    {
        Self {
            inner: Box::pin(stream),
        }
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

/// Async SQL query operations — for wasm backends (D1) where the underlying
/// JS APIs are Promise-based and cannot be called synchronously.
#[async_trait::async_trait]
pub trait QueryStore: Send + Sync {
    /// Execute a query that returns rows as an async stream.
    ///
    /// For Turso/Libsql: yields rows one at a time as they're fetched.
    /// For D1 backends: buffered (API returns all rows at once).
    ///
    /// Use `.collect_all().await` to get all rows, or iterate row-by-row
    /// via `.next().await`.
    async fn query(&self, sql: &str, params: &[DataValue])
        -> StorageResult<AsyncQueryStream>;

    /// Execute a statement that returns number of rows affected.
    async fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64>;

    /// Execute a batch of SQL statements.
    async fn execute_batch(&self, sql: &str) -> StorageResult<()>;

    /// Execute several parameterized statements in one all-or-nothing unit.
    ///
    /// Backends that support it wrap the statements in one transaction (or a
    /// D1 batch): a failure in any statement leaves none of them applied. The
    /// default runs each statement in turn and is not atomic.
    async fn execute_batch_statements(
        &self,
        statements: &[(String, Vec<DataValue>)],
    ) -> StorageResult<()> {
        for (sql, params) in statements {
            self.execute(sql, params).await?;
        }
        Ok(())
    }
}

/// Async key-value store operations — for wasm backends where the underlying
/// JS APIs are Promise-based and cannot be called synchronously.
#[async_trait::async_trait]
pub trait KeyValueStore: Send + Sync {
    async fn get<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>>;
    async fn set<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> StorageResult<()>;
    async fn delete(&self, key: &str) -> StorageResult<()>;
    async fn exists(&self, key: &str) -> StorageResult<bool>;
    /// List all keys with optional prefix filter, returning an async stream.
    ///
    /// For Turso/Libsql: yields keys one at a time.
    /// For D1 backends: buffered (API returns all keys at once).
    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream>;
}

/// A byte-chunk stream for a blob body. Each item is one chunk; the whole
/// object is the concatenation of all chunks.
pub type BlobStream = Pin<Box<dyn futures_core::Stream<Item = StorageResult<Vec<u8>>> + Send>>;

/// An ordered stream of blob keys from a listing.
pub type BlobListStream = Pin<Box<dyn futures_core::Stream<Item = StorageResult<String>> + Send>>;

/// A half-open byte range `[start, end)` for [`BlobStore::get_blob_range`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlobRange {
    /// Inclusive first byte offset.
    pub start: u64,
    /// Exclusive last byte offset; `None` means "to the end".
    pub end: Option<u64>,
}

/// A precondition for [`BlobStore::put_blob_conditionally`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobPutCondition {
    /// Reject the write if the object already exists (`If-None-Match: *`).
    IfNoneMatch,
    /// Reject the write unless the stored object's `ETag` equals this value.
    IfMatch(String),
}

/// Collect a [`BlobStream`] into one buffer. Use only for small objects;
/// the stores prefer [`BlobStore::put_blob_stream`] for large bodies.
pub(crate) async fn collect_blob_stream(mut chunks: BlobStream) -> StorageResult<Vec<u8>> {
    let mut out = Vec::new();
    while let Some(chunk) = chunks.next().await {
        out.extend_from_slice(&chunk?);
    }
    Ok(out)
}

/// A deterministic `ETag` for blob content: lowercase SHA-256 hex. Backends that
/// have no server-side `ETag` derive one from the stored bytes so conditional
/// writes work everywhere.
#[must_use]
pub fn content_etag(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(data))
}

/// Async blob store operations — for wasm backends where the underlying
/// JS APIs are Promise-based and cannot be called synchronously.
#[async_trait::async_trait]
pub trait BlobStore: Send + Sync {
    /// Store a whole object. A thin wrapper around
    /// [`put_blob_stream`](Self::put_blob_stream) for small objects.
    async fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()>;

    /// Read a whole object. A thin wrapper around
    /// [`get_blob_stream`](Self::get_blob_stream) for small objects.
    async fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>>;

    async fn delete_blob(&self, key: &str) -> StorageResult<()>;
    async fn blob_exists(&self, key: &str) -> StorageResult<bool>;

    /// Stream a blob body out, one chunk at a time.
    ///
    /// Default: the whole object as a single chunk.
    async fn get_blob_stream(&self, key: &str) -> StorageResult<BlobStream> {
        let data = self.get_blob(key).await?.unwrap_or_default();
        Ok(Box::pin(futures_lite::stream::iter(
            (!data.is_empty()).then_some(data).into_iter().map(Ok),
        )))
    }

    /// Stream a blob body in. `length`, when known, lets the backend set
    /// `Content-Length`; `None` means an unknown-length (chunked) body.
    ///
    /// Default: buffer the whole body, then [`put_blob`](Self::put_blob).
    async fn put_blob_stream(
        &self,
        key: &str,
        length: Option<u64>,
        chunks: BlobStream,
    ) -> StorageResult<()> {
        let _ = length;
        let data = collect_blob_stream(chunks).await?;
        self.put_blob(key, &data).await
    }

    /// The blob's `ETag`, or `None` if the backend does not expose one.
    async fn blob_etag(&self, key: &str) -> StorageResult<Option<String>> {
        let _ = key;
        Ok(None)
    }

    /// Read a byte range `[start, end)` as a stream.
    ///
    /// Default: read the whole object and slice it in memory.
    async fn get_blob_range(&self, key: &str, range: BlobRange) -> StorageResult<BlobStream> {
        let data = self.get_blob(key).await?.unwrap_or_default();
        let start = usize::try_from(range.start)
            .unwrap_or(usize::MAX)
            .min(data.len());
        let end = range
            .end
            .map_or(data.len(), |e| usize::try_from(e).unwrap_or(usize::MAX))
            .min(data.len());
        let slice = if start >= end {
            Vec::new()
        } else {
            data[start..end].to_vec()
        };
        Ok(Box::pin(futures_lite::stream::iter(
            vec![slice].into_iter().map(Ok),
        )))
    }

    /// Write a blob only when `condition` holds. Fails with
    /// [`StorageError::PreconditionFailed`] when it does not.
    ///
    /// Default: a non-atomic check-then-write.
    async fn put_blob_conditionally(
        &self,
        key: &str,
        data: &[u8],
        condition: BlobPutCondition,
    ) -> StorageResult<()> {
        match &condition {
            BlobPutCondition::IfNoneMatch => {
                if self.blob_exists(key).await? {
                    return Err(StorageError::PreconditionFailed(key.to_string()));
                }
            }
            BlobPutCondition::IfMatch(expected) => {
                let current = self.blob_etag(key).await?;
                if current.as_deref() != Some(expected.as_str()) {
                    return Err(StorageError::PreconditionFailed(key.to_string()));
                }
            }
        }
        self.put_blob(key, data).await
    }

    /// Ordered blob keys that start with `prefix` and sort after `start_after`,
    /// up to `limit` (`None` = unlimited), as a stream.
    ///
    /// Default: unsupported (a backend that lists blobs overrides it).
    async fn list_blobs_after(
        &self,
        prefix: &str,
        start_after: Option<&str>,
        limit: Option<usize>,
    ) -> StorageResult<BlobListStream> {
        let _ = (prefix, start_after, limit);
        Err(StorageError::Generic(
            "list_blobs_after is not supported for this backend".to_string(),
        ))
    }
}

/// Fixed-window rate limits.
///
/// A window lasts `window_seconds`, which the caller takes from its own
/// configuration and passes to both `check_rate_limit` and
/// `record_rate_limit`. Once a key's window has elapsed
/// (`now - window_start >= window_seconds`) a check allows it again and the
/// next record starts a new window; inside the window, a record increments
/// the count and keeps the window's start.
#[async_trait::async_trait]
pub trait RateLimiterStore: Send + Sync {
    /// Whether one more hit on `key` is allowed: its window has elapsed, or it
    /// has had fewer than `max_count` hits in the current one.
    async fn check_rate_limit(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool>;

    /// Record one hit on `key` and return the count in its current window
    /// (1 when this hit starts a new window).
    async fn record_rate_limit(&self, key: &str, window_seconds: u64) -> StorageResult<u32>;

    /// Forget `key`'s window.
    async fn reset_rate_limit(&self, key: &str) -> StorageResult<()>;
}

/// A single document in a document store.
///
/// `#[non_exhaustive]` so future promoted columns can be added without breaking
/// construction sites (use `Document { .. }` with the named fields).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Document {
    /// Unique document ID within the collection (scru128 — time-ordered).
    pub id: String,
    /// The document content as a JSON string (full fidelity, source of truth).
    pub content: String,
    /// Optional metadata (`created_at`, `updated_at`, etc.).
    pub metadata: serde_json::Value,
    /// Promoted searchable column — a short title/label (nullable).
    pub title: Option<String>,
    /// Promoted searchable column — a summary/first-line (nullable).
    pub summary: Option<String>,
    /// Promoted searchable column — the record discriminant
    /// ("conversation"/"observation"/...); nullable.
    pub record_type: Option<String>,
}

impl Document {
    #[must_use]
    pub fn new(
        id: String,
        content: String,
        metadata: serde_json::Value,
        title: Option<String>,
        summary: Option<String>,
        record_type: Option<String>,
    ) -> Self {
        Self {
            id,
            content,
            metadata,
            title,
            summary,
            record_type,
        }
    }
}

/// A type whose values can populate the `DocumentStore`'s promoted columns.
///
/// WHY: rather than make callers pass `title`/`summary`/`record_type` to every
/// `append`, a record type implements this once and the store extracts them
/// (OD-06-1). F01's `SessionRecord` implements it (`record_type` from the
/// variant, `summary` from observation/reflection text).
pub trait PromotableDocument {
    /// The record discriminant ("conversation"/"observation"/...), if any.
    fn record_type(&self) -> Option<String> {
        None
    }
    /// A short title/label, if any.
    fn title(&self) -> Option<String> {
        None
    }
    /// A summary/first-line, if any.
    fn summary(&self) -> Option<String> {
        None
    }
}

/// Async document store operations — for wasm backends where the underlying
/// JS APIs are Promise-based and cannot be called synchronously.
#[async_trait::async_trait]
pub trait DocumentStore: Send + Sync {
    /// Append a document to a collection.
    async fn append<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document>;

    /// Append a document with a caller-supplied scru128 `doc_id` (OD-06-5).
    ///
    /// Idempotent (F16 E7): appending the same `doc_id` in the same collection
    /// again is a no-op. The existing document is kept and no duplicate row is
    /// created. This lets a caller retry a write whose future was dropped after
    /// the store accepted it, without writing the record twice.
    async fn append_with_id<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document>;

    /// Append a document, populating the promoted columns from [`PromotableDocument`]
    /// (async mirror of `append_promotable`; OD-22b-1).
    async fn append_promotable<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        content: V,
    ) -> StorageResult<Document>;

    /// Like [`append_promotable`](Self::append_promotable) with a caller-supplied `doc_id`.
    ///
    /// Idempotent like [`append_with_id`](Self::append_with_id): re-appending
    /// the same `doc_id` in the same collection is a no-op (F16 E7).
    async fn append_promotable_with_id<V: Serialize + PromotableDocument + Send + 'static>(
        &self,
        key: &str,
        doc_id: &str,
        content: V,
    ) -> StorageResult<Document>;

    /// Scan the last N documents as full [`Document`]s (promoted columns observable),
    /// newest-first. Bounded by `limit`, so this returns a `Vec` (async mirror of
    /// `scan_documents`; OD-22b-1).
    async fn scan_documents(&self, key: &str, limit: usize) -> StorageResult<Vec<Document>>;

    /// Scan documents with id **>= `from_id`** as full [`Document`]s, oldest-first,
    /// up to `limit` (0 = unlimited).
    async fn scan_documents_from(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<Vec<Document>>;

    /// Scan the last N documents from a collection, newest-first (by `doc_id`).
    /// Returns a lazily-pulled [`AsyncStorageItemStream`] — pull each item with
    /// `.next().await`; nothing is materialized into a `Vec` (no OOM).
    async fn scan<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>>;

    /// Scan all documents from a collection, oldest-first (by `doc_id`), as a
    /// lazily-pulled [`AsyncStorageItemStream`].
    async fn scan_all<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>>;

    /// Scan documents whose id is >= `from_id` (inclusive), oldest-first, up to
    /// `limit` (0 = unlimited), as a lazily-pulled [`AsyncStorageItemStream`]
    /// (`while let Some(item) = stream.next().await { … }`). CF KV/D1 impls land
    /// in F23 — OD-06-7.
    async fn scan_from<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
        from_id: &str,
        limit: usize,
    ) -> StorageResult<AsyncStorageItemStream<'_, V>>;

    /// Delete a specific document.
    async fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()>;

    /// Delete all documents in a collection.
    async fn delete_all(&self, key: &str) -> StorageResult<u64>;

    /// Count documents in a collection.
    async fn count(&self, key: &str) -> StorageResult<u64>;
}

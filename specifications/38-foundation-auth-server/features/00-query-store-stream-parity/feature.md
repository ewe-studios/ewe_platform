# Feature 00: QueryStore Stream Parity

## Description

Fix the fundamental API mismatch in `foundation_db` between sync and async query traits. Currently `QueryStore::query()` (sync) returns a valtron `StorageItemStream` (StreamIterator — composable, progress-driven, incremental), while `AsyncQueryStore::query_async()` returns `StorageResult<Vec<SqlRow>>` (all-or-nothing Vec). The async version is at fault — it defeats the purpose of streaming for multi-row results.

This feature introduces `AsyncQueryStream` — an async stream type that wraps valtron's StreamIterator as a `futures_core::Stream`, providing parity between sync and async query operations.

## Prerequisites

This is a **preceding feature** — it must be implemented before the IdP service features (10-12) that depend on `AsyncQueryStore`.

## Why This Matters

The whole point of streaming is to support multiple items without buffering everything into memory. If the async version returns a `Vec`, it:
- Defeats the purpose of streaming for large result sets
- Breaks composability — callers can't transform rows as they arrive
- Creates inconsistent APIs where sync and async behave fundamentally differently

## Modules

- `backends/foundation_db/src/core/storage_provider.rs` — add `AsyncQueryStream` type, update `AsyncQueryStore` trait
- `backends/foundation_db/src/backends/turso/` — update Turso backend to return `AsyncQueryStream`
- `backends/foundation_db/src/backends/libsql/` — update libsql backend to return `AsyncQueryStream`
- `backends/foundation_db/src/backends/d1_wasm/` — update D1 wasm backend to return `AsyncQueryStream`

## API Changes

### New Type: AsyncQueryStream

```rust
use futures_core::Stream;
use foundation_core::valtron::{StreamAsFutureStream, StreamIterator, StorageItemStream};

/// An async stream of query results.
///
/// Created by `AsyncQueryStore::query_async()`. Implements `futures_core::Stream`
/// so it can be used with `.next().await` in async code, or collected with
/// `.collect::<Vec<_>>().await` when all rows are needed at once.
///
/// Internally bridges valtron's `StreamIterator` (sync) to async via
/// `StreamAsFutureStream`, which uses `wake_by_ref()` for immediate re-polling.
pub struct AsyncQueryStream<T> {
    inner: StreamAsFutureStream<StorageItemStream<'static, T>>,
}

impl<T: Unpin + Send> AsyncQueryStream<T> {
    /// Wrap a valtron StorageItemStream into an async stream.
    pub fn new(stream: StorageItemStream<'_, T>) -> Self {
        // Safety: we transmute the lifetime to 'static because the stream
        // is moved into the async context where it's owned.
        Self {
            inner: StreamAsFutureStream::new(unsafe {
                std::mem::transmute::<StorageItemStream<'_, T>, StorageItemStream<'static, T>>(stream)
            }),
        }
    }

    /// Collect all remaining items into a Vec.
    /// Convenience for callers who want all rows at once.
    pub async fn collect_all(self) -> StorageResult<Vec<T>> {
        use futures_lite::stream::StreamExt;
        let mut results = Vec::new();
        let mut stream = self;
        while let Some(item) = stream.next().await {
            results.push(item?);
        }
        Ok(results)
    }

    /// Get the next item, or None when exhausted.
    pub async fn next_item(&mut self) -> Option<StorageResult<T>> {
        use futures_lite::stream::StreamExt;
        self.inner.next().await
    }
}

impl<T: Unpin + Send> Stream for AsyncQueryStream<T> {
    type Item = StorageResult<T>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_next(cx)
    }
}
```

### Updated AsyncQueryStore Trait

```rust
#[async_trait::async_trait(?Send)]
pub trait AsyncQueryStore {
    /// Execute a query that returns rows as an async stream.
    ///
    /// Callers can:
    /// - Process rows one at a time: `while let Some(row) = stream.next_item().await { ... }`
    /// - Collect all rows: `let rows = stream.collect_all().await?;`
    /// - Use as a `futures_core::Stream` with any async stream combinators.
    async fn query_async(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<AsyncQueryStream<SqlRow>>;

    /// Execute a statement that returns number of rows affected.
    /// Single-value operation — Result is acceptable (no streaming needed).
    async fn execute_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64>;

    /// Execute a batch of SQL statements.
    /// Single-value operation — Result is acceptable (no streaming needed).
    async fn execute_batch_async(&self, sql: &str) -> StorageResult<()>;
}
```

### Updated AsyncKeyValueStore (List parity)

```rust
#[async_trait::async_trait(?Send)]
pub trait AsyncKeyValueStore {
    async fn get_async<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>>;
    async fn set_async<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()>;
    async fn delete_async(&self, key: &str) -> StorageResult<()>;
    async fn exists_async(&self, key: &str) -> StorageResult<bool>;

    /// List all keys with optional prefix filter.
    /// Returns an async stream — callers can iterate or collect.
    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream<String>>;
}

/// Async stream for KV list operations (same pattern as AsyncQueryStream).
pub struct AsyncListStream<T> {
    inner: StreamAsFutureStream<StorageItemStream<'static, T>>,
}
```

### Implementation for Native Backends

For native backends (Turso, libsql), the implementation calls the existing sync `QueryStore::query()` and wraps the result:

```rust
#[async_trait::async_trait(?Send)]
impl AsyncQueryStore for TursoStore {
    async fn query_async(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<AsyncQueryStream<SqlRow>> {
        // Call the existing sync query (which uses valtron internally)
        let stream = self.query(sql, params)?;
        Ok(AsyncQueryStream::new(stream))
    }

    async fn execute_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        let stream = self.execute(sql, params)?;
        // Single-value — collect_one
        collect_one(stream)
            .ok_or_else(|| StorageError::NoResult)
            .and_then(|r| r)
    }

    async fn execute_batch_async(&self, sql: &str) -> StorageResult<()> {
        let stream = self.execute_batch(sql)?;
        collect_one(stream)
            .ok_or_else(|| StorageError::NoResult)
            .and_then(|r| r)
    }
}
```

### Implementation for WASM Backend (D1)

For the D1 wasm backend, the implementation is different because the underlying JS API returns all rows at once via Promise. The `AsyncQueryStream` wraps a valtron stream created from the Promise result:

```rust
#[async_trait::async_trait(?Send)]
impl AsyncQueryStore for D1WasmStore {
    async fn query_async(
        &self,
        sql: &str,
        params: &[DataValue],
    ) -> StorageResult<AsyncQueryStream<SqlRow>> {
        // Execute the D1 query via JsFuture
        let rows = self.execute_d1_query(sql, params).await?;

        // Create a valtron stream from the Vec using from_iter
        let stream = from_iter(rows.into_iter().map(Ok::<_, StorageError>));
        Ok(AsyncQueryStream::new(stream))
    }

    // execute_async and execute_batch_async similar
}
```

## Sync/Async Parity Summary

| Operation | Sync API | Async API | Symmetric? |
|-----------|----------|-----------|------------|
| Query (multi-row) | `StorageItemStream<SqlRow>` | `AsyncQueryStream<SqlRow>` | ✅ Both stream |
| Execute (single) | `StorageItemStream<u64>` | `StorageResult<u64>` | ✅ Single value — Result OK |
| Batch (single) | `StorageItemStream<()>` | `StorageResult<()>` | ✅ Single value — Result OK |
| KV Get (single) | `StorageItemStream<Option<V>>` | `StorageResult<Option<V>>` | ✅ Single value — Result OK |
| KV List (multi) | `StorageItemStream<String>` | `AsyncListStream<String>` | ✅ Both stream |

## Dependencies

- `futures-core` — for `futures_core::Stream` trait (already in workspace)
- `futures-lite` — for `StreamExt` (already in workspace)
- Existing: `foundation_core` valtron types (`StreamAsFutureStream`, `StorageItemStream`)

## Testing

- `AsyncQueryStream` from sync backend: iterate row-by-row via `.next_item().await`
- `AsyncQueryStream` from sync backend: collect all via `.collect_all().await`
- `AsyncQueryStream` from wasm backend: same patterns work
- Parity: sync `collect_one(query())` produces same results as `query_async().await.collect_all()`
- Empty result set: stream yields None immediately
- Large result sets: memory usage is bounded (rows processed incrementally, not all buffered)

# Feature 00: Storage Trait Stream Parity — Async Primary, Sync Wraps

## Context

`foundation_db` has sync/async duplication in multiple traits. The audit found **3 problem areas**:

### Problem 1: `QueryStore::query` / `AsyncQueryStore::query_async`

| Backend | Sync | Async | Status |
|---------|------|-------|--------|
| Turso | Own logic (`run_future_iter` + `RowsIterator`) | Own logic (`query_async_internal`, collects to `Vec`) | ❌ **Duplication** |
| Libsql | Own logic (`run_future_iter` + `LibsqlRowsIterator`) | Own logic (`query_async_internal`, collects to `Vec`) | ❌ **Duplication** |
| D1Store | Own logic (HTTP → collect → `wrap_vec`) | Own logic (HTTP → collect → `Vec`) | ❌ **Duplication** |
| D1Wasm | Calls async via `exec_future` | Own logic (JS Promise → collect) | ✅ **Correct** |

Both Turso/Libsql sync and async have their own query logic. The async collects to `Vec` when it should stream.

### Problem 2: `KeyValueStore::list_keys` / `AsyncKeyValueStore::list_keys_async`

| Backend | Sync | Async | Status |
|---------|------|-------|--------|
| Turso | Own logic (`run_future_iter` + `RowsIterator`) | Calls **sync** `list_keys` and collects! | ❌ **Backwards + duplication** |
| Libsql | Own logic (`run_future_iter` + `LibsqlRowsIterator`) | Calls **sync** `list_keys` and collects! | ❌ **Backwards + duplication** |

The async calls sync, which is backwards. The sync duplicates query logic.

### Problem 3: Other traits

| Trait | Status |
|-------|--------|
| KV get/set/delete/exists (Turso/Libsql) | ✅ Sync calls `*_async_internal` via `wrap_async` |
| QueryStore execute/execute_batch (Turso/Libsql) | ✅ Sync calls `*_async_internal` via `wrap_async` |
| BlobStore (all) | ✅ Sync calls `*_async_internal` via `wrap_async` |
| RateLimiterStore (all) | ✅ Sync calls `*_async_internal` via `wrap_async` |
| D1Wasm (all) | ✅ Sync calls async via `schedule_future`/`exec_future` |

## Design: Async Primary, Sync Wraps via valtron

### The Pattern

**For Turso/Libsql (!Send rows):**
- Async owns the logic, returns `AsyncQueryStream` wrapping the `Rows` type as `futures_core::Stream`
- Sync calls async, uses `run_future_iter` because the stream is `!Send` (it holds the `Rows`)
- `run_future_iter` owns the `!Send` stream on a worker thread forever, bridges via `ThreadedValue`

**For D1 backends (rows arrive all at once):**
- Async owns the logic, returns `AsyncQueryStream` buffered internally
- Sync calls async via `from_future` + `execute` (no `!Send` concern)

### Step 1: New Stream Types

```rust
use futures_core::Stream;
use futures_lite::stream::StreamExt;

/// Async stream of SQL rows.
///
/// For Turso/Libsql: yields rows one at a time as they're fetched from the DB.
/// For D1 backends: buffered (API returns all rows at once).
pub struct AsyncQueryStream {
    inner: Pin<Box<dyn Stream<Item = StorageResult<SqlRow>> + Send>>,
}

impl AsyncQueryStream {
    pub fn new<S>(stream: S) -> Self
    where S: Stream<Item = StorageResult<SqlRow>> + Send + 'static
    {
        Self { inner: Box::pin(stream) }
    }

    /// Collect all rows. Convenience for callers.
    pub async fn collect_all(self) -> StorageResult<Vec<SqlRow>> {
        self.inner.collect::<Vec<_>>().await.into_iter().collect()
    }
}

impl Stream for AsyncQueryStream {
    type Item = StorageResult<SqlRow>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_next(cx)
    }
}

/// Async stream of KV list keys. Same pattern as AsyncQueryStream.
pub struct AsyncListStream {
    inner: Pin<Box<dyn Stream<Item = StorageResult<String>> + Send>>,
}

impl AsyncListStream {
    pub fn new<S>(stream: S) -> Self
    where S: Stream<Item = StorageResult<String>> + Send + 'static
    {
        Self { inner: Box::pin(stream) }
    }

    pub async fn collect_all(self) -> StorageResult<Vec<String>> {
        self.inner.collect::<Vec<_>>().await.into_iter().collect()
    }
}

impl Stream for AsyncListStream {
    type Item = StorageResult<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_next(cx)
    }
}
```

### Step 2: Turso — Async Primary, Sync Wraps

**AsyncQueryStream from Rows** — use the `async-stream` crate's `try_stream!` macro:

```rust
#[cfg(feature = "async-stream")]
use async_stream::try_stream;

async fn query_rows_stream(
    conn: Arc<turso::Connection>,
    sql: String,
    params: Vec<turso::Value>,
) -> impl Stream<Item = StorageResult<SqlRow>> + Send {
    let mut stmt = match conn.prepare(&sql).await {
        Ok(s) => s,
        Err(e) => { yield Err(StorageError::Backend(e.to_string())); return; }
    };
    let mut rows = match stmt.query(params).await {
        Ok(r) => r,
        Err(e) => { yield Err(StorageError::Backend(e.to_string())); return; }
    };

    loop {
        match rows.next().await {
            Ok(Some(row)) => {
                let col_count = row.column_count() as i32;
                yield Self::turso_row_to_sql_row(&row, col_count);
            }
            Ok(None) => break,
            Err(e) => {
                yield Err(StorageError::Backend(e.to_string()));
                break;
            }
        }
    }
}
```

Same for list_keys:

```rust
async fn list_keys_stream(
    conn: Arc<turso::Connection>,
    sql: String,
    param: turso::Value,
) -> impl Stream<Item = StorageResult<String>> + Send {
    let mut stmt = match conn.prepare(&sql).await {
        Ok(s) => s,
        Err(e) => { yield Err(StorageError::Backend(e.to_string())); return; }
    };
    let mut rows = match stmt.query([param]).await {
        Ok(r) => r,
        Err(e) => { yield Err(StorageError::Backend(e.to_string())); return; }
    };

    while let Ok(Some(row)) = rows.next().await {
        match row.get::<String>(0) {
            Ok(key) => yield Ok(key),
            Err(e) => yield Err(StorageError::SqlConversion(e.to_string())),
        }
    }
}
```

**Updated `AsyncQueryStore` impl:**

```rust
#[async_trait::async_trait(?Send)]
impl AsyncQueryStore for TursoStorage {
    async fn query_async(
        &self, sql: &str, params: &[DataValue],
    ) -> StorageResult<AsyncQueryStream> {
        let conn = Arc::clone(&self.conn);
        let sql = sql.to_string();
        let params = Self::to_turso_params(params);
        let stream = query_rows_stream(conn, sql, params).await;
        Ok(AsyncQueryStream::new(stream))
    }

    async fn execute_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        self.execute_async_internal(sql, params).await  // single value, OK
    }
    async fn execute_batch_async(&self, sql: &str) -> StorageResult<()> {
        self.execute_batch_async_internal(sql).await  // single value, OK
    }
}
```

**Updated `AsyncKeyValueStore` impl:**

```rust
impl AsyncKeyValueStore for TursoStorage {
    async fn get_async<V: DeserializeOwned + Send + 'static>(
        &self, key: &str,
    ) -> StorageResult<Option<V>> {
        self.get_async_internal(key).await
    }
    // ... set, delete, exists unchanged — single value

    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream> {
        let (sql, param) = match prefix {
            Some(p) => ("SELECT key FROM kv_store WHERE key LIKE ? ORDER BY key", p.to_string()),
            None => ("SELECT key FROM kv_store ORDER BY key", String::new()),
        };
        let conn = Arc::clone(&self.conn);
        let param = Self::to_turso_value(param);
        let stream = list_keys_stream(conn, sql.to_string(), param).await;
        Ok(AsyncListStream::new(stream))
    }
}
```

**Updated sync `QueryStore` — wraps async via `run_future_iter`:**

```rust
impl QueryStore for TursoStorage {
    fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        let this = self.clone();
        let sql = sql.to_string();
        let params = params.to_vec();

        let iter = run_future_iter(move || async move {
            let stream = this.query_async(&sql, &params).await?;
            // Bridge the !Send AsyncQueryStream to a sync iterator
            Ok::<_, StorageError>(AsyncStreamIterator::new(stream))
        }, None, None)?;

        let stream = iter.map(|tv| match tv {
            ThreadedValue::Value(result) => Stream::Next(result),
            ThreadedValue::Waiting => Stream::Pending(()),
        });
        Ok(Box::new(stream))
    }

    fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, u64>> {
        Self::wrap_async(async move { self.execute_async_internal(sql, params).await })
    }

    fn execute_batch(&self, sql: &str) -> StorageResult<StorageItemStream<'_, ()>> {
        Self::wrap_async(async move { self.execute_batch_async_internal(sql).await })
    }
}
```

**Updated sync `KeyValueStore::list_keys` — wraps async via `run_future_iter`:**

```rust
impl KeyValueStore for TursoStorage {
    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let this = self.clone();
        let prefix = prefix.map(String::from);

        let iter = run_future_iter(move || async move {
            let stream = this.list_keys_async(prefix.as_deref()).await?;
            Ok::<_, StorageError>(AsyncStreamIterator::new(stream))
        }, None, None)?;

        let stream = iter.map(|tv| match tv {
            ThreadedValue::Value(result) => Stream::Next(result),
            ThreadedValue::Waiting => Stream::Pending(()),
        });
        Ok(Box::new(stream))
    }
    // ... get/set/delete/exists unchanged
}
```

**AsyncStreamIterator** — bridges `AsyncQueryStream`/`AsyncListStream` to a sync Iterator:

```rust
/// Wraps an async stream as a sync iterator.
/// Used inside `run_future_iter` worker threads where `block_on` is available.
pub struct AsyncStreamIterator<T> {
    stream: AsyncQueryStream,  // or AsyncListStream<T>
    _marker: PhantomData<T>,
}

impl<T: Send + 'static> Iterator for AsyncStreamIterator<T> {
    type Item = Result<T, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        // block_on works because we're inside run_future_iter's worker thread
        block_on(async { self.stream.next().await })
    }
}
```

### Step 3: Libsql — Same Pattern

Identical to Turso. Replace `turso::` with `libsql::` and `RowsIterator` with `LibsqlRowsIterator`.

### Step 4: D1Store (native HTTP) — Buffered Stream

D1 HTTP API returns all rows at once, so we buffer but still present a stream API:

```rust
#[async_trait::async_trait(?Send)]
impl AsyncQueryStore for D1Store {
    async fn query_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<AsyncQueryStream> {
        let rows = self.do_query(sql, params)?;  // existing HTTP call
        let stream = futures_lite::stream::iter(rows.into_iter().map(Ok));
        Ok(AsyncQueryStream::new(stream))
    }

    async fn execute_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        // existing logic — single value, OK
    }
    async fn execute_batch_async(&self, sql: &str) -> StorageResult<()> {
        // existing logic — single value, OK
    }
}

impl QueryStore for D1Store {
    fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        let this = self.clone();
        let sql = sql.to_string();
        let params = params.to_vec();

        // No !Send concern — D1 rows are just Vec<SqlRow>
        // Use from_future + execute to bridge
        Self::wrap_vec_stream(async move {
            this.query_async(&sql, &params).await?.collect_all().await
        })
    }

    // ... execute/execute_batch unchanged
}
```

### Step 5: D1Wasm — Already Correct, Minor Updates

D1Wasm already wraps async via valtron in sync. Just update the return types:

```rust
#[async_trait::async_trait(?Send)]
impl AsyncQueryStore for D1WasmStorage {
    async fn query_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<AsyncQueryStream> {
        let rows = Self::do_query_rows_async(&self.db, sql, params).await?;
        let stream = futures_lite::stream::iter(rows.into_iter().map(Ok));
        Ok(AsyncQueryStream::new(stream))
    }
    // ...
}

// Sync already wraps async via exec_future — just update return type handling
```

## Updated `AsyncQueryStore` / `AsyncKeyValueStore` Traits

```rust
#[async_trait::async_trait(?Send)]
pub trait AsyncQueryStore {
    async fn query_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<AsyncQueryStream>;
    async fn execute_async(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64>;
    async fn execute_batch_async(&self, sql: &str) -> StorageResult<()>;
}

#[async_trait::async_trait(?Send)]
pub trait AsyncKeyValueStore {
    async fn get_async<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>>;
    async fn set_async<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()>;
    async fn delete_async(&self, key: &str) -> StorageResult<()>;
    async fn exists_async(&self, key: &str) -> StorageResult<bool>;
    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream>;
}
```

## Files Modified

1. **`backends/foundation_db/src/core/storage_provider.rs`**
   - Add `AsyncQueryStream`, `AsyncListStream` types
   - Update `AsyncQueryStore::query_async` → returns `AsyncQueryStream`
   - Update `AsyncKeyValueStore::list_keys_async` → returns `AsyncListStream`

2. **`backends/foundation_db/src/native/turso_backend.rs`**
   - Add `query_rows_stream`, `list_keys_stream` async stream generators
   - Rewrite `AsyncQueryStore::query_async` → returns stream
   - Rewrite `AsyncKeyValueStore::list_keys_async` → returns stream
   - Rewrite `QueryStore::query` → wraps async via `run_future_iter`
   - Rewrite `KeyValueStore::list_keys` → wraps async via `run_future_iter`
   - Remove `query_async_internal`, `RowsIterator` duplication

3. **`backends/foundation_db/src/native/libsql_store.rs`**
   - Same pattern as Turso

4. **`backends/foundation_db/src/native/d1_kvstore.rs`**
   - Update `AsyncQueryStore::query_async` → returns buffered stream
   - Update sync `query` → wraps async

5. **`backends/foundation_db/src/wasm/wasm_storage/d1_wasm.rs`**
   - Update `AsyncQueryStore::query_async` → returns buffered stream
   - Sync already correct, just adapt to new types

6. **`backends/foundation_db/src/native/rows_stream.rs`**
   - Remove `RowsIterator` and `LibsqlRowsIterator` (no longer needed — async stream handles it)
   - Keep `AsyncStreamIterator` for sync-to-async bridging

7. **`backends/foundation_db/Cargo.toml`**
   - Add `async-stream = "0.3"` dependency

## Dependencies

- `async-stream = "0.3"` — for `try_stream!` macro to create async streams
- `futures-core` — for `futures_core::Stream` trait (already in workspace via tokio-stream)
- `futures-lite` — for `StreamExt` and `block_on` (already present)

## Tests

- **Parity**: `collect_result(sync_query())` == `async_query_async().collect_all().await` for all backends
- **Streaming**: Turso/Libsql large result set doesn't buffer in memory (async yields row-by-row)
- **Empty**: zero-row query returns empty stream for both sync and async
- **Error**: failing query returns error in stream for both sync and async
- **list_keys parity**: sync `collect_result(list_keys())` == async `list_keys_async().collect_all().await`
- **Existing tests must pass** — update any test that expected `Vec<SqlRow>` from `query_async`

## Migration Note

Breaking change for `AsyncQueryStore::query_async` consumers:

```rust
// OLD:
let rows: Vec<SqlRow> = store.query_async(sql, params).await?;

// NEW:
let rows = store.query_async(sql, params).await?.collect_all().await?;
```

Breaking change for `AsyncKeyValueStore::list_keys_async`:

```rust
// OLD:
let keys: Vec<String> = store.list_keys_async(prefix).await?;

// NEW:
let keys = store.list_keys_async(prefix).await?.collect_all().await?;
```

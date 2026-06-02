---
feature: "Async Store Rework — True Async-First Implementations"
description: "Rewrite AsyncKeyValueStore/AsyncBlobStore/AsyncQueryStore/AsyncRateLimiterStore to be truly async-first, with sync traits wrapping async via valtron from_future. Add send_async() to SimpleHttpClient, AsyncSendSafeBody body reader, and native async SQL/HTTP helpers. D1/R2 use native async APIs, Turso/libsql use native crate async APIs."
status: "implemented"
priority: "high"
depends_on: ["11-valtron-async-bridge"]
estimated_effort: "large"
created: 2026-05-30
last_updated: 2026-05-30
author: "Main Agent"
tasks:
  completed: 7
  uncompleted: 0
  total: 7
  completion_percentage: 100%
---

# Feature: Async Store Rework — True Async-First Implementations

## Overview

The current `AsyncKeyValueStore`, `AsyncBlobStore`, `AsyncQueryStore`, and `AsyncRateLimiterStore` implementations in **TursoStorage**, **LibsqlStore**, **D1Store**, and **R2Store** are structurally inverted — the `async fn` bodies are fully synchronous, blocking the thread before ever yielding.

## Current Architecture (Broken)

```
async fn get_async() → <Self as KeyValueStore>::get() → collect_one(stream) → return
                         │
                         └─ schedule_future(async_db_op)  -- Future → Valtron Stream
                            │
                            └─ execute(task) → drive iterator synchronously → collect_one blocks
```

Problems:

1. **`async fn` body is synchronous** — `collect_one(stream)` drives the `StorageItemStream` iterator to completion on the calling thread. The async fn never yields; it blocks then returns.
2. **Double wrapping** — async db op → `schedule_future` → `StorageItemStream` → `collect_one` → `Result<T>`. The async→Stream→Future roundtrip is backwards.
3. **D1/R2 `SimpleHttpClient::send()` is fully blocking** — the HTTP client drives the entire request-response cycle synchronously.
4. **Body reading is manual** — D1Store/R2Store match on `SendSafeBody` variants manually (`Text` → clone, `Bytes` → UTF-8, etc.). `body_reader.rs` already has `collect_bytes_from_send_safe` and `try_collect_string` for sync code, but nothing for async.

## Target Architecture (Async-First)

```
async fn get_async() → execute_sql_async(sql) → await → return Result<T>     ← truly async
                                                    │
fn get() → from_future(async move { execute_sql_async().await }) → execute(task) → StorageItemStream
          └─ wraps async as Valtron FutureTask       │
             └─ lazy, streamable, non-blocking      └─ schedules on thread pool
```

---

## Phase 1: SimpleHttpClient `send_async()` in foundation_netio

**File:** `backends/foundation_netio/src/simple_http/client/native/api.rs`

`ClientRequest::start()` returns `(RequestIntroStream, MappedDrivenBodyStream<R>)` — both are `StreamIterator`s. Feature 11 provides `into_ready_future()` to convert any `StreamIterator` into a standard Rust `Future`.

```rust
impl<R: DnsResolver + 'static> ClientRequest<R> {
    /// Asynchronous version of `send()`.
    pub async fn send_async(mut self) -> Result<FinalizedResponse<SendSafeBody, R>, HttpClientError> {
        let (mut intro_stream, mut body_stream) = self.start()?;

        // Drive body first (required by split_collect_one_map ordering).
        let body_result = body_stream
            .into_ready_future()
            .await
            .ok_or(HttpClientError::InvalidRequestState)?;
        let (conn, body) = body_result.0?;

        // Drive intro stream.
        let intro_result = intro_stream
            .into_ready_future()
            .await
            .ok_or(HttpClientError::InvalidRequestState)?;
        let (intro, headers) = intro_result.0?;

        let mut response = SimpleResponse::new(intro.status, headers, body);

        if let Some(request) = &self.original_request {
            self.middleware_chain.process_response(request, &mut response)?;
        }

        Ok(FinalizedResponse::new(response, conn, self.pool.clone()))
    }
}
```

That's it. Both `into_ready_future()` return `impl Future<Output = Option<(T, SI)>>` from feature 11. No new structs, no new types — just calling existing methods.

---

## Phase 2: `AsyncSendSafeBody` — async body reader

**Files:** `backends/foundation_netio/src/simple_http/shared/impls.rs` (`SendSafeBody::to_async`), `backends/foundation_netio/src/simple_http/client/shared/body_reader.rs` (`AsyncSendSafeBody` struct, async helpers)

`SendSafeBody` contains `Option<Box<dyn Iterator>>` for stream variants — not a valtron `StreamIterator`. We need a wrapper that owns the `SendSafeBody` and implements `futures_core::Stream<Item = Result<Vec<u8>, BoxedError>>` so body data can be read in async contexts.

### `SendSafeBody::to_async(self)` convenience method

Added to `impl SendSafeBody` in `impls.rs`:

```rust
impl SendSafeBody {
    /// Consume this body and return an async stream yielding byte chunks.
    ///
    /// - `Text` / `Bytes` → yields one chunk then ends.
    /// - `Stream` / `ChunkedStream` / `LineFeedStream` / `SseStream` → yields each chunk from the inner iterator.
    /// - `None` → yields nothing.
    pub fn to_async(self) -> AsyncSendSafeBody {
        AsyncSendSafeBody::from(self)
    }
}
```

This lets callers write `body.to_async()` instead of `AsyncSendSafeBody::from(body)` — the most ergonomic path.

### `AsyncSendSafeBody` struct

```rust
use futures_core::Stream as FuturesStream;

/// Async stream that reads body bytes from a `SendSafeBody`.
///
/// - `Text` / `Bytes` → yields one chunk then ends.
/// - `Stream` / `ChunkedStream` / `LineFeedStream` / `SseStream` → yields each chunk from the inner iterator.
/// - `None` → yields nothing.
///
/// Since the inner iterators are synchronous, each poll returns `Poll::Ready`
/// (data available or exhausted). The wrapper implements `futures_core::Stream`
/// so it integrates with `.collect().await`, `StreamExt`, etc.
pub struct AsyncSendSafeBody {
    inner: SendSafeBody,
}

impl From<SendSafeBody> for AsyncSendSafeBody {
    fn from(body: SendSafeBody) -> Self {
        Self { inner: body }
    }
}

impl FuturesStream for AsyncSendSafeBody {
    type Item = Result<Vec<u8>, BoxedError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Match on inner variant:
        //   Text(t)  → first poll: Ready(Some(t.into_bytes())), then take → Done
        //   Bytes(b) → first poll: Ready(Some(b)), then take → Done
        //   None     → Ready(None)
        //   Stream(Some(iter)) → iter.next() → Ready(Some(chunk)) or Ready(None)
        //   ChunkedStream(Some(iter)) → iter.next() → Ready(Some(data)) or Ready(None)
        //   LineFeedStream(Some(iter)) → iter.next() → Ready(Some(line_bytes)) or Ready(None)
        //   SseStream(Some(iter)) → iter.next() → Ready(Some(event_data)) or Ready(None)
    }
}
```

### Async body reading helpers

Alongside the stream wrapper, add async convenience functions:

```rust
/// Collect all body bytes from an `AsyncSendSafeBody` in async context.
pub async fn collect_bytes_async(body: AsyncSendSafeBody) -> Result<Vec<u8>, BoxedError> {
    use futures_util::TryStreamExt;
    body.try_concat().await
}

/// Collect body as a String in async context.
pub async fn collect_string_async(body: AsyncSendSafeBody) -> Result<String, BoxedError> {
    let bytes = collect_bytes_async(body).await?;
    String::from_utf8(bytes).map_err(|e| Box::new(e) as BoxedError)
}
```

These mirror the existing sync `collect_bytes_from_send_safe` / `try_collect_string` but work in async contexts and properly handle all `SendSafeBody` variants including streaming ones.

### Usage in `send_async`

After `send_async()` returns a `FinalizedResponse<SendSafeBody, R>`, the caller extracts the body and reads it async:

```rust
let response = client.post(url)?.send_async().await?;
let (_, _, body, ..) = response.into_parts();
let text = collect_string_async(body.to_async()).await?;
```

---

## Phase 3: D1Store — Native async `execute_sql_async`

**File:** `backends/foundation_db/src/native/d1_kvstore.rs`

```rust
impl D1Store {
    async fn execute_sql_async(&self, sql: &str, params: &[serde_json::Value]) -> Result<serde_json::Value, StorageError> {
        let body = serde_json::json!({ "sql": sql, "params": params });
        let response = self
            .client
            .post(&self.query_url())?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header())
            .header(SimpleHeader::CONTENT_TYPE, "application/json")
            .body_text(body.to_string())
            .build_client()?
            .send_async()
            .await?;

        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(
                format!("D1 query failed with status {}", response.get_status())
            ));
        }

        let (_, _, body, ..) = response.into_parts();
        let text = collect_string_async(body.to_async())
            .await
            .map_err(|e| StorageError::Backend(format!("D1 response read failed: {e}")))?;

        serde_json::from_str(&text)
            .map_err(|e| StorageError::Serialization(format!("D1 response parse failed: {e}")))
    }
}
```

D1Store needs `Clone` (`SimpleHttpClient` is `Arc`-based, `ZeroizingString` implements `Clone`).

---

## Phase 4: R2Store — Native async HTTP helpers

**File:** `backends/foundation_db/src/native/r2_blobstore.rs`

```rust
impl R2Store {
    async fn get_object_async(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let url = self.object_url(key);
        let response = self.client.get(&url)?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .build_client()?
            .send_async()
            .await?;

        if response.get_status() == Status::NotFound { return Ok(None); }
        if response.get_status() != Status::OK {
            return Err(StorageError::Backend(format!("R2 GET failed: {}", response.get_status())));
        }
        let (_, _, body, ..) = response.into_parts();
        let bytes = collect_bytes_async(body.to_async())
            .await
            .map_err(|e| StorageError::Backend(format!("R2 body read failed: {e}")))?;
        Ok(Some(bytes))
    }

    async fn put_object_async(&self, key: &str, data: &[u8], content_type: &str) -> Result<(), StorageError> {
        let url = self.object_url(key);
        let response = self.client.put(&url)?
            .header(SimpleHeader::AUTHORIZATION, self.auth_header_bearer())
            .header(SimpleHeader::CONTENT_TYPE, content_type)
            .body_bytes(data.to_vec())
            .build_client()?
            .send_async()
            .await?;
        let status: usize = response.get_status().into();
        if status >= 400 {
            return Err(StorageError::Backend(format!("R2 PUT failed: {}", response.get_status())));
        }
        Ok(())
    }

    async fn delete_object_async(&self, key: &str) -> Result<(), StorageError> { /* same pattern */ }
    async fn head_object_async(&self, key: &str) -> Result<bool, StorageError> { /* same pattern */ }
    async fn list_objects_async(&self, prefix: &str) -> Result<serde_json::Value, StorageError> { /* collect_string_async + parse */ }
}
```

R2Store needs `Clone` too.

---

## Phase 5: Rewrite Async trait implementations

### D1Store — AsyncKeyValueStore

**File:** `backends/foundation_db/src/native/d1_kvstore.rs`

```rust
#[async_trait::async_trait(?Send)]
impl AsyncKeyValueStore for D1Store {
    async fn get_async<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>> {
        let sql = format!("SELECT value FROM {} WHERE key = ?", self.kv_table());
        let response = self.execute_sql_async(&sql, &[serde_json::Value::String(key.to_string())]).await?;
        let rows = Self::extract_rows(&response);
        match rows.first() {
            Some(row) => {
                let value: String = row.get("value").and_then(serde_json::Value::as_str).map(String::from)
                    .ok_or_else(|| StorageError::SqlConversion("missing value".into()))?;
                Ok(Some(serde_json::from_str(&value)?))
            }
            None => Ok(None),
        }
    }

    async fn set_async<V: Serialize>(&self, key: &str, value: V) -> StorageResult<()> {
        let json_value = serde_json::to_string(&value)?;
        let sql = format!("INSERT INTO {} (key, value, updated_at) VALUES (?, ?, strftime('%s', 'now') * 1000) ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = strftime('%s', 'now') * 1000", self.kv_table());
        let kv = serde_json::Value::String(key.to_string());
        let jv = serde_json::Value::String(json_value);
        self.execute_sql_async(&sql, &[kv.clone(), jv.clone(), jv]).await?;
        Ok(())
    }
    // delete_async, exists_async, list_keys_async follow same pattern
}
```

Same pattern for `AsyncQueryStore`, `AsyncRateLimiterStore`, `AsyncBlobStore`.

### R2Store — AsyncBlobStore

```rust
#[async_trait::async_trait(?Send)]
impl AsyncBlobStore for R2Store {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> StorageResult<()> {
        self.put_object_async(&self.blob_object_key(key), data, "application/octet-stream").await
    }
    async fn get_blob_async(&self, key: &str) -> StorageResult<Option<Vec<u8>>> {
        self.get_object_async(&self.blob_object_key(key)).await
    }
    async fn delete_blob_async(&self, key: &str) -> StorageResult<()> {
        self.delete_object_async(&self.blob_object_key(key)).await
    }
    async fn blob_exists_async(&self, key: &str) -> StorageResult<bool> {
        self.head_object_async(&self.blob_object_key(key)).await
    }
}
```

### TursoStorage and LibsqlStore

Extract core async logic into `async fn get_async_internal<V>()` methods. Async traits call these directly.

---

## Phase 6: Rewrite sync traits to call async via `from_future`

### D1Store KeyValueStore

```rust
use foundation_core::valtron::{from_future, execute, StreamIteratorExt};

impl KeyValueStore for D1Store {
    fn get<'a, V: DeserializeOwned + Send + 'static>(&'a self, key: &str) -> StorageResult<StorageItemStream<'a, Option<V>>> {
        let key = key.to_string();
        let this = self.clone();

        let future = async move {
            let sql = format!("SELECT value FROM {} WHERE key = ?", this.kv_table());
            let response = this.execute_sql_async(&sql, &[serde_json::Value::String(key)]).await?;
            let rows = D1Store::extract_rows(&response);
            match rows.first() {
                Some(row) => {
                    let value: String = row.get("value")
                        .and_then(serde_json::Value::as_str).map(String::from)
                        .ok_or_else(|| StorageError::SqlConversion("missing value".into()))?;
                    Ok(Some(serde_json::from_str(&value)?))
                }
                None => Ok(None),
            }
        };

        let task = from_future(future);
        let stream = execute(task, None)
            .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;

        Ok(Box::new(
            stream
                .map_done(|r: Result<Option<V>, StorageError>| r)
                .map_pending(|_| ()),
        ))
    }
    // set, delete, exists, list_keys follow same pattern
}
```

### R2Store BlobStore

```rust
impl BlobStore for R2Store {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<StorageItemStream<'_, ()>> {
        let this = self.clone();
        let key = self.blob_object_key(key).to_string();
        let data = data.to_vec();
        let future = async move {
            this.put_object_async(&key, &data, "application/octet-stream").await
        };
        let task = from_future(future);
        let stream = execute(task, None)?;
        Ok(Box::new(
            stream.map_done(|r: Result<(), StorageError>| r).map_pending(|_| ()),
        ))
    }
    // get_blob, delete_blob, blob_exists follow same pattern
}
```

### TursoStorage and LibsqlStore

Sync trait methods wrap `async move { self.get_async_internal(&key).await }` via `from_future` → `execute`.

---

## Phase 7: Clone implementations

Add `#[derive(Clone)]` to `D1Store` and `R2Store`:
- `SimpleHttpClient` is already `Clone` (`Arc`-based internally)
- `ZeroizingString` implements `Clone`
- TursoStorage and LibsqlStore already `Clone` via `Arc<Connection>`

---

## File-by-File Changes

| File | Change |
|------|--------|
| `foundation_netio/.../client/native/api.rs` | Add `send_async()` using existing `into_ready_future()` |
| `foundation_netio/.../shared/impls.rs` | Add `SendSafeBody::to_async(self) -> AsyncSendSafeBody` convenience method |
| `foundation_netio/.../client/shared/body_reader.rs` | Add `AsyncSendSafeBody` (futures_core::Stream), `collect_bytes_async`, `collect_string_async` |
| `foundation_db/.../native/d1_kvstore.rs` | Add `Clone`, `execute_sql_async`, rewrite all Async traits, rewrite sync traits |
| `foundation_db/.../native/r2_blobstore.rs` | Add `Clone`, async HTTP helpers, rewrite Async traits, rewrite sync traits |
| `foundation_db/.../native/turso_backend.rs` | Add `*_async_internal` methods, rewrite Async traits, rewrite sync traits |
| `foundation_db/.../native/libsql_store.rs` | Same pattern as TursoStorage |

## Backward Compatibility

Trait signatures unchanged. D1/R2 sync calls go through valtron thread pool instead of direct blocking — result is the same, latency dominated by network I/O.

## Verification

```bash
cargo test -p foundation_netio -- client
cargo test -p foundation_db
cargo test -p foundation_auth
cargo build -p foundation_db --target wasm32-unknown-unknown --no-default-features --features d1,r2,foundation_core/ssl-rustls-awsrc,foundation_core/std
cargo build -p foundation_db --features turso,d1,r2
```

---

_Created: 2026-05-30_

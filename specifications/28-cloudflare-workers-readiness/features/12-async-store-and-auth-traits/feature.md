---
feature: "Async Store Traits and Auth Async Methods"
description: "Add async trait variants for KeyValueStore, BlobStore, RateLimiterStore, CredentialStore, SessionManager, and NativeOAuth, enabling native .await usage in async contexts. Uses #[async_trait(?Send)] for wasm compatibility, with the valtron stream-to-future bridge for native async implementations. Eliminates all futures_lite::block_on usage."
status: "proposed"
priority: "high"
depends_on:
  - "11-valtron-async-bridge"
estimated_effort: "large"
created: 2026-05-20
last_updated: 2026-05-20
author: "Main Agent"
---

# Feature: Async Store Traits, Auth Async Methods, and HTTP Serve Async

## Overview

Add async trait variants for storage, authentication, and HTTP handler types, enabling native `.await` usage in async contexts (CF Workers, axum servers, etc.). This extends the pattern established by `AsyncQueryStore` to cover all storage operations, auth operations, and wasm HTTP serve handlers.

Critically: **all `futures_lite::block_on` usage is eliminated**. Wasm backends no longer block on JS Promises from sync contexts. Native backends use the valtron stream-to-future bridge for "fake async" that runs on the valtron thread pool.

**Three categories of additions:**

1. **Async storage traits** — `AsyncKeyValueStore`, `AsyncBlobStore`, `AsyncRateLimiterStore` mirroring their sync counterparts, returning `Result<T, StorageError>` directly (no streams).
2. **Async auth methods** — `_async` variants on `SessionManager` and `NativeOAuth` for native async token exchange and session management.
3. **Async HTTP serve traits** — `CfServe` and `WebServe` become `#[async_trait(?Send)]` traits with `async fn serve_cf` / `async fn serve_web`. Since these traits only ever execute in wasm async contexts (CF Workers `handle_request` is already `async fn`), there's no need for sync variants — just make the traits async directly.

## Problem Statement

Currently, all storage and auth operations are synchronous. In async contexts:

- **CF Workers**: Wasm backends like `D1WasmStorage` use `futures_lite::block_on` to resolve JS Promises inside sync trait methods. This blocks the single-threaded event loop — semantically wrong and potentially deadlocking in future CF runtime versions.
- **Native async servers** (axum, actix): Calling sync storage blocks the async runtime thread, reducing throughput.
- **Composition**: Async code cannot compose storage operations with other futures using `.await`.

The valtron stream-to-future bridge (feature 11) provides the mechanism to convert sync `StreamIterator`-based operations into `Future`s. This feature defines the async traits that expose them.

## Design Principles

### 1. `#[async_trait(?Send)]` for wasm compatibility

All async traits use `#[async_trait::async_trait(?Send)]` — not `Send`. This is required because wasm32 futures are `!Send` (single-threaded). Native implementations can still implement these traits; `?Send` only relaxes the bound, it doesn't require `!Send`.

### 2. Single-value → `Result<T>`, multi-value → `impl Stream`

This is the most important distinction. Async methods must not break the streaming model for operations that return many items:

- **Single-value ops** (`get`, `set`, `delete`, `exists`, `put_blob`, `get_blob`, `delete_blob`, `blob_exists`, `check_rate_limit`, `record_rate_limit`, `reset_rate_limit`, `execute`, `execute_batch`) → return `Result<T, StorageError>` directly. Bridged via `.into_ready_future().await`.

- **Multi-value ops** (`list_keys`, `query`) → return `impl futures_core::Stream<Item = Result<T, StorageError>>`. Bridged via `.into_future_stream()`. The caller gets a proper `futures_core::Stream` they can iterate item-by-item with `.next().await`, compose with other stream combinators, or collect into a `Vec` if they choose. Collecting into a `Vec` inside the trait method would force a full materialization point and defeat the purpose of having a stream in the first place.

The sync traits always return `StorageItemStream` (the valtron `StreamIterator` wrapped in `Box<dyn Iterator>`) — that's the baseline. Async traits adapt it: single values resolve to a single `.await` that yields the result; multi values yield a `Stream` that can be consumed lazily.

### 3. Wasm implementations call JS async APIs directly

Wasm backends (like `D1WasmStorage`) already have private async methods that resolve `JsFuture`. Make them public. The async trait impl calls them directly — no valtron bridge needed. The sync trait impl also calls the public async method, but since it's a sync context, wraps the `Result<T>` into a trivial single-item `StorageItemStream` (not via `block_on`).

### 4. Native implementations use valtron bridge

For native backends (`D1KeyValueStore`), methods already produce `StorageItemStream` from inline HTTP + SQL logic. Extract that logic to public stream-producing methods. Sync trait impls call them directly (passthrough). Async trait impls call the same public method and bridge via `.into_ready_future()` / `.into_collect_future()`.

### 5. Sync traits remain the default

Sync traits (`KeyValueStore`, `BlobStore`, etc.) remain the primary interface. Async traits are opt-in additions for async contexts. No breaking changes to existing implementations.

### 6. Why `#[async_trait(?Send)]` over native async traits

Rust 1.75+ supports native async trait methods (RPITIT), but this codebase uses `#[async_trait::async_trait(?Send)]` for two reasons:

- **Consistency with `AsyncQueryStore`** — the existing async trait already uses this macro. Mixing native and macro-based async traits creates an inconsistent API surface.
- **`?Send` control** — the macro explicitly makes futures `!Send`, which is required for wasm32 where all code runs on a single thread. Native async traits infer `Send` from bounds, which can cause compile errors on wasm if a trait method happens to be used in a `Send`-requiring context.

Native async traits can be adopted once the entire async surface is migrated together — not piecemeal.

## Deep Backend Analysis: All foundation_db Backends

There are **10 storage backends** across 5 distinct patterns. Not all implement all 4 traits — R2 backends are BlobStore-only, KV backends lack QueryStore, Memory/MemoryJson/JsonFile reject QueryStore.

### Pattern 1: wasm-bindgen async backends (D1WasmStorage, R2WasmStorage, KVWasmStorage)

These have private `async fn` methods that resolve JS Promises via `JsFuture`. Sync traits call them through `futures_lite::block_on` and wrap in `stream_once`/`stream_many`. **~30 block_on calls to eliminate across 3 backends.**

#### D1WasmStorage — 15 private async methods, implements all 4 traits + AsyncQueryStore

| Private async method | Returns | Trait |
|---|---|---|
| `get_async<V>` | `Result<Option<V>>` | KeyValueStore |
| `set_async<V>` | `Result<()>` | KeyValueStore |
| `delete_async` | `Result<()>` | KeyValueStore |
| `exists_async` | `Result<bool>` | KeyValueStore |
| `list_keys_async` | `Result<Vec<String>>` | KeyValueStore |
| `query_async` | `Result<Vec<SqlRow>>` | QueryStore |
| `execute_async` | `Result<u64>` | QueryStore |
| `execute_batch_async` | `Result<()>` | QueryStore |
| `check_rate_limit_async` | `Result<bool>` | RateLimiterStore |
| `record_rate_limit_async` | `Result<u32>` | RateLimiterStore |
| `reset_rate_limit_async` | `Result<()>` | RateLimiterStore |
| `put_blob_async` | `Result<()>` | BlobStore |
| `get_blob_async` | `Result<Option<Vec<u8>>>` | BlobStore |
| `delete_blob_async` | `Result<()>` | BlobStore |
| `blob_exists_async` | `Result<bool>` | BlobStore |

Already implements `AsyncQueryStore` (delegates to private async methods). Needs `AsyncKeyValueStore`, `AsyncBlobStore`, `AsyncRateLimiterStore`.

#### R2WasmStorage — 4 private async methods, BlobStore only

| Private async method | Returns | Trait |
|---|---|---|
| `put_blob_async` | `Result<()>` | BlobStore |
| `get_blob_async` | `Result<Option<Vec<u8>>>` | BlobStore |
| `delete_blob_async` | `Result<()>` | BlobStore |
| `blob_exists_async` | `Result<bool>` | BlobStore |

Needs `AsyncBlobStore`.

#### KVWasmStorage — 12 private async methods, KeyValueStore + RateLimiterStore + BlobStore (no QueryStore)

| Private async method | Returns | Trait |
|---|---|---|
| `get_async<V>` | `Result<Option<V>>` | KeyValueStore |
| `set_async<V>` | `Result<()>` | KeyValueStore |
| `delete_async` | `Result<()>` | KeyValueStore |
| `exists_async` | `Result<bool>` | KeyValueStore |
| `list_keys_async` | `Result<Vec<String>>` | KeyValueStore |
| `check_rate_limit_async` | `Result<bool>` | RateLimiterStore |
| `record_rate_limit_async` | `Result<u32>` | RateLimiterStore |
| `reset_rate_limit_async` | `Result<()>` | RateLimiterStore |
| `put_blob_async` | `Result<()>` | BlobStore |
| `get_blob_async` | `Result<Option<Vec<u8>>>` | BlobStore |
| `delete_blob_async` | `Result<()>` | BlobStore |
| `blob_exists_async` | `Result<bool>` | BlobStore |

Needs `AsyncKeyValueStore`, `AsyncBlobStore`, `AsyncRateLimiterStore`.

**All 3 backends follow the same refactor**: Make private async methods `pub`. Sync trait impls call them and convert `Result<T>` to `StorageItemStream` directly (no `block_on`). Async trait impls call them with `.await`.

### Pattern 2: HTTP-based native backends (D1KeyValueStore, R2BlobStore)

Sync trait methods do inline HTTP calls via `SimpleHttpClient`, SQL execution, JSON parsing, then wrap in `StorageItemStream` via `wrap_value`/`wrap_vec`. No `block_on` used.

#### D1KeyValueStore — implements KeyValueStore + QueryStore + RateLimiterStore + BlobStore

All methods are monolithic: `execute_sql()` HTTP call → `extract_rows()` → deserialization → `wrap_value`/`wrap_vec`. Helpers: `execute_sql`, `extract_rows`, `wrap_value`, `wrap_vec`.

Refactor: Extract each method's logic into a public stream-producing method (e.g., `get_kv_stream`, `query_stream`). Sync trait becomes passthrough. Async trait bridges via `.into_ready_future()` / `.into_future_stream()`.

#### R2BlobStore — BlobStore only

4 sync methods: `put_blob`, `get_blob`, `delete_blob`, `blob_exists`. Each does inline HTTP (PUT/GET/DELETE/HEAD) → status check → `wrap_value`. Helpers: `body_bytes`, `wrap_value`.

Refactor: Same as D1KeyValueStore — extract to public stream methods, sync passthrough, async bridge.

### Pattern 3: valtron-native backends (TursoStorage, LibsqlStorage)

**Already use valtron `schedule_future` / `run_future_iter` natively.** These are the most async-ready backends in the codebase. Sync trait methods already produce `StorageItemStream` from async library APIs (turso/libsql crate). Uses `exec_future` only for init/migrations. Key patterns:

- Single-value ops: `schedule_future(async { ... })` → `map_circuit` for error propagation → `Box::new(stream)`
- Multi-value ops: `run_future_iter(...)` → `RowsIterator` / `LibsqlRowsIterator` for lazy row iteration
- Error handling: `map_circuit` + `ShortCircuit` pattern throughout
- Encryption: `maybe_encrypt` / `maybe_decrypt` in `map_done` closures

**Refactor: Extract `async move { ... }` blocks into named public async methods.** Currently the async logic is inline inside `schedule_future(async move { ... })`. Extract each into a `pub async fn` (e.g., `pub async fn get_value_async<V>(&self, key: &str) -> Result<Option<V>, StorageError>`). This becomes the single source of truth:

1. **Sync trait impls** — call the public async method via `schedule_future(async { self.xxx_async(...).await })` + `map_circuit` + `map_done` (same outer pattern, but the future body is a named method, not inline logic).
2. **Async trait impls** — call the public async method directly with `.await`.

Multi-value ops (`list_keys`, `query`) keep using `run_future_iter` in the sync path since they need lazy row iteration; the async path bridges via `.into_future_stream()`.

Both implement all 4 traits (KeyValueStore, QueryStore, RateLimiterStore, BlobStore).

### Pattern 4: simple sync backends (MemoryStorage, MemoryJsonStore, JsonFileStorage)

All operations are purely in-memory or file-based with no async underlying APIs. Methods produce trivial streams via `Box::new(std::iter::once(...))` or `Box::new(vec.into_iter().map(...))`.

- **MemoryStorage**: `HashMap<String, Zeroizing<Vec<u8>>>` — implements KeyValueStore, RateLimiterStore, BlobStore. Rejects QueryStore.
- **MemoryJsonStore**: `HashMap<String, String>` — implements KeyValueStore, RateLimiterStore, BlobStore, QueryStore(rejects). Has `stream_once`/`stream_many` helpers.
- **JsonFileStorage**: `HashMap<String, Zeroizing<Vec<u8>>>` + atomic disk flush — implements KeyValueStore, BlobStore. Rejects QueryStore, RateLimiterStore.

Async traits for these would bridge trivial streams. Since there's no real I/O, async methods would be immediate `Ready` — useful for API consistency but no performance benefit. Implement by wrapping existing sync logic in `std::future::ready()`.

### Pattern 5: CredentialStorage (foundation_auth) — consumes streams from StorageProvider

Not a storage backend itself — wraps `StorageProvider` and drains its streams. Each sync method calls `StorageProvider` (returns `StorageItemStream`), then drains via `.flat_map(...).next()` or `.collect()`.

Refactor: Async trait impls call same `StorageProvider` methods and bridge via `.into_ready_future()` / `.into_future_stream()`. No extraction needed.

### Trait coverage matrix

| Backend | KeyValueStore | BlobStore | QueryStore | RateLimiterStore | AsyncQueryStore |
|---|---|---|---|---|---|
| TursoStorage | ✅ | ✅ | ✅ | ✅ | needs impl |
| LibsqlStorage | ✅ | ✅ | ✅ | ✅ | needs impl |
| D1KeyValueStore | ✅ | ✅ | ✅ | ✅ | needs impl |
| R2BlobStore | ❌ | ✅ | ❌ | ❌ | N/A |
| MemoryStorage | ✅ | ✅ | ❌ | ✅ | N/A |
| MemoryJsonStore | ✅ | ✅ | ❌(rejects) | ✅ | N/A |
| JsonFileStorage | ✅ | ✅ | ❌(rejects) | ❌(rejects) | N/A |
| D1WasmStorage | ✅ | ✅ | ✅ | ✅ | ✅ (existing) |
| R2WasmStorage | ❌ | ✅ | ❌ | ❌ | N/A |
| KVWasmStorage | ✅ | ✅ | ❌(rejects) | ✅ | N/A |

### Backend coverage in this feature

| Backend | Pattern | block_on calls | Refactor needed | Async traits needed |
|---|---|---|---|---|
| D1WasmStorage | 1 | 15 | pub async, remove block_on | AsyncKV + AsyncBlob + AsyncRate |
| R2WasmStorage | 1 | 4 | pub async, remove block_on | AsyncBlob |
| KVWasmStorage | 1 | 12 | pub async, remove block_on | AsyncKV + AsyncBlob + AsyncRate |
| D1KeyValueStore | 2 | 0 | extract stream methods | AsyncKV + AsyncBlob + AsyncRate + AsyncQuery |
| R2BlobStore | 2 | 0 | extract stream methods | AsyncBlob |
| TursoStorage | 3 | 0 | extract async methods from inline schedule_future | AsyncKV + AsyncBlob + AsyncRate + AsyncQuery |
| LibsqlStorage | 3 | 0 | extract async methods from inline schedule_future | AsyncKV + AsyncBlob + AsyncRate + AsyncQuery |
| MemoryStorage | 4 | 0 | wrap in ready future | AsyncKV + AsyncBlob + AsyncRate |
| MemoryJsonStore | 4 | 0 | wrap in ready future | AsyncKV + AsyncBlob + AsyncRate |
| JsonFileStorage | 4 | 0 | wrap in ready future | AsyncKV + AsyncBlob |

### Pattern A: wasm-bindgen backends (D1WasmStorage, R2WasmStorage, KVWasmStorage)

1. **Make private async methods public** — `get_async`, `set_async`, etc. are already fully implemented, just change visibility.
2. **Sync trait impls** — call the public async method and convert `Result<T>` to `StorageItemStream` via `Box::new(std::iter::once(Stream::Next(result)))`. No `block_on` needed.
3. **Async trait impls** — call the public async method directly with `.await`. No valtron bridge needed.

### Pattern B: HTTP-based native backends (D1KeyValueStore, R2BlobStore)

1. **Public methods produce valtron streams** — extract the SQL/HTTP/parsing/deserialization logic from each sync trait method into public methods that return `StorageItemStream<D, P>`. These are the single source of truth.
2. **Sync trait impls** call the public stream method directly — passthrough, no change in behavior.
3. **Async trait impls** call the same public stream method and bridge:
   - Single-value ops → `.into_ready_future().await` (resolves to `Result<T>`)
   - Multi-value ops (`list_keys_stream`, `query_stream`) → `.into_future_stream()` (returns `impl Stream<Item = Result<T, E>>`)

### Pattern C: valtron-native backends (TursoStorage, LibsqlStorage)

1. **Extract `async move { ... }` blocks into named public async methods** — the business logic (SQL prep, encryption, deserialization) currently inline in `schedule_future` becomes a `pub async fn`.
2. **Sync trait impls** — call the public async method via `schedule_future(async { self.xxx_async(...).await })` + `map_circuit` (same outer wrapping, named method instead of inline).
3. **Async trait impls** — call the public async method directly with `.await`.

### Pattern D: simple sync backends (MemoryStorage, MemoryJsonStore, JsonFileStorage)

1. **No extraction needed** — methods already produce trivial streams directly.
2. **Sync trait impls** — unchanged.
3. **Async trait impls** — call existing sync trait method (returns `StorageItemStream`), bridge via `.into_ready_future()` / `.into_future_stream()`. Since these are in-memory, the stream resolves immediately.

### Pattern E: CredentialStorage — consumes streams from StorageProvider

1. **No extraction needed** — StorageProvider already produces streams.
2. **Sync trait impls** — drain streams via `.flat_map(...).next()` (single value) / `.collect()` (multi-value) (unchanged).
3. **Async trait impls** — call StorageProvider methods (return streams), bridge:
   - Single-value ops → `.into_ready_future().await`
   - Multi-value (`list_keys`) → `.into_future_stream()`

### Pattern F: NativeOAuth — inline sync HTTP

1. **Extract shared private helpers** — body builders (form-urlencoded construction), response parsers (JSON → OAuthToken), HTTP executor (SimpleHttpClient POST).
2. **Sync methods** — call helpers directly (refactored from current inline logic).
3. **Async methods** — wrap HTTP executor in valtron `from_future` + `execute`, bridge via `.into_ready_future()`, call parser on result.

### Pattern G: WasmOAuth — already follows Pattern A

1. **Make private async methods public** — already implemented.
2. **Sync trait impls** — call public async method, convert Result to stream (no `block_on`).
3. **Async trait impls** — call public async method directly.

## Architecture

### Async Storage Traits

Three new traits in `foundation_db/src/core/storage_provider.rs`, mirroring their sync counterparts:

- **AsyncKeyValueStore**:
  - Single-value → `Result<T>`: `get_async<V>`, `set_async<V>`, `delete_async`, `exists_async`
  - Multi-value → `impl Stream<Item = Result<String, StorageError>>`: `list_keys_async`
- **AsyncBlobStore** — all single-value → `Result<T>`: `put_blob_async`, `get_blob_async`, `delete_blob_async`, `blob_exists_async`
- **AsyncRateLimiterStore** — all single-value → `Result<T>`: `check_rate_limit_async`, `record_rate_limit_async`, `reset_rate_limit_async`

### Async Auth Traits

New trait in `foundation_auth/src/shared/credential_store.rs`:

- **AsyncCredentialStore**:
  - Single-value → `Result<T>`: `get_async<V>`, `set_async<V>`, `delete_async`, `exists_async`
  - Multi-value → `impl Stream<Item = Result<String, CredentialStoreError>>`: `list_keys_async`

### Async HTTP Serve Traits (foundation_http)

Convert existing `CfServe` and `WebServe` traits to `#[async_trait(?Send)]` — no sync variant needed since these traits only ever execute in wasm async contexts (the dispatch is already `async fn`).

- **CfServe** (converted to `#[async_trait(?Send)]`):
  - `async fn serve_cf(&self, bag: Arc<ContextBag>, req: SimpleIncomingRequest, conn: &mut CfConn) -> CfConnectionResult`
- **WebServe** (converted to `#[async_trait(?Send)]`):
  - `async fn serve_web(&self, bag: Arc<ContextBag>, req: SimpleIncomingRequest, conn: &mut WebConn) -> WebConnectionResult`

`CfServeFactory` and `WebServeFactory` stay unchanged — they just instantiate handlers.

**Dispatch:** `dispatch_cf` and `dispatch_web` in `dispatch.rs` become `async fn`. The handler call changes from `handler.serve_cf(bag, req, &mut conn)` to `handler.serve_cf(bag, req, &mut conn).await`. Middleware remains sync (same `MiddlewareResult::Continue` loop).

**No type separation needed:** Since there's only one variant (async), `HttpApp<Arc<dyn CfServe>>` remains the single type. The `CfHttpApp` wrapper stays the same — only its `handle_request` implementation changes from calling sync `dispatch_cf` to `await`ing `dispatch_cf`.

**Migration impact:** All existing `impl CfServe` implementations must become `async fn serve_cf`. Handlers that don't need async can just `async fn serve_cf(...) -> ... { Ok(CfConnectionResult::Ok) }` — trivial to migrate. No `block_on` is needed inside handlers since the method is now natively async.

### SessionManager Async Methods

New `_async` methods on `SessionManager`, gated on `S: AsyncCredentialStore`:

- `create_session_async` — same business logic, calls `store.set_async`
- `get_session_async` — same business logic, calls `store.get_async`. For the key scan, calls `store.list_keys_async` (returns Stream), iterates with `while let Some(key) = stream.next().await`, then fires `store.get_async` for each key via `join_all` for parallelism
- `revoke_session_async` — same business logic, calls `store.get_async` + `store.set_async`
- `revoke_all_sessions_async` — calls `store.list_keys_async` (returns Stream), collects keys, then per-key async calls
- `extend_session_async` (private) — same business logic, calls `store.set_async`

No `block_on` in async methods.

### NativeOAuth Async Methods

New async methods on `NativeOAuth`:

- `exchange_code_async`
- `client_credentials_async`
- `refresh_token_async`

These use the valtron stream-to-future bridge to run sync HTTP on the valtron thread pool and expose it as async.

## Refactor Principle: Public Stream Methods, Two Consumers

**Core rule: do not duplicate logic.** The actual work (SQL execution, HTTP calls, deserialization) is identical for sync and async — only how the result is collected differs.

### Pattern A: wasm-bindgen backends (D1WasmStorage, R2WasmStorage, KVWasmStorage)

1. **Make private async methods public** — `get_async`, `set_async`, etc. are already fully implemented, just change visibility.
2. **Sync trait impls** — call the public async method and convert `Result<T>` to `StorageItemStream` via `Box::new(std::iter::once(Stream::Next(result)))`. No `block_on` needed because the async method is called from a context where the Promise is already resolved (wasm single-threaded runtime).
3. **Async trait impls** — call the public async method directly with `.await`. No valtron bridge needed.

### Pattern B: HTTP-based native backends (D1KeyValueStore, R2BlobStore)

1. **Public methods produce valtron streams** — extract the SQL/HTTP/parsing/deserialization logic from each sync trait method into public methods that return `StorageItemStream<D, P>`. These are the single source of truth.
2. **Sync trait impls** call the public stream method directly — passthrough, no change in behavior.
3. **Async trait impls** call the same public stream method and bridge:
   - Single-value ops → `.into_ready_future().await` (resolves to `Result<T>`)
   - Multi-value ops (`list_keys_stream`, `query_stream`) → `.into_future_stream()` (returns `impl Stream<Item = Result<T, E>>`)

### Pattern C: valtron-native backends (TursoStorage, LibsqlStorage)

1. **No extraction needed** — these already produce `StorageItemStream` from async library APIs using `schedule_future` / `run_future_iter`.
2. **Sync trait impls** — unchanged, already return `StorageItemStream`.
3. **Async trait impls** — call the existing sync trait method (which returns `StorageItemStream`), bridge:
   - Single-value ops → `.into_ready_future().await`
   - Multi-value ops (`list_keys`, `query`) → `.into_future_stream()`

### Pattern D: CredentialStorage — consumes streams from StorageProvider

1. **No extraction needed** — StorageProvider already produces streams.
2. **Sync trait impls** — drain streams via `.flat_map(...).next()` (single value) / `.collect()` (multi-value) (unchanged).
3. **Async trait impls** — call StorageProvider methods (return streams), bridge:
   - Single-value ops → `.into_ready_future().await`
   - Multi-value (`list_keys`) → `.into_future_stream()`

### Pattern E: simple sync backends (MemoryStorage, MemoryJsonStore, JsonFileStorage)

1. **No extraction needed** — methods already produce trivial streams directly.
2. **Sync trait impls** — unchanged.
3. **Async trait impls** — call existing sync trait method (returns `StorageItemStream`), bridge via `.into_ready_future()` / `.into_future_stream()`. Since these are in-memory, the stream resolves immediately.

### Pattern F: NativeOAuth — inline sync HTTP

1. **Extract shared private helpers** — body builders (form-urlencoded construction), response parsers (JSON → OAuthToken), HTTP executor (SimpleHttpClient POST).
2. **Sync methods** — call helpers directly (refactored from current inline logic).
3. **Async methods** — wrap HTTP executor in valtron `from_future` + `execute`, bridge via `.into_ready_future()`, call parser on result.

### Pattern G: WasmOAuth — already follows Pattern A

1. **Make private async methods public** — already implemented.
2. **Sync trait impls** — call public async method, convert Result to stream (no `block_on`).
3. **Async trait impls** — call public async method directly.

## Implementation Strategy

### Phase 1: wasm-bindgen backends — Make Private Async Methods Public, Eliminate block_on

**Files:**
- `backends/foundation_db/src/wasm/wasm_storage/d1_wasm.rs` (15 methods)
- `backends/foundation_db/src/wasm/wasm_storage/r2_wasm.rs` (4 methods)
- `backends/foundation_db/src/wasm/wasm_storage/kv_wasm.rs` (12 methods)

Change visibility of all private async methods to `pub`. Remove `futures_lite::block_on` from all sync trait impls. Replace with direct calls to the public async methods, converting `Result<T>` to `StorageItemStream` via `Self::stream_once()` / `Self::stream_many()`.

The key insight: on wasm32, calling an async method from a sync context doesn't require `block_on` because the async method's body is already being evaluated eagerly — the JS Promise resolution happens through the wasm-bindgen/futures runtime, not a thread pool. The `stream_once`/`stream_many` wrappers already produce the correct `StorageItemStream` shape.

### Phase 2: HTTP-based native backends — Extract Public Stream Methods

**Files:**
- `backends/foundation_db/src/core/backends/d1_kvstore.rs`
- `backends/foundation_db/src/core/backends/r2_blobstore.rs`

For each sync trait method, extract the SQL/HTTP/parsing logic into a public method returning `StorageItemStream`:

- **D1KeyValueStore**: `get_kv_stream`, `set_kv_stream`, `exists_stream`, `list_keys_stream`, `delete_stream`, `query_stream`, `execute_stream`, `execute_batch_stream`, `check_rate_limit_stream`, `record_rate_limit_stream`, `reset_rate_limit_stream`, `put_blob_stream`, `get_blob_stream`, `delete_blob_stream`, `blob_exists_stream`
- **R2BlobStore**: `put_blob_stream`, `get_blob_stream`, `delete_blob_stream`, `blob_exists_stream`

The existing private helpers (`execute_sql`, `extract_rows`, `wrap_value`, `wrap_vec` for D1; `body_bytes`, `wrap_value` for R2) stay unchanged.

### Phase 3: valtron-native backends — Extract Public Async Methods + Async Traits

**Files:**
- `backends/foundation_db/src/native/turso_backend.rs`
- `backends/foundation_db/src/native/libsql_backend.rs`

Extract each `async move { ... }` block from `schedule_future()` calls into a named `pub async fn`. For example, the `get` method's `async move { conn.prepare(...).await; rows.next().await; ... }` becomes `pub async fn get_value_async<V>(&self, key: &str) -> Result<Option<V>, StorageError>`.

Sync trait impls are refactored to call these named methods via `schedule_future(async { self.xxx_async(...).await })` — the outer `map_circuit`/`map_done` wrapping stays the same. This eliminates inline async blocks and creates a single source of truth.

Then implement `AsyncKeyValueStore`, `AsyncBlobStore`, `AsyncRateLimiterStore`, `AsyncQueryStore` — each async method calls the corresponding `pub async fn` directly with `.await`.

### Phase 3b: HTTP-based native backends — Async Trait Implementations

Implement async traits for D1KeyValueStore and R2BlobStore using the public stream methods extracted in Phase 2:

- **D1KeyValueStore** — `AsyncKeyValueStore`, `AsyncBlobStore`, `AsyncRateLimiterStore`, `AsyncQueryStore`. Each calls the corresponding `*_stream` method and bridges via `.into_ready_future()` / `.into_future_stream()`.
- **R2BlobStore** — `AsyncBlobStore` only.

### Phase 4: Simple sync backends — Async Trait Implementations

**Files:**
- `backends/foundation_db/src/core/backends/memory.rs`
- `backends/foundation_db/src/core/backends/memory_json.rs`
- `backends/foundation_db/src/native/json_file.rs`

Implement async traits for MemoryStorage, MemoryJsonStore, JsonFileStorage. Since these are in-memory, async methods wrap existing sync logic in `std::future::ready()` — immediate resolution with no real async work needed.

### Phase 5: wasm-bindgen backends — Async Trait Implementations

**Files:**
- `backends/foundation_db/src/wasm/wasm_storage/d1_wasm.rs`
- `backends/foundation_db/src/wasm/wasm_storage/r2_wasm.rs`
- `backends/foundation_db/src/wasm/wasm_storage/kv_wasm.rs`

Implement `AsyncKeyValueStore`, `AsyncBlobStore`, `AsyncRateLimiterStore` for each. Async methods call the `pub async fn` methods from Phase 1 directly with `.await`. D1WasmStorage already has `AsyncQueryStore` — no change needed there.

### Phase 6: StorageProvider — Async Trait Passthroughs

**File:** `backends/foundation_db/src/storage_provider.rs`

Implement `AsyncKeyValueStore`, `AsyncBlobStore`, `AsyncRateLimiterStore`, `AsyncQueryStore` on `StorageProvider`. Each method delegates to the inner backend variant. For backends that don't support a trait (e.g., R2 → KeyValueStore), return `StorageError::Generic("...")` matching the sync behavior.

### Phase 7: WasmOAuth — Same Pattern as Phase 1

**File:** `backends/foundation_auth/src/wasm_bindgen/oauth.rs`

Make private async methods public. Remove `futures_lite::block_on` from sync methods. Async trait impl calls public async methods directly.

### Phase 8: NativeOAuth — Extract Helpers + Add Async Methods

**File:** `backends/foundation_auth/src/native/oauth.rs`

Factor out shared logic into private helpers:

- **Body builders** — form-urlencoded body construction for each OAuth flow.
- **Response parsers** — token parsing and refresh token parsing.
- **HTTP executor** — the SimpleHttpClient POST call.

Sync methods call these helpers directly (refactored from current inline logic). Async methods wrap the HTTP executor in valtron `from_future` + `execute`, bridge via `.into_ready_future().await`, then call the parser.

### Phase 11: Convert CfServe and WebServe to Async Traits (foundation_http)

**Files:**
- `backends/foundation_http/src/wasm/serve_cf.rs`
- `backends/foundation_http/src/wasm/serve_web.rs`
- `backends/foundation_http/src/wasm/dispatch.rs`
- `backends/foundation_http/src/wasm/bridge/cf.rs`

1. **Convert `CfServe`** to `#[async_trait(?Send)]` — change `fn serve_cf` to `async fn serve_cf`. No new trait needed.
2. **Convert `WebServe`** to `#[async_trait(?Send)]` — change `fn serve_web` to `async fn serve_web`. No new trait needed.
3. **Convert `dispatch_cf` and `dispatch_web`** to `async fn` — middleware loop stays sync, handler call becomes `.await`.
4. **Update `CfHttpApp::handle_request`** — calls `dispatch_cf(bag, req).await` instead of sync `dispatch_cf(bag, req)`.
5. **Update all existing `impl CfServe` / `impl WebServe`** — make their methods `async`. The `cf-login-app` example handler is the primary consumer that needs this update.

### Phase 12: Async CredentialStore Trait

**File:** `backends/foundation_auth/src/shared/credential_store.rs`

Add `AsyncCredentialStore` trait. Implement for:

- **CredentialStorage**: single-value methods call `self.storage.get()`, `self.storage.set()`, etc. and bridge via `.into_ready_future().await`. Multi-value `list_keys_async` bridges the StorageProvider stream via `.into_future_stream()`.
- **D1WasmStorage**: calls its public async methods directly (from Phase 1).
- **TursoStorage** / **LibsqlStorage**: calls existing sync methods, bridges via `.into_ready_future()` / `.into_future_stream()`.
- **MemoryStorage** / **MemoryJsonStore**: wraps sync logic in `std::future::ready()`.

### Phase 13: SessionManager Async Methods

**File:** `backends/foundation_auth/src/shared/session.rs`

Add `_async` methods gated on `S: CredentialStore + AsyncCredentialStore + Send + Sync`. Each mirrors its sync counterpart but calls async store methods instead.

`get_session_async` should parallelize `get_async` calls using `join_all` for a real performance improvement over the sync linear scan.

### Phase 14: Tests

- Roundtrip tests for each async trait
- `test_async_then_sync_consistency` — write via async, read via sync
- `test_sync_then_async_consistency` — write via sync, read via async
- `test_parallel_async_operations` — concurrent async ops
- Session manager: create, get, revoke via async
- **Async HTTP serve**: register a `CfServeAsync` handler that `.await`s a storage operation, verify response contains the stored value
- Verify no `futures_lite::block_on` remains in wasm backends (grep test)

## Module Layout

```
backends/foundation_db/src/core/storage_provider.rs
  ├── KeyValueStore (existing, unchanged)
  ├── BlobStore (existing, unchanged)
  ├── QueryStore (existing, unchanged)
  ├── AsyncQueryStore (existing, unchanged)
  ├── RateLimiterStore (existing, unchanged)
  ├── AsyncKeyValueStore (NEW)
  ├── AsyncBlobStore (NEW)
  └── AsyncRateLimiterStore (NEW)

backends/foundation_db/src/core/backends/d1_kvstore.rs
  ├── Public stream methods (NEW) — extracted from current trait impls
  ├── Sync trait impls (refactored to passthroughs)
  ├── impl AsyncKeyValueStore (NEW)
  ├── impl AsyncBlobStore (NEW)
  ├── impl AsyncRateLimiterStore (NEW)
  └── impl AsyncQueryStore (NEW)

backends/foundation_db/src/core/backends/r2_blobstore.rs
  ├── Public stream methods (NEW)
  ├── Sync trait impls (refactored to passthroughs)
  └── impl AsyncBlobStore (NEW)

backends/foundation_db/src/native/turso_backend.rs
  ├── Sync trait impls (unchanged — already valtron-native)
  ├── impl AsyncKeyValueStore (NEW)
  ├── impl AsyncBlobStore (NEW)
  ├── impl AsyncRateLimiterStore (NEW)
  └── impl AsyncQueryStore (NEW)

backends/foundation_db/src/native/libsql_backend.rs
  ├── Sync trait impls (unchanged — already valtron-native)
  ├── impl AsyncKeyValueStore (NEW)
  ├── impl AsyncBlobStore (NEW)
  ├── impl AsyncRateLimiterStore (NEW)
  └── impl AsyncQueryStore (NEW)

backends/foundation_db/src/core/backends/memory.rs
  ├── Sync trait impls (unchanged)
  ├── impl AsyncKeyValueStore (NEW)
  ├── impl AsyncBlobStore (NEW)
  └── impl AsyncRateLimiterStore (NEW)

backends/foundation_db/src/core/backends/memory_json.rs
  ├── Sync trait impls (unchanged)
  ├── impl AsyncKeyValueStore (NEW)
  ├── impl AsyncBlobStore (NEW)
  └── impl AsyncRateLimiterStore (NEW)

backends/foundation_db/src/native/json_file.rs
  ├── Sync trait impls (unchanged)
  ├── impl AsyncKeyValueStore (NEW)
  └── impl AsyncBlobStore (NEW)

backends/foundation_db/src/wasm/wasm_storage/d1_wasm.rs
  ├── Existing private async methods → make pub
  ├── Sync trait impls (refactored: no block_on, Result→Stream conversion)
  ├── impl AsyncKeyValueStore (NEW)
  ├── impl AsyncBlobStore (NEW)
  ├── impl AsyncRateLimiterStore (NEW)
  └── impl AsyncQueryStore (already exists)

backends/foundation_db/src/wasm/wasm_storage/r2_wasm.rs
  ├── Existing private async methods → make pub
  ├── Sync trait impls (refactored: no block_on)
  └── impl AsyncBlobStore (NEW)

backends/foundation_db/src/wasm/wasm_storage/kv_wasm.rs
  ├── Existing private async methods → make pub
  ├── Sync trait impls (refactored: no block_on)
  ├── impl AsyncKeyValueStore (NEW)
  ├── impl AsyncBlobStore (NEW)
  └── impl AsyncRateLimiterStore (NEW)

backends/foundation_db/src/storage_provider.rs
  ├── impl AsyncKeyValueStore (NEW — delegates to inner)
  ├── impl AsyncBlobStore (NEW — delegates to inner)
  ├── impl AsyncRateLimiterStore (NEW — delegates to inner)
  └── impl AsyncQueryStore (NEW — delegates to inner)

backends/foundation_auth/src/shared/credential_store.rs
  ├── CredentialStore (existing, unchanged)
  ├── CredentialStorage (existing, unchanged)
  ├── AsyncCredentialStore (NEW trait)
  ├── impl AsyncCredentialStore for CredentialStorage (NEW)
  ├── impl AsyncCredentialStore for D1WasmStorage (NEW)
  ├── impl AsyncCredentialStore for TursoStorage (NEW)
  ├── impl AsyncCredentialStore for LibsqlStorage (NEW)
  └── impl AsyncCredentialStore for MemoryStorage/MemoryJsonStore (NEW)

backends/foundation_auth/src/shared/session.rs
  ├── SessionManager sync methods (unchanged)
  └── SessionManager async methods (NEW, gated on S: AsyncCredentialStore)

backends/foundation_auth/src/native/oauth.rs
  ├── Shared private helpers (NEW) — body builders, parsers, HTTP executor
  ├── NativeOAuth sync methods (refactored to use helpers)
  └── NativeOAuth async methods (NEW)

backends/foundation_auth/src/wasm_bindgen/oauth.rs
  ├── Existing private async methods → make pub
  └── Sync methods (refactored: no block_on)

backends/foundation_http/src/wasm/serve_cf.rs
  ├── CfServe (existing, unchanged)
  ├── CfServeFactory (existing, unchanged)
  ├── CfServeAsync (NEW trait, #[async_trait(?Send)])
  └── CfServeAsyncFactory (NEW trait)

backends/foundation_http/src/wasm/serve_web.rs
  ├── WebServe (existing, unchanged)
  ├── WebServeFactory (existing, unchanged)
  ├── WebServeAsync (NEW trait, #[async_trait(?Send)])
  └── WebServeAsyncFactory (NEW trait)

backends/foundation_http/src/wasm/dispatch.rs
  ├── HttpAppCfDispatch (existing, unchanged — sync dispatch)
  ├── HttpAppWebDispatch (existing, unchanged — sync dispatch)
  ├── HttpAppCfAsyncDispatch (NEW — async dispatch for CfServeAsync)
  └── HttpAppWebAsyncDispatch (NEW — async dispatch for WebServeAsync)

backends/foundation_http/src/wasm/bridge/cf.rs
  ├── CfHttpApp (existing)
  │   ├── handle_request (existing — calls sync dispatch)
  │   └── handle_request_async (NEW — calls async dispatch)
  └── (new CfHttpAppAsync for HttpApp<Arc<dyn CfServeAsync>>)

backends/foundation_http/src/shared/app/mod.rs
  ├── HttpApp<Arc<dyn CfServe>> route methods (unchanged)
  ├── HttpApp<Arc<dyn WebServe>> route methods (unchanged)
  ├── HttpApp<Arc<dyn CfServeAsync>> route_cf_async methods (NEW)
  └── HttpApp<Arc<dyn WebServeAsync>> route_web_async methods (NEW)
```

## Testing

### Sync tests (unchanged)

All existing sync tests continue to pass. No breaking changes to sync traits.

### Async tests

- Roundtrip tests for each async trait (write via async, read via async)
- `test_async_then_sync_consistency` — write via async, read via sync, verify same value
- `test_sync_then_async_consistency` — write via sync, read via async, verify same value
- `test_parallel_async_operations` — fire multiple async ops concurrently, verify all complete
- Session manager: create, get, revoke via async, verify cookies and storage state
- Verify no `futures_lite::block_on` remains in wasm backends (grep test)

## Dependency Impact

- `async-trait` already a dependency (used by `AsyncQueryStore`).
- `tokio` and `smol` already dev-dependencies (from feature 11).
- No new runtime dependencies.

## Migration Path for Existing Code

Existing code using sync traits is unaffected. Migration to async is opt-in:

```rust
// Before (sync)
let user = store.get::<User>("user:1")?;

// After (async, opt-in)
let user = store.get_async::<User>("user:1").await?;
```

For wasm backends, prefer async in async contexts (CF Workers handlers). For native backends, async provides concurrency via valtron thread pool but the underlying I/O may still be blocking.

## Key Learnings

### ?Send vs Send in async_trait

`#[async_trait(?Send)]` allows the future to be `!Send`. This is required for wasm32 where all code runs on a single thread. Native implementations can still implement `?Send` traits — the bound is relaxed, not restricted.

### Valtron bridge for "fake async"

When a backend has no native async capability (e.g., `SimpleHttpClient`), the valtron stream-to-future bridge (`schedule_future` + `StreamReadyFuture`) provides a way to expose async methods. The work runs on the valtron thread pool — concurrent with other tasks, but not truly async I/O. This is still valuable for throughput in async servers.

### Refactor before duplicating

Adding `_async` methods that duplicate sync logic leads to drift. Factor out shared helpers (request building, response parsing, business logic) first, then both sync and async methods call them with different execution wrappers.

### Public stream methods prevent duplication

When a sync method returns a `StorageItemStream`, that stream IS the result. The sync trait impl is a passthrough. The async impl bridges it. No logic is duplicated — the stream-producing method is the single source of truth.

### Single-value vs multi-value bridging matters

Not all async methods should return `Result<T>`. Methods that produce multiple items (`list_keys`, `query`) must return `impl Stream<Item = Result<T, E>>` via `.into_future_stream()`. Collecting into a `Vec` inside the trait method would force a full materialization point and defeat the streaming model. The caller chooses whether to collect — the trait should not decide for them. Single-value methods (`get`, `set`, `delete`, `exists`, etc.) correctly resolve to `Result<T>` via `.into_ready_future()`.

### block_on elimination for wasm backends

On wasm32-unknown-unknown, async methods that resolve JS Promises via `JsFuture` are already "native async" — they don't need a thread pool or executor. The `futures_lite::block_on` was only needed because sync trait methods were the only public API. With public async methods, sync trait impls can call them without blocking: the Result is already computed, just wrap in a trivial stream. This is semantically correct because wasm32 is single-threaded — there's no real "blocking" of another thread.

### Parallel scan in get_session_async

`get_session` does a linear scan of all session keys. The async version should parallelize `get_async` calls using `join_all` or similar. This is a real performance improvement over the sync version.

---

_Created: 2026-05-20_

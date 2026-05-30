---
feature: "Async Store Traits and Auth Async Methods"
description: "Add async trait variants for KeyValueStore, BlobStore, RateLimiterStore, CredentialStore, SessionManager, and NativeOAuth, enabling native .await usage in async contexts. Uses #[async_trait(?Send)] for wasm compatibility, with the valtron stream-to-future bridge for native async implementations. Eliminates all futures_lite::block_on usage."
status: "pending"
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

This is the most important distinction. Async methods return the result directly, not wrapped in streams:

- **Single-value ops** (`get`, `set`, `delete`, `exists`, `put_blob`, `get_blob`, `delete_blob`, `blob_exists`, `check_rate_limit`, `record_rate_limit`, `reset_rate_limit`, `execute`, `execute_batch`) → return `Result<T, StorageError>` directly.

- **Multi-value ops** (`list_keys`, `query`) → return `Result<Vec<T>, StorageError>`. The caller can collect, iterate, or wrap in `.into_future_stream()` if they need lazy consumption. The trait does not force a streaming model — the caller decides.

The sync traits always return `StorageItemStream` (the valtron `StreamIterator` wrapped in `Box<dyn Iterator>`) — that's the baseline. Sync produces the stream by calling async methods via `schedule_future`. Async calls the same methods directly with `.await`.

### 3. Wasm implementations: async is the source of truth (exception)

Wasm backends (like `D1WasmStorage`) already have private async methods that resolve `JsFuture`. Make them public — these are the source of truth (same as async-only libraries). The async trait impl calls them directly with `.await`. The sync trait impl calls them via `schedule_future` + valtron bridge (no `block_on`). JS Promises on CF Workers are natively async, so these follow the async-first pattern.

### 4. Native implementations (non-async-only): public methods produce valtron streams

For native backends that are NOT backed by async-only libraries (`D1KeyValueStore`, `R2BlobStore`, in-memory, etc.), the public methods produce `StorageItemStream`. These are the source of truth. Sync trait impls call them directly (passthrough). Async trait impls bridge via `.into_ready_future()` / `.into_future_stream()`.

### 5. Sync traits remain the default

Sync traits (`KeyValueStore`, `BlobStore`, etc.) remain the primary interface. Async traits are opt-in additions for async contexts. No breaking changes to existing implementations. For backends where streams are the source of truth, sync impls are simple passthroughs to the stream-producing methods. For async-only backends, sync impls call `pub async fn` via `schedule_future` — the valtron bridge handles the conversion to `StorageItemStream`.

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

**All 3 backends follow the same refactor**: Make private async methods `pub async`. These are the source of truth. Sync trait impls call them via `schedule_future` + valtron bridge to produce `StorageItemStream`. Async trait impls call them directly with `.await`.

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

Multi-value ops (`list_keys`, `query`) keep using `run_future_iter` in the sync path since they need lazy row iteration; the async path returns `Result<Vec<T>>` directly.

Both implement all 4 traits (KeyValueStore, QueryStore, RateLimiterStore, BlobStore).

### Pattern 4: simple sync backends (MemoryStorage, MemoryJsonStore, JsonFileStorage)

All operations are purely in-memory or file-based with no async underlying APIs. Methods produce trivial streams via `Box::new(std::iter::once(...))` or `Box::new(vec.into_iter().map(...))`.

- **MemoryStorage**: `HashMap<String, Zeroizing<Vec<u8>>>` — implements KeyValueStore, RateLimiterStore, BlobStore. Rejects QueryStore.
- **MemoryJsonStore**: `HashMap<String, String>` — implements KeyValueStore, RateLimiterStore, BlobStore, QueryStore(rejects). Has `stream_once`/`stream_many` helpers.
- **JsonFileStorage**: `HashMap<String, Zeroizing<Vec<u8>>>` + atomic disk flush — implements KeyValueStore, BlobStore. Rejects QueryStore, RateLimiterStore.

Async traits for these call the existing sync trait methods (which return `StorageItemStream`) and bridge via `.into_ready_future()` / `.into_future_stream()`. Since in-memory, streams resolve immediately.

### Pattern 5: CredentialStorage (foundation_auth) — consumes streams from StorageProvider

Not a storage backend itself — wraps `StorageProvider` and drains its streams. Each sync method calls `StorageProvider` (returns `StorageItemStream`), then drains via `.flat_map(...).next()` or `.collect()`.

Refactor: Sync trait impls unchanged (drain streams from StorageProvider). Async trait impls call same `StorageProvider` methods (return streams), bridge:
   - Single-value ops → `.into_ready_future().await`
   - Multi-value (`list_keys`) → `.into_future_stream()`

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
| D1WasmStorage | 1 | 15 | pub async; sync calls via schedule_future | AsyncKV + AsyncBlob + AsyncRate |
| R2WasmStorage | 1 | 4 | pub async; sync calls via schedule_future | AsyncBlob |
| KVWasmStorage | 1 | 12 | pub async; sync calls via schedule_future | AsyncKV + AsyncBlob + AsyncRate |
| D1KeyValueStore | 2 | 0 | extract public stream methods | AsyncKV + AsyncBlob + AsyncRate + AsyncQuery |
| R2BlobStore | 2 | 0 | extract public stream methods | AsyncBlob |
| TursoStorage | 3 | 0 | extract inline async to named pub async fn | AsyncKV + AsyncBlob + AsyncRate + AsyncQuery |
| LibsqlStorage | 3 | 0 | extract inline async to named pub async fn | AsyncKV + AsyncBlob + AsyncRate + AsyncQuery |
| MemoryStorage | 4 | 0 | no extraction — methods produce streams | AsyncKV + AsyncBlob + AsyncRate |
| MemoryJsonStore | 4 | 0 | no extraction — methods produce streams | AsyncKV + AsyncBlob + AsyncRate |
| JsonFileStorage | 4 | 0 | no extraction — methods produce streams | AsyncKV + AsyncBlob |

### Pattern A: wasm-bindgen backends (D1WasmStorage, R2WasmStorage, KVWasmStorage)

1. **Make private async methods public** — `get_async`, `set_async`, etc. are already fully implemented, just change visibility.
2. **Sync trait impls** — call the public async method via `schedule_future` + `map_circuit` + `map_done` to produce `StorageItemStream`. No `block_on`.
3. **Async trait impls** — call the public async method directly with `.await`. No valtron bridge needed.

### Pattern B: HTTP-based native backends (D1KeyValueStore, R2BlobStore)

1. **Public methods produce valtron streams** — extract the SQL/HTTP/parsing/deserialization logic into public methods returning `StorageItemStream<D, P>`. These are the single source of truth.
2. **Sync trait impls** — call the stream method directly (passthrough).
3. **Async trait impls** — bridge the stream via `.into_ready_future().await` (single value) or `.into_future_stream()` (multi-value).

### Pattern C: valtron-native backends (TursoStorage, LibsqlStorage)

1. **Extract `async move { ... }` blocks into named public async methods** — the business logic (SQL prep, encryption, deserialization) currently inline in `schedule_future` becomes a `pub async fn`.
2. **Sync trait impls** — call the public async method via `schedule_future(async { self.xxx_async(...).await })` + `map_circuit` (same outer wrapping, named method instead of inline).
3. **Async trait impls** — call the public async method directly with `.await`.

### Pattern D: simple sync backends (MemoryStorage, MemoryJsonStore, JsonFileStorage)

1. **No extraction needed** — methods already produce trivial streams directly.
2. **Sync trait impls** — unchanged (call stream methods directly).
3. **Async trait impls** — bridge existing streams via `.into_ready_future()` / `.into_future_stream()`.

### Pattern E: CredentialStorage — consumes streams from StorageProvider

1. **No extraction needed** — StorageProvider already produces streams.
2. **Sync trait impls** — drain streams via `.flat_map(...).next()` (single value) / `.collect()` (multi-value) (unchanged).
3. **Async trait impls** — call StorageProvider methods (return streams), bridge:
   - Single-value ops → `.into_ready_future().await`
   - Multi-value (`list_keys`) → `.into_future_stream()`

### Pattern F: NativeOAuth — inline sync HTTP

1. **Public methods produce valtron streams** — extract the HTTP/parsing logic into public methods returning `StorageItemStream`. These are the single source of truth.
2. **Sync methods** — call stream methods directly (passthrough).
3. **Async methods** — bridge via `.into_ready_future().await`.

### Pattern G: WasmOAuth — already follows Pattern A

1. **Make private async methods public** — already implemented.
2. **Sync trait impls** — call public async method via `schedule_future` + valtron bridge (no `block_on`).
3. **Async trait impls** — call public async method directly with `.await`.

## Architecture

### Async Storage Traits

Three new traits in `foundation_db/src/core/storage_provider.rs`, mirroring their sync counterparts:

- **AsyncKeyValueStore**:
  - Single-value → `Result<T>`: `get_async<V>`, `set_async<V>`, `delete_async`, `exists_async`
  - Multi-value → `impl Stream<Item = Result<String, StorageError>>`: `list_keys_async` (wraps `list_keys_async` pub async fn via `stream_many().into_future_stream()`)
- **AsyncBlobStore** — all single-value → `Result<T>`: `put_blob_async`, `get_blob_async`, `delete_blob_async`, `blob_exists_async`
- **AsyncRateLimiterStore** — all single-value → `Result<T>`: `check_rate_limit_async`, `record_rate_limit_async`, `reset_rate_limit_async`

### Async Auth Traits

New trait in `foundation_auth/src/shared/credential_store.rs`:

- **AsyncCredentialStore**:
  - Single-value → `Result<T>`: `get_async<V>`, `set_async<V>`, `delete_async`, `exists_async`
  - Multi-value → `impl Stream<Item = Result<String, CredentialStoreError>>`: `list_keys_async` (wraps pub async fn via `stream_many().into_future_stream()`)

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

These call the public stream-producing methods and bridge via `.into_ready_future().await`.

## Refactor Principle: Public Methods, Two Consumers

**Core rule: public methods contain the real logic. Both sync and async call them.** The correct approach depends on the backend's underlying APIs:

**For async-only APIs** — `pub async fn` is the source of truth, sync wraps via `schedule_future`:
- **Async-only libraries**: Turso, Libsql (their Rust crates only expose async APIs)
- **JS Promise APIs**: wasm-bindgen backends (D1WasmStorage, R2WasmStorage, KVWasmStorage, WasmOAuth) — JS Promises are natively async on the CF Workers runtime

**For all other backends** — public methods produce valtron streams (`StorageItemStream`), sync passthrough, async bridges via `.into_ready_future()` / `.into_future_stream()`:
- HTTP-native backends (D1KeyValueStore, R2BlobStore)
- Simple sync backends (MemoryStorage, MemoryJsonStore, JsonFileStorage)
- CredentialStorage (consumes StorageProvider streams)
- NativeOAuth (inline sync HTTP via SimpleHttpClient)

### Pattern A: wasm-bindgen backends (D1WasmStorage, R2WasmStorage, KVWasmStorage)

1. **Make private async methods public** — `get_async`, `set_async`, etc. already resolve JS Promises via `JsFuture`. Just change visibility.
2. **Sync trait impls** — call the public async method via `schedule_future(async { self.xxx_async(...).await })` + `map_circuit` + `map_done` to produce `StorageItemStream`. No `block_on` needed.
3. **Async trait impls** — call the public async method directly with `.await`. No valtron bridge needed.

### Pattern B: HTTP-based native backends (D1KeyValueStore, R2BlobStore)

1. **Public methods produce valtron streams** — extract the SQL/HTTP/parsing/deserialization logic from each sync trait method into public methods returning `StorageItemStream<D, P>`. These are the single source of truth.
2. **Sync trait impls** — call the public stream method directly (passthrough, no behavior change).
3. **Async trait impls** — call the same public stream method and bridge:
   - Single-value ops → `.into_ready_future().await` (resolves to `Result<T>`)
   - Multi-value ops (`list_keys`, `query`) → `.into_future_stream()` (returns `impl Stream<Item = Result<T, E>>`)

### Pattern C: valtron-native backends (TursoStorage, LibsqlStorage)

1. **Extract `async move { ... }` blocks into named `pub async fn`** — the business logic (SQL prep, encryption, deserialization) currently inline in `schedule_future` becomes a `pub async fn` returning `Result<T>`.
2. **Sync trait impls** — call the public async method via `schedule_future(async { self.xxx_async(...).await })` + `map_circuit` (same outer wrapping, named method instead of inline). Multi-value ops keep `run_future_iter` for lazy row iteration.
3. **Async trait impls** — call the public async method directly with `.await`.

### Pattern D: simple sync backends (MemoryStorage, MemoryJsonStore, JsonFileStorage)

1. **Public methods produce valtron streams** — methods already produce trivial streams directly (no change needed).
2. **Sync trait impls** — unchanged (call stream methods directly).
3. **Async trait impls** — call existing stream method, bridge via `.into_ready_future()` / `.into_future_stream()`. Since in-memory, streams resolve immediately.

### Pattern E: CredentialStorage — consumes StorageProvider streams

1. **No extraction needed** — StorageProvider already produces streams.
2. **Sync trait impls** — drain streams via `.flat_map(...).next()` (single value) / `.collect()` (multi-value) (unchanged).
3. **Async trait impls** — call StorageProvider methods (return streams), bridge:
   - Single-value ops → `.into_ready_future().await`
   - Multi-value (`list_keys`) → `.into_future_stream()`

### Pattern F: NativeOAuth — inline sync HTTP

1. **Public methods produce valtron streams** — extract the HTTP execution + parsing logic into public methods returning `StorageItemStream`. These are the single source of truth.
2. **Sync methods** — call the stream methods directly (passthrough).
3. **Async methods** — call the same stream methods and bridge via `.into_ready_future().await`.

### Pattern G: WasmOAuth — already follows Pattern A

1. **Make private async methods public** — already implemented.
2. **Sync trait impls** — call public async method via `schedule_future` + valtron bridge (no `block_on`).
3. **Async trait impls** — call public async method directly with `.await`.

## Implementation Strategy

### Phase 1: wasm-bindgen backends — Make Private Async Methods Public, Eliminate block_on

**Files:**
- `backends/foundation_db/src/wasm/wasm_storage/d1_wasm.rs` (15 methods)
- `backends/foundation_db/src/wasm/wasm_storage/r2_wasm.rs` (4 methods)
- `backends/foundation_db/src/wasm/wasm_storage/kv_wasm.rs` (12 methods)

Change visibility of all private async methods to `pub`. Replace `futures_lite::block_on` in all sync trait impls with `schedule_future(async { self.xxx_async(...).await })` + `map_circuit` + `map_done` to produce `StorageItemStream`. The public async methods are the source of truth; sync wraps them via valtron, async calls them directly.

### Phase 2: HTTP-based native backends — Extract Public Stream Methods

**Files:**
- `backends/foundation_db/src/core/backends/d1_kvstore.rs`
- `backends/foundation_db/src/core/backends/r2_blobstore.rs`

For each sync trait method, extract the SQL/HTTP/parsing logic into a public method returning `StorageItemStream`:

- **D1KeyValueStore**: `get_kv_stream`, `set_kv_stream`, `exists_stream`, `list_keys_stream`, `delete_stream`, `query_stream`, `execute_stream`, `execute_batch_stream`, `check_rate_limit_stream`, `record_rate_limit_stream`, `reset_rate_limit_stream`, `put_blob_stream`, `get_blob_stream`, `delete_blob_stream`, `blob_exists_stream`
- **R2BlobStore**: `put_blob_stream`, `get_blob_stream`, `delete_blob_stream`, `blob_exists_stream`

The existing private helpers (`execute_sql`, `extract_rows`, `wrap_value`, `wrap_vec` for D1; `body_bytes`, `wrap_value` for R2) stay unchanged.

Sync trait impls call these async methods via `schedule_future` + `map_circuit` + `map_done` to produce `StorageItemStream`.

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

Implement async traits for MemoryStorage, MemoryJsonStore, JsonFileStorage. Since these are in-memory, async methods call existing sync trait methods (which return `StorageItemStream`) and bridge via `.into_ready_future()` / `.into_future_stream()`. Streams resolve immediately.

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

Make private async methods public (async is source of truth, same as Pattern A — JS Promises are natively async). Replace `futures_lite::block_on` in sync methods with `schedule_future` + valtron bridge. Async trait impl calls public async methods directly.

### Phase 8: NativeOAuth — Extract Helpers + Add Async Methods

**File:** `backends/foundation_auth/src/native/oauth.rs`

Factor out shared logic into public stream-producing methods:

- **Body builders** — form-urlencoded body construction for each OAuth flow.
- **Response parsers** — token parsing and refresh token parsing.
- **HTTP executor** — the SimpleHttpClient POST call, wrapped in valtron `StorageItemStream`.

Sync methods call stream methods directly (passthrough). Async methods bridge via `.into_ready_future().await`.

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
- **Async HTTP serve**: register a `CfServe` handler that `.await`s a storage operation, verify response contains the stored value
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
  ├── Public stream methods (NEW — extracted from current trait impls)
  ├── Sync trait impls (refactored to passthroughs)
  ├── impl AsyncKeyValueStore (NEW)
  ├── impl AsyncBlobStore (NEW)
  ├── impl AsyncRateLimiterStore (NEW)
  └── impl AsyncQueryStore (NEW)

backends/foundation_db/src/core/backends/r2_blobstore.rs
  ├── Public stream methods (NEW — extracted from current trait impls)
  ├── Sync trait impls (refactored to passthroughs)
  └── impl AsyncBlobStore (NEW)

backends/foundation_db/src/native/turso_backend.rs
  ├── Public async methods (extracted from inline schedule_future blocks)
  ├── Sync trait impls (refactored: call pub async via schedule_future)
  ├── impl AsyncKeyValueStore (NEW)
  ├── impl AsyncBlobStore (NEW)
  ├── impl AsyncRateLimiterStore (NEW)
  └── impl AsyncQueryStore (NEW)

backends/foundation_db/src/native/libsql_backend.rs
  ├── Public async methods (extracted from inline schedule_future blocks)
  ├── Sync trait impls (refactored: call pub async via schedule_future)
  ├── impl AsyncKeyValueStore (NEW)
  ├── impl AsyncBlobStore (NEW)
  ├── impl AsyncRateLimiterStore (NEW)
  └── impl AsyncQueryStore (NEW)

backends/foundation_db/src/core/backends/memory.rs
  ├── Sync trait impls (unchanged — already produce streams)
  ├── impl AsyncKeyValueStore (NEW)
  ├── impl AsyncBlobStore (NEW)
  └── impl AsyncRateLimiterStore (NEW)

backends/foundation_db/src/core/backends/memory_json.rs
  ├── Sync trait impls (unchanged — already produce streams)
  ├── impl AsyncKeyValueStore (NEW)
  ├── impl AsyncBlobStore (NEW)
  └── impl AsyncRateLimiterStore (NEW)

backends/foundation_db/src/native/json_file.rs
  ├── Sync trait impls (unchanged — already produce streams)
  ├── impl AsyncKeyValueStore (NEW)
  └── impl AsyncBlobStore (NEW)

backends/foundation_db/src/wasm/wasm_storage/d1_wasm.rs
  ├── Existing private async methods → make pub (source of truth)
  ├── Sync trait impls (refactored: call pub async via schedule_future, no block_on)
  ├── impl AsyncKeyValueStore (NEW)
  ├── impl AsyncBlobStore (NEW)
  ├── impl AsyncRateLimiterStore (NEW)
  └── impl AsyncQueryStore (already exists)

backends/foundation_db/src/wasm/wasm_storage/r2_wasm.rs
  ├── Existing private async methods → make pub (source of truth)
  ├── Sync trait impls (refactored: call pub async via schedule_future, no block_on)
  └── impl AsyncBlobStore (NEW)

backends/foundation_db/src/wasm/wasm_storage/kv_wasm.rs
  ├── Existing private async methods → make pub (source of truth)
  ├── Sync trait impls (refactored: call pub async via schedule_future, no block_on)
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
  ├── Public stream methods (NEW — extracted from current sync methods)
  ├── Sync methods (refactored to passthroughs)
  └── Async methods (bridge streams via .into_ready_future())

backends/foundation_auth/src/wasm_bindgen/oauth.rs
  ├── Existing private async methods → make pub (source of truth)
  └── Sync methods (refactored: call pub async via schedule_future, no block_on)

backends/foundation_http/src/wasm/serve_cf.rs
  ├── CfServe (converted to #[async_trait(?Send)])
  └── CfServeFactory (existing, unchanged)

backends/foundation_http/src/wasm/serve_web.rs
  ├── WebServe (converted to #[async_trait(?Send)])
  └── WebServeFactory (existing, unchanged)

backends/foundation_http/src/wasm/dispatch.rs
  ├── dispatch_cf (converted to async fn, handler call becomes .await)
  └── dispatch_web (converted to async fn, handler call becomes .await)

backends/foundation_http/src/wasm/bridge/cf.rs
  └── CfHttpApp (handle_request calls dispatch_cf(...).await)
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

Adding `_async` methods that duplicate sync logic leads to drift. Factor out the real work into public methods first, then both sync and async call them. For async-only backends (Turso/Libsql/wasm-bindgen), the public method is `pub async fn`. For all others, it produces `StorageItemStream`. This ensures no logic is duplicated.

### Public methods prevent duplication

The correct approach depends on whether the backend uses async-only APIs:

- **Async-only (Turso/Libsql/wasm-bindgen)**: `pub async fn` is the source of truth. Both sync and async call the same method — sync via `schedule_future`, async via `.await`.
- **All others**: public methods produce `StorageItemStream`. These are the source of truth. Sync calls directly (passthrough). Async bridges via `.into_ready_future()` / `.into_future_stream()`.

### Single-value vs multi-value in async traits

For backends where async is the source of truth (Turso/Libsql/wasm-bindgen), single-value async methods return `Result<T>` directly. Multi-value async methods return `Result<Vec<T>>`.

For backends where streams are the source of truth, the async TRAIT bridges existing streams: single-value → `.into_ready_future().await`, multi-value → `.into_future_stream()`. The caller chooses whether to collect; the trait does not force materialization.

### block_on elimination for wasm backends

On wasm32-unknown-unknown, async methods that resolve JS Promises via `JsFuture` are natively async — they don't need a thread pool. The `futures_lite::block_on` was only needed because sync trait methods were the only public API. With public async methods as the source of truth, sync trait impls call them via `schedule_future` (for wasm this runs on the single-threaded event loop, no actual blocking). Async trait impls call them directly with `.await`.

### Parallel scan in get_session_async

`get_session` does a linear scan of all session keys. The async version should parallelize `get_async` calls using `join_all` or similar. This is a real performance improvement over the sync version.

---

_Created: 2026-05-20_

# Spec 50 Progress: Valtron Async Readiness

_Updated: 2026-10-21_

## Status: In progress

The feature front matter still says `pending`; the code on master is ahead of
it. What master has, checked against the source:

| Feature | State on master | Evidence |
|---------|-----------------|----------|
| F01 — waker → `QueueReadiness` bridge | Present | `future_task.rs`: parking `FutureTask`/`StreamTask` use `queue_waker`; the no-op waker is kept only for the non-`multi` inline re-poll path |
| F02 — readiness-source seam | Not verified | No reactor/readiness-source module found under `valtron/`; needs a check before marking either way |
| F03 — async `#[valtron]` / `#[valtron_test]` | Present | `foundation_macros/src/valtron_entry.rs` accepts `async fn` |

## Landed

### PR #130 — async-first valtron API (merged, `591de4d1b`)

- Async names: `values` / `items` / `collect_async` / `first`, and
  `TaskStatus::from_stream`.
- Sync only at the boundary: `BlockingFutureExt::block()` and
  `BlockingStreamExt::blocking()`. Both are native-only and compiled out on
  JS-hosted wasm (`foundation_core/js-wasmbindgen`), so blocking in a browser
  or Worker is a compile error, not a spin.
- Idle executors wake as soon as a parked task becomes ready (was up to a
  100 ms back-off).
- `send` / `execute` return `Err` with no pool instead of panicking.
- `#[valtron_test]` timeout: `PoolGuard::shutdown_within` tears the pool down
  in bounded time, detaching workers still stuck in a task, so one hung test
  fails instead of hanging the suite.
- Fixes found on the way: `foundation_connectrpc` runs the `H2Transport`
  handshake in the pump, not in `open()`; `foundation_proxy` gained
  `ProxyServer::shutdown_async` so async tests stop blocking workers.

## In progress

### foundation_db: async-only stores (branch `db/async-stores`, PR not open yet)

Done (committed):
- `foundation_core`: `valtron::BLOCKING_BOUNDARY` + `blocking_boundary!` macro
  (expands only where `.block()` exists), with a compile-time probe that fails
  the build if the macro and the blocking helpers ever disagree.
- `foundation_db`:
  - `Blocking<S>` adapter (`src/blocking.rs`): sync `KeyValueStore`,
    `BlobStore`, `QueryStore`, `RateLimiterStore`, `DocumentStore` (and so
    `StorageCleanup`) over the `Async*` traits.
  - Sync impls deleted from Turso, libsql, D1, R2, `JsonFileStorage`,
    `StorageProvider` and the Cloudflare D1/KV/R2 Worker stores. Every
    `exec_future` / `wrap_async_value` helper is gone.
  - New async impls: `AsyncQueryStore` + `AsyncRateLimiterStore` for
    `JsonFileStorage`, `AsyncQueryStore` for `KVWasmStorage`, `MemoryStorage`
    and `MemoryJsonStore`.
  - One SQL document store: `SqlDocumentStore<Q: AsyncQueryStore>`
    (`AsyncSqlDocumentStore` removed).
  - Turso/libsql constructors, `init_schema`/`init_kv`/`migrate` and
    `StorageProvider::new` are `async fn`. `D1Store::init_kv` is async too.
  - `wasm-bindgen-bindings` turns on `foundation_core/js-wasmbindgen`, so
    `Blocking` is absent whenever the Cloudflare bindings are built
    (compile-time asserted in `lib.rs`).
- `cargo test --profile uat -p foundation_db`: all pass, including the new
  `blocking_tests` (no pool) and `blocking_executor_tests` (pool, Pending
  futures/streams, Turso, `StorageCleanup`).

Left: callers in the other crates, docs + bug backlog, and the `ci.py` runs
per touched crate.

## Next

- Follow-up PR: move foundation_ai's `AgentSession` stores (MessageApi,
  MemoryStore/KvMemoryStore, MemoryCoordinator, ContextProvider, tools) to the
  `Async*` traits, then delete the remaining sync impls and sync traits.
- Confirm F02's state and update all three features' front matter.

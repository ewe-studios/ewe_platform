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

Callers (committed):
- foundation_auth:
  - The wasm OAuth client is async-only (`exec_future` drain removed).
  - The D1 credential store is async-only.
  - `CredentialStorage`'s sync impl and `AuthManager` sit behind
    `blocking_boundary!`.
  - The `SessionManager` async methods need only `AsyncCredentialStore`.
  - The inline `provisioning_service` tests moved to `tests/`.
- foundation_vectors, foundation_ai (the SQL session test),
  foundation_keychain: use `Blocking` / await the async constructors.
- foundation_nativeapis: the D1/R2 VFS deltas hold `Blocking<_>`. This also
  repaired code that didn't compile with `vfs-native,vfs-d1,vfs-r2`.
- cf-login-app: awaits `StorageProvider::new`. Its manifest is fixed; it
  couldn't load because it inherited workspace fields while excluded from
  the workspace.
- Docs, bug backlog, and a CI wasm run for foundation_db's Cloudflare bindings.

Checks (`scripts/ci/ci.py run <crate> --step build --step clippy --step test --step wasm`):

| Crate | Result |
|-------|--------|
| foundation_core | pass (both test runs, 3 wasm runs) |
| foundation_db | pass |
| foundation_vectors | pass |
| foundation_auth | pass |
| foundation_nativeapis | pass |
| foundation_keychain | pass |
| foundation_ai | running |

Extra checks:
- foundation_ai `--no-default-features --features multi,testing --test
  agentic_tests`: 280 pass.
- foundation_auth `server-test`: 17 IdP HTTP tests fail, on master too
  (backlog §12).
- wasm32 builds:
  - foundation_db `wasm` and `wasm-bindgen-storage`;
  - foundation_auth `wasm`, `wasm-bindgen-session`, `wasm-bindgen-oauth`;
  - foundation_ai `--no-default-features`;
  - foundation_keychain;
  - `examples/cf-login-app` and `examples/cf-agent-loop`.

Left: the foundation_ai CI run; then report back (no PR).

## Next

- Follow-up PR: move foundation_ai's `AgentSession` stores (MessageApi,
  MemoryStore/KvMemoryStore, MemoryCoordinator, ContextProvider, tools) to the
  `Async*` traits, then delete the remaining sync impls and sync traits.
- Then the other sync storage callers (backlog §11): foundation_auth server
  services, foundation_cedar, foundation_vectors, foundation_cronjobs,
  `MigrationRunner::run`, `StorageCleanup`; and an async `StateStore`.
- Confirm F02's state and update all three features' front matter.

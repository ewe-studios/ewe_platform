# Spec 50 Progress: Valtron Async Readiness

_Updated: 2026-10-21_

## Status: In progress

## The async-storage stream (overall plan and status)

_Recorded 2026-10-21._ Goal: async all the way, with sync only at real boundaries, so the same code runs natively and on Cloudflare Workers (where nothing may block).

| Step | PR / branch | State |
|---|---|---|
| valtron async-first API (`.block()` / `.blocking()` native-only, `blocking_boundary!`) | #130 | ✅ merged |
| foundation_db: I/O-backed stores async-only; `Blocking<S>` for sync callers | #131 | ✅ merged |
| foundation_ai: `AgentSession` on the `Async*` traits; in-memory stores async-only; `build()` / `end()` async | #132 | ✅ merged |
| Remaining sync callers (foundation_auth server, cedar, vectors, cronjobs, `MigrationRunner`, `StorageCleanup`), async `StateStore`, no blocking `Default`, then delete the sync storage traits; backlog §11 fixes and CI runs | `db/async-callers` (draft PR #133, spec 50 F04) | 🔧 in progress: T1–T2 done |

**After this stream:** spec 65's agent-rpc review round 2 on PR #126. The plan is recorded in `specifications/65-agent-rpc-serving-system/progress.md`: pluggable traits for bare metal / Docker / Workers, auth moving into foundation_auth with a `CaStore` trait, and a `ClientAuth` provider built on our signed JSON secret (not a JWT).

Also still open here: confirm whether F02 (the readiness-source seam) exists, then correct the three features' front matter.


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

### foundation_db: async-only stores (PR #131, merged)

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
| foundation_ai | pass. Steps run one at a time with `target/uat` wiped in between, for disk: `build`, `clippy`, the two `test` runs (`testing`; `--no-default-features --features testing`) as the exact `cargo test` commands ci.py issues, then `wasm`. |

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

Merged as PR #131 (merge commit `c84382913`).

### foundation_ai on the async store traits (PR #132, merged)

Done (committed and pushed):
- foundation_ai: `MessageApi`, `KvMemoryStore`, `MemoryCoordinator`,
  `MemoryHierarchy`, `ContextProvider`, the memory/search/agent tools,
  `AgentSession` and its builder are generic over `AsyncDocumentStore` /
  `AsyncKeyValueStore` and await them.
- `AgentLoop` awaits the stores from its own states: `InnerAssembling`
  (context assembly), `Flushing` (when the buffer reaches
  `MessageApiConfig::flush_threshold`, and at turn end) and `Summarizing`.
  It polls a `FutureTask` in place, so a ready store adds no step and a
  waiting one parks the task. Store failures end the turn with a
  `FailedAction` (`AgenticError::MessageStore`).
- Breaking API: `build()` / `end()` async; `MessageApi` reads and `flush`
  async, `append` buffers only, `with_config` takes `MessageApiConfig`;
  `ContextProvider::{assemble, assemble_from_memory, search,
  search_from_memory}` async and return `StorageResult`;
  `MemoryStore::hydrate_sync` removed.
- foundation_db: `MemoryStorage`, `MemoryJsonStore`, `MemoryDocumentStore`
  lost their sync impls. Small fixes there: wasm-safe clock, no unwrap or
  underflow in the rate limiters, `get_blob_async` reports a non-blob value,
  no panic on a poisoned lock.
- Callers: foundation_auth server defaults to `Blocking<MemoryStorage>`;
  foundation_cedar and foundation_cronjobs tests use `Blocking` (cronjobs'
  inline tests moved to `tests/`); answerme-agent, cf-agent-loop and
  cf-cloud-provider await `build()` / `end()`.
- Docs (foundation_ai README and docs 00–12, foundation_db docs 00/01) and
  the bug backlog are updated.

Checks (`scripts/ci/ci.py run <crate> --step build --step clippy --step test --step wasm`):

| Crate | Result |
|-------|--------|
| foundation_db | pass |
| foundation_cedar | pass; `--features db` tests (not in CI) pass too |
| foundation_cronjobs | pass |
| foundation_auth | pass; `server-test`: 124 pass, the 17 known IdP HTTP failures (backlog §12) |
| answerme-agent | pass (first run failed: `block_on_future` needs a `'static` future, so `AgentSession::end` now returns one, like `run_turn`) |
| foundation_ai | pass, steps one at a time with `target/uat` wiped in between: `build`, `clippy`, `test` run 1 (`testing`: one flaky provider test failed once, fixed — it recorded only the last request path), `test` run 2 (`--no-default-features --features testing`), `wasm` |

Extra: `examples/cf-agent-loop` and `examples/cf-cloud-provider` build for
wasm32 (separate target dir, deleted after).

## In progress

### Remaining sync storage callers (branch `db/async-callers`, draft PR #133)

Plan: `features/04-async-only-storage/feature.md` (tasks T1–T14). Backlog §11,
crate by crate, then an async `StateStore`, no blocking `Default`, and
deleting the sync storage traits.

| Task | State | Checks |
|------|-------|--------|
| T1 foundation_auth IdP server on the async traits | done | `ci.py run foundation_auth` (build, clippy, test, wasm): pass. `cargo test --features server-test`: 138 pass, 0 fail (the 17 IdP failures are fixed: bodies were never read, and handler errors became 500) |
| T2 foundation_auth credential/session layer async-only | done | `cargo test -p foundation_auth`: pass (22 AuthManager tests moved to `tests/`); `--features server-test`: 138 pass; wasm `wasm`, `wasm,wasm-bindgen-session`, `wasm,wasm-bindgen-oauth`: build; `examples/cf-login-app` wasm32: builds |
| T3–T14 | pending | |

## Next

- Remove the sync storage traits once the other sync callers have moved.
- The other sync storage callers (backlog §11): foundation_auth server
  services, foundation_cedar, foundation_vectors, foundation_cronjobs,
  `MigrationRunner::run`, `StorageCleanup`; and an async `StateStore`.
- Confirm F02's state and update all three features' front matter.

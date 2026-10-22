# Spec 50 Progress: Valtron Async Readiness

_Updated: 2026-10-22_

## Status: storage layer async-only (pending #134 merge); F05 (remaining sync twins) and F02 open

## The async-storage stream (overall plan and status)

_Recorded 2026-10-21._ Goal: async all the way, with sync only at real boundaries, so the same code runs natively and on Cloudflare Workers (where nothing may block).

| Step | PR / branch | State |
|---|---|---|
| valtron async-first API (`.block()` / `.blocking()` native-only, `blocking_boundary!`) | #130 | ✅ merged |
| foundation_db: I/O-backed stores async-only; `Blocking<S>` for sync callers | #131 | ✅ merged |
| foundation_ai: `AgentSession` on the `Async*` traits; in-memory stores async-only; `build()` / `end()` async | #132 | ✅ merged |
| Remaining sync callers (foundation_auth server, cedar, vectors, cronjobs, `MigrationRunner`, `StorageCleanup`), async `StateStore`, no blocking `Default`, then delete the sync storage traits; backlog §11 fixes and CI runs | #133 (spec 50 F04) | ✅ merged (`7db6b473a`) |
| Rename the async storage traits to plain names (`AsyncQueryStore` → `QueryStore`, …); drop `SessionManager`'s `_async` suffixes | #134 (`db/rename-async-traits`) | ✅ done; CI and review (last step of the stream) |

**The storage part is complete when #134 merges.** No sync storage trait and no `Blocking<S>` remain;
sync code exists only at real boundaries (`main`, sync third-party traits), where it waits through
valtron's `.block()` / `block_on_future` behind `blocking_boundary!`.

**Not yet complete:** a survey on 2026-10-22 found two more sync twins with I/O-backed
implementations: the vector store (`foundation_vectors`) and the virtual file system
(`foundation_nativeapis`). Feature 05 tracks them, with two owner decisions.

**After this stream:** spec 65 feature 07 (pluggable agent-rpc) on PR #126. Its full design is in
`specifications/65-agent-rpc-serving-system/features/07-pluggable-agent-rpc/`, with the owner's
decisions. Task T0 (merge `master` into #126's branch) starts it.

Still open in this spec: F05 (remaining sync twins), and F02 (the readiness-source seam), which is **not implemented** (checked 2026-10-22: no `ReadinessSource` trait in `foundation_core/src/valtron`; only an old `Reactor` example in `valtron/docs/thread_looping.md`). `requirements.md` lists both.


The feature front matter still says `pending`; the code on master is ahead of
it. What master has, checked against the source:

| Feature | State on master | Evidence |
|---------|-----------------|----------|
| F01 — waker → `QueueReadiness` bridge | Present | `future_task.rs`: parking `FutureTask`/`StreamTask` use `queue_waker`; the no-op waker is kept only for the non-`multi` inline re-poll path |
| F02 — readiness-source seam | **Not present** | Checked 2026-10-22: no `ReadinessSource` trait under `valtron/`; the only `Reactor` text is an example in `valtron/docs/thread_looping.md` |
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

### Remaining sync storage callers (PR #133, merged `7db6b473a`)

Plan: `features/04-async-only-storage/feature.md` (tasks T1–T14). Backlog §11,
crate by crate, then an async `StateStore`, no blocking `Default`, and
deleting the sync storage traits.

| Task | State | Checks |
|------|-------|--------|
| T1 foundation_auth IdP server on the async traits | done | `ci.py run foundation_auth` (build, clippy, test, wasm): pass. `cargo test --features server-test`: 138 pass, 0 fail (the 17 IdP failures are fixed: bodies were never read, and handler errors became 500) |
| T2 foundation_auth credential/session layer async-only | done | `cargo test -p foundation_auth`: pass (22 AuthManager tests moved to `tests/`); `--features server-test`: 138 pass; wasm `wasm`, `wasm,wasm-bindgen-session`, `wasm,wasm-bindgen-oauth`: build; `examples/cf-login-app` wasm32: builds |
| T3 foundation_cedar async `PolicyStore` | done | `ci.py run foundation_cedar` (fmt, build, clippy, test, wasm): pass; `--features db`: all pass, incl. new `tests/sql_store.rs` (first `SqlPolicyStore` tests, over Turso) |
| T4 foundation_vectors SQL stores on `AsyncVectorStore` | done | `ci.py run foundation_vectors` (build, clippy, test, wasm): pass; `--features sql-store`: all pass (SQL 11, libsql 5); new `TopK` accumulator: search ranks rows as they stream (O(k) memory) |
| T5 foundation_cronjobs async `CronScheduler` | done | `ci.py run foundation_cronjobs` (build, clippy, test, wasm): pass; new Turso tests: save/load round trip, a bad row is an error, save without the table is an error |
| T6 `MigrationRunner::run` removed; `AsyncStorageCleanup` + `CleanupConfig` | done | `ci.py run foundation_db` (build, clippy, test, wasm): pass (run with `CARGO_PROFILE_UAT_DEBUG=0`: with debuginfo the ~40 test binaries filled the disk). New cleanup tests over Turso (retention honoured, only expired rows removed) |
| T7 foundation_nativeapis off the sync traits | done | `ci.py run foundation_nativeapis`: pass; `--features vfs-native,vfs-d1,vfs-r2,vfs-fjall`: `blob_delta_vfs` 12 new tests (the delta over the in-memory blob store, async and through `SyncFs`), `fjall_document_store` 19, `d1_delta_vfs` 9, `r2_delta_vfs` 9 (the D1/R2 integration tests skip without the emulator) |
| T8 async `StateStore` and its callers (deployment crates, proxy) | done | `ci.py run` (build, clippy, test, wasm): foundation_db, foundation_deployment, foundation_deployment_docker, foundation_deployment_hetzner, foundation_proxy: all pass. New: `libsql_state_store_tests` 5 (first libsql state coverage), `r2_state_store_tests` 6 (in-process R2 mock, paged listings), `NamespacedStore::set` namespace test. Bugs fixed on the way: libsql/D1 state `init` was a SQL syntax error, libsql/D1 `find_by_status` never matched, R2 `find_by_*` didn't filter and listing read one page only, `NamespacedStore::set` wrote outside its namespace |
| T9 agent tool store factory; `builder_for` and the blocking `Default`s removed | done | `ci.py run foundation_ai` build, clippy, wasm: pass; test: 2730 pass, 0 fail (a first run failed to link with the disk full; rerun clean). New agent-tool tests: a factory builds stores per `start`, a failing factory fails `start`. foundation_db `default_stores_tests` 5 (now over `in_memory()`), `libsql_document_store_tests` (now a real libsql run). `examples/cf-agent-loop`, `examples/cf-cloud-provider` wasm32: build |
| T10 sync storage traits and `Blocking<S>` deleted | done | `cargo check --workspace --tests`: pass; `ci.py run` (build, clippy, test, wasm): foundation_db, foundation_auth pass; foundation_db wasm32 `--features wasm-bindgen-storage`: builds. Ten foundation_db test files moved off `Blocking` onto the async traits. Examples: `cf-agent-loop`, `cf-cloud-provider`, `cf-login-app` (wasm32) and `apps/answerme-agent` (native): build |
| T11 §11 small items: rate-limit window, target-specific dependencies | done | Shared fixed-window logic in `core/rate_limit.rs` (a busy key now resets when its window ends; every SQL backend uses the migration-014 table); `tests/rate_limit_window_tests.rs`. `worker` wasm-only, `async-stream` and the full `foundation_netio` native-only. `cargo check -p foundation_db --all-features` clean; wasm32 `--features wasm-bindgen-storage`: no foundation_db warnings (removed the dead `get_num_field`) |
| T12 CI runs in `ci/packages.toml` | done | New runs: foundation_auth `server-test`, foundation_db `libsql`, foundation_vectors `sql-store`, foundation_nativeapis `vfs-native,vfs-d1,vfs-r2`. GitHub CI on the T11 commit (all runs): pass |
| T13 docs and records | done | foundation_db / foundation_auth / foundation_ai docs; `docs/bug-backlog.md` §11 (fixed entries removed, new ones added); `learnings.md`; F04 status `done` |
| T14 `Async*` → plain-name rename | moved out | Follow-up PR after #133 merges (owner decision) |

### Rename to plain trait names (PR #134)

Mechanical, names only. 129 files.

| Item | Detail |
|---|---|
| Renamed traits | `QueryStore`, `DocumentStore`, `KeyValueStore`, `BlobStore`, `CredentialStore`, `RateLimiterStore`, `AuthStore`, `TosStore`, `StorageCleanup`, `PasskeyStore` (each lost `Async`) |
| Renamed methods | `SessionManager::{create_session, get_session, revoke_session, revoke_all_sessions, extend_session}` (each lost `_async`) |
| Kept | `AsyncDeltaStore`, `AsyncVectorStore` (sync `DeltaStore` / `VectorStore` traits still exist); every storage trait method's `_async` suffix (owner did not decide; feature 05 task T3) |
| Checks | `cargo check --workspace --tests`: pass. wasm32: foundation_db (`wasm`, `wasm-bindgen-storage`), foundation_auth (`wasm`, `wasm,wasm-bindgen-session`, `wasm,wasm-bindgen-oauth`), foundation_ai (`--no-default-features`): pass. Examples `cf-agent-loop`, `cf-cloud-provider`, `cf-login-app` (wasm32) and `apps/answerme-agent` (native): pass |

## Next

1. Merge #134.
2. Feature 05 (remaining sync twins): the vector store first; the VFS and the method suffixes
   after the owner decides (`features/05-remaining-sync-twins/feature.md` section 4).
3. Then spec 65 feature 07 (agent-rpc), task T0.
4. F02 (readiness-source seam) and the F01/F03 front matter.

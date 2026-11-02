# Bug and limitation backlog

This is the single list of known bugs and limitations that are still open.

CLAUDE.md rule 9 says every bug found gets tracked and scheduled. This file is where they are tracked. Each entry names the crate and file, says what is wrong, and gives the likely fix.

How to use it:
- When you find a new bug, add it here in the same change, even if you are not fixing it.
- When you fix one, delete its entry in the PR that fixes it and name the PR in the commit message.
- Group entries by crate. Mark limits that are still hard-coded constants with **(rule 8)**.

Last refreshed: 2026-10-27, on `fix/foundational-backlog`: sections 1–5 checked entry by entry against master `38398e928` (fixed entries removed, partly fixed ones rewritten; the table is in that branch's PR). Before that: 2026-10-22, on `nativeapis/async-vfs` (spec 50 F05 T2, the VFS async-first); 2026-10-21, on `ai/async-stores` and `db/async-stores`; 2026-10-20, after PR #96. In flight on branches:
- `fix/foundational-backlog`: sections 1–5 (foundation_connectrpc, netio, http, core/valtron, nostd/wasm/iogate).
---

## 1. foundation_connectrpc

(no open entries)

## 2. foundation_netio

(no open entries)

## 3. foundation_http

(no open entries)

## 4. foundation_core / valtron

- **A single-thread `DrivenStreamIterator` cannot park an async consumer**
  (`executors/non_sendables.rs`, `wake`). The iterator drives the engine in
  `next`, and a parked task becomes runnable when a foreign thread flips its
  readiness (`WakeSignal`, a future's waker, a socket) - which pushes nothing
  and fires no queue wake. So its `wake` only registers on the queue and the
  inner pull still blocks once (`max_spins` timed waits); an async consumer
  (`into_collect_future` under smol/tokio) whose task stays parked for more
  than about two of those waits returns `Pending` with nothing to wake it.
  Likely fix: let `EventReadiness` sources register a wake (a `Send` callback;
  `WakeCallback` is not `Send` without `multi`), and have this `wake` register
  on the sleepers of the engine as well as on the queue.
## 5. foundation_nostd / foundation_wasm / foundation_iogate

(no open entries)

## 6. foundation_deployment_docker / foundation_deployment

- **Docker POST requests with a body stall ~2 minutes on `100 Continue`.**
  Seen against OrbStack 29.8.2: `containers/create` logs "Connecting to URL
  .../containers/create" and then nothing for exactly 2 minutes, until
  "Received intro for request: (Continue, HTTP11, ...)" arrives and the body
  is sent. Every daemon-backed POST (`containers/create`, `containers/start`)
  pays the same 2-minute stall, so the Docker integration tests
  (`docker_macro_tests`, `container_integration`, `image_build_integration`,
  `docker_provider_tests`) take 10+ minutes each and look hung — their
  `redis:7-alpine` containers sit in `Created` state. GET requests
  (`/version`, image inspect) are fast. Fix: do not send
  `Expect: 100-continue`, or send the request body immediately instead of
  waiting for the interim response in the HTTP transport's
  `GetRequestIntroTask` path.

## 7. foundation_deployment_platform

- **CI does not lint the `vms` feature tree.** The CI clippy and build steps run
  default features only (`runs[0] = ""`); `vms` compiles only under
  `cargo test --features vms`, which does not lint. The `vms` tree is now at
  zero clippy warnings (cleaned 2026-11-02), but nothing keeps it there.
  Fix: extend the CI clippy run to the `vms` feature so this debt stays zero.

## 8. foundation_openapi / foundation_codegentools

- **Pre-existing clippy warnings.** `cargo clippy` reports ~112 lints in
  `foundation_openapi` (mostly `unified/generator.rs` and `unified/analyzer.rs`:
  docs missing backticks, `too_many_lines`, `map(...).unwrap_or(false)`, an
  unused `self`, consecutive `str::replace`, trailing commas), ~231 in
  `foundation_deployment_docker` (`format!` inline variables, trailing commas,
  `::default()` calls, missing `# Errors` sections), and ~67 in
  `foundation_codegentools` (`format!` inline variables, redundant closures,
  `too_many_lines`, missing `# Errors`/`# Panics` sections). None come from
  feature 14's change; a dedicated clippy-cleanup pass should clear them, then
  keep them at zero.

## 9. foundation_proxy

- **`forward_http` relays on a dedicated thread and returns `Take`.** Feature 08
  made the h1 `Serve` handler able to return `ConnectionResult::Async`, so a
  long-lived handler parks instead of holding a worker. The proxy still relays on
  a throwaway thread and returns `Take`. It can move to `Async` later once its
  relay future is `'static`.

## 10. foundation_ai

- **The `fullfp16` target feature is unstable in rustc.** `.cargo/config.toml`
  sets `-C target-feature=+fullfp16` for `aarch64-apple-darwin` (the gemm-f16
  candle fix). Clippy/rustc warns the feature name is "unknown and unstable",
  will become a hard error in a future release, and suggests `fp16`. Fix: use
  the stable feature name — verify that `fp16` still enables full half-precision
  arithmetic (`vfmaq_laneq_f16`) on Apple Silicon — or gate the flag behind the
  rustc version that accepts it.

## 11. foundation_db and its sync callers

Follow-ups from making the I/O backends async-only (branch `db/async-stores`).
- **Auth tables mix timestamp units.** The `created_at` defaults in the core
  migrations are milliseconds (`strftime('%s','now') * 1000`), while
  `AsyncStorageCleanup` compares `expires_at` with Unix seconds and the IdP
  server writes milliseconds for its own rows. Fix: pick one unit per column,
  document it in the migrations, and make cleanup follow it.
- **`KVWasmStorage`'s rate limiter isn't atomic**
  (`src/wasm/wasm_storage/kv_wasm.rs`): a record reads the entry, then
  writes it back, so two concurrent hits can count once (and KV is
  eventually consistent across locations). Fix: use a Durable Object or D1
  for rate limits on Workers, and document KV as best-effort.
- **Databases created by the old rate-limit code have the wrong table.**
  Before F04 T11 a store that wasn't migrated created
  `rate_limits (key PRIMARY KEY, count, window_start)`; the store now uses the
  migration 014 shape (`id`, `key UNIQUE`, `count`, `window_start`,
  `window_end`), which `CREATE TABLE IF NOT EXISTS` won't add to an existing
  old-shape table. Fix: a migration that rebuilds an old-shape table (rate
  limit rows are disposable), or drop it on detection.
- **The wasm stores keep a second, inherent `*_async` API.** `D1WasmStorage`,
  `KVWasmStorage` and `R2WasmStorage` (`src/wasm/wasm_storage/*.rs`) have
  public inherent methods with the old trait names (`get_async`,
  `query_async`, `put_blob_async`, ...). They return `!Send` futures, and
  `D1WasmStorage::list_keys_async` / `KVWasmStorage::list_keys_async` return
  a `Vec` instead of a stream. The trait impls, `StorageProvider`,
  `WasmCredentialStore`, `foundation_auth`'s `CredentialStore for
  D1WasmStorage` and `examples/cf-login-app` call these. Spec 50 F05 T3
  dropped the suffix from the trait methods, so the two APIs no longer share
  a name, but a caller can still pick either by accident. Fix: make the
  inherent methods private helpers (as the native stores' `*_internal` ones
  are) and route every caller through the traits (`list_keys` +
  `collect_all` where a `Vec` is needed).
- **No CI job builds the example workspaces** (`examples/cf-*`,
  `apps/answerme-agent`); `examples/cf-login-app`'s manifest had stopped
  loading unnoticed. Fix: an examples job (wasm32 for `cf-*`). The feature
  runs that had rotted (foundation_db `libsql`, foundation_nativeapis
  `vfs-native,vfs-d1,vfs-r2`, foundation_vectors `sql-store`, foundation_auth
  `server-test`) are in `ci/packages.toml` since F04 T12.

## 12. foundation_auth

- **The `server`/`server-native` feature tree has over 200 clippy warnings.**
  `cargo clippy -p foundation_auth --features server-native` reports 212 lints
  across `src/server/` (`config.rs`, `handlers/*`, `services/*`): redundant
  closures, missing `#[must_use]`, docs missing backticks, `too_many_lines`,
  and more. The CI clippy step runs default features only (`runs[0] = ""`), so
  none of these are enforced. Fix: a dedicated clippy-cleanup pass over the
  server tree, then extend the CI clippy run to `server-test` so the debt
  stays cleared.

## 13. foundation_nativeapis

- **`FjallDocumentStore` scans buffer their rows.** `scan`,
  `scan_all` and `scan_from` collect the matching rows into a
  `Vec` before streaming them (`src/shared/vfs/fjall_document_store.rs`);
  `scan_all` is unbounded. Fix: walk the fjall index range lazily.

- **No delta store on the local Turso engine.** `TursoDelta` never compiled and was
  deleted in spec 50 F05 T2 (with its example, the `vfs-turso` feature and the
  shim's `turso` backend). A Turso cloud database still works through
  `LibsqlDelta` with `TursoRemoteConfig`. Fix: rebuild it on the `turso` crate as
  an async-only store; tracked in ewe-studios/ewe_platform#142.
- **`InodeFs` / `InodeDelta` ignore their path.** `src/shared/vfs/fjall_fs/`
  keeps everything in memory; `InodeFs::open(path)` and `open_with_config`
  ignore the path and the config, and the `FjallFs` / `FjallDelta` aliases
  suggest a disk store. They implement only the sync traits. Fix: rename the
  constructors (`InodeFs::new()`), drop the aliases, and add the async face
  like `MemoryFs`.
- **`FjallDocumentStore` uses the sync VFS traits.** `FjallDocumentStore<V:
  VfsFileSystem>` (`src/shared/vfs/fjall_document_store.rs`) calls its VFS
  synchronously inside the async `DocumentStore` methods. With an I/O-backed
  VFS (wrapped in `SyncFs`) each call blocks. Fix: make `V:
  AsyncVfsFileSystem` and await the calls.
- **NFS `readdir` ignores `start_after`.** `VfsNfs::readdir`
  (`src/native/vfs/nfs/mod.rs`) always starts at the first entry, so a
  directory larger than one page repeats; the attribute times are always 0.
  Fix: skip entries up to `start_after`, and map `VfsMetadata` times.
- **Whole-file reads in the VFS defaults.** `read_file`, `copy` and
  `copy_async` on the VFS traits read the whole file into one `Vec`
  **(rule 4)**. Fix: copy in chunks of a configured size.
- **`SyncFs` is not gated as a blocking boundary.** `exec_async`
  (`src/shared/vfs/exec_async.rs`) uses `collect_one` when a future is
  pending, without `blocking_boundary!`, so on JS-hosted wasm it would spin.
  Fix: gate `SyncFs` and `exec_async` with `blocking_boundary!`.

## 16. Workspace / process

- **Shared target dir clobbering.** When parallel worktrees share one `CARGO_TARGET_DIR`, crates such as foundation_core and foundation_macros build with the same hashes and overwrite each other's test binaries and proc-macro artifacts mid-run. That gives stale results and `E0460` doctest failures.
  - Give each worktree its own target dir, or `RUSTFLAGS=-Cmetadata=<id>`. Note that the latter rebuilds all dependencies.
- **`cargo fmt --check` diverges between local and CI.** The Linux CI image's
  `rustfmt` reformats whole crates differently (64 files in foundation_auth,
  91 in foundation_netio). CI treats fmt as report-only, so scripts still exit
  0, but the repo is not fmt-clean under the CI toolchain. Fix: pin one rustfmt
  version (a `rust-toolchain.toml` with the rustfmt component) and format the
  repo with it.
- **`crates/ewe_deployables` doesn't build.** It isn't a workspace member, so
  nothing compiles it, and it still implements an older `Deployable` shape
  (sync `TaskIterator`s, sync `store(&client).get_typed(..)`), which no longer
  exists. Fix: port it to the current async `Deployable` / `StateStore` and
  add it to the workspace (or delete it if superseded by the per-provider
  deployment crates).
  Its HTTP client references were moved to `HttpClientBuilder` /
  `ProviderClient::with_client` for #140, but the rest is still stale: it
  imports `foundation_netio::simple_http`, `ProviderClient<S, R>` and
  `foundation_deployment::providers::cloudflare` (now
  `foundation_deployment_cloudflare`). `CloudflareWorker::deploy` also reads
  the script from disk into `_script` and never uploads it, so a deploy would
  push an empty module.
- **Cloudflare crate size.** Split the generated Cloudflare groups into per-group crates (task #28).
- **Wasm runtime tests never run in CI** (spec 68 T4). CI's wasm step is a
  compile gate only (`scripts/ci/ci.py --step wasm` → `cargo check --target
  wasm32-unknown-unknown`). The runtime suites (`foundation_core/integration/wasm_tests`
  via `wasm-testbed deno`/`web`, the bindgen-deno path, `cf-cloud-provider` under
  miniflare) are local mise tasks, and `integration/wasm_tests` is a standalone
  crate outside the root workspace, so not even its compile is gated. That is how
  the feature 06 T5 isolate freeze shipped behind green CI, and how the
  `wasm-embedded-js` testbed stopped building (deno_v8 had no JS engine selected)
  unnoticed. Add a CI job that runs `mise run test:wasm-testbed:deno` (embedded
  Deno; no browser needed) and `mise run test:wasm-bindgen:deno`.
- **`wasm-testbed` warns on `Command::Bindgen(args)`** (`foundation_testbed/src/bin/wasm-testbed.rs:37`):
  `args` is unused unless the `wasm-bindgen-test` feature is on (e.g. under
  `--no-default-features --features wasm-embedded-js`). Gate the binding with the
  same `cfg` as its use, or prefix it with `_` in the other arm.
- **Running a wasm mise task installs every tool in `mise.toml`.** `mise exec`/`mise run`
  first installs all pinned tools (bacon, cargo-nextest, cargo-audit, python…),
  building several from source. On a small disk that fills it, and where the
  GitHub attestation API is blocked the python install fails, so
  `test:wasm-bindgen:deno` cannot start even though it only needs deno and
  wasm-bindgen. Split the wasm tools into their own mise env/profile, or have the
  task invoke the two binaries via `mise which`.

- **Wasm runtime tests never run in CI.** `foundation_core`'s
  `tests/valtron/wasm_js_yield_integration.rs` (and the other
  `wasm_bindgen_test` suites) need `wasm-testbed` + Deno; the CI wasm step
  only runs `cargo check --target wasm32` on the library, so these tests are
  not even compiled there. Fix: a CI step that at least builds them
  (`cargo check --tests --target wasm32-unknown-unknown`) and, where a
  runner is available, runs them.
- **`foundation_db` dead-code warnings in test builds**: `core/rate_limit.rs`
  `CREATE_TABLE_SQL`, `RECORD_SQL`, `SELECT_SQL`, `sql_check`, `sql_record`,
  `sql_reset` are unused in the feature set the connectrpc test builds pull
  in. Fix: gate them on the features that use them.
- **`foundation_reactor` publish fails tarball verification: `foundation_core` is stale on crates.io.**
  `foundation_reactor` 0.0.1 (a new crate) depends on `foundation_core` 0.0.6, but its source uses
  `foundation_core::valtron::request_executor_resume` (`reactor.rs:840`) and
  `EventReadiness::clear` (`fd_readiness.rs:143`) that were added to the local tree after 0.0.6 was
  published. `release-plz release` verifies the tarball against the published 0.0.6, so it fails with
  E0425/E0407. Fix: bump `foundation_core` (and its `version_group` partner `foundation_macros`) to
  0.0.7 and update the dependents, so 0.0.7 publishes before `foundation_reactor`.
- **`release-plz release-pr` fails: the workspace root set the nightly-only `codegen-backend`.**
  `release-plz` runs `cargo package --list` (on the stable release toolchain from
  `ci/release-rust-toolchain`) in a temp copy of the repo. To package `foundation_ui_traits` it must
  resolve `edition.workspace = true`, so it parses the workspace root `Cargo.toml`; the root's
  `cargo-features = ["codegen-backend"]` is nightly-only, so stable Cargo refuses to load it
  (`feature codegen-backend is required`) and the bump never computes. Fixed by df86d7e6d
  ("CI(release): build releases on stable"), which dropped the codegen-backend feature and switched
  the release pipeline to the stable toolchain.

## 17. foundation_vectors

- **`FjallVectorStore` runs fjall inline in its async methods**
  (`src/fjall_store.rs`). fjall has only a sync API. The calls read and write
  the page cache without `fsync`, but the first use of a namespace scans all
  of its vectors to rebuild the index, and `flush` (`fsync`) is sync. Both can
  hold a worker for a long time. Fix: run them on a blocking-work seam when
  valtron has one (or in a task of their own), and make `flush` async.
- **`SqliteVecVectorStore` ships only a linux-x86_64 `vec0.so`**
  (`vendor/vec0.so`). `open` fails on every other platform. Fix: vendor the
  extension for each supported target, or build sqlite-vec from source.
- **`SqliteVecVectorStore` tables from before spec 50 F05 T1 do not work.**
  T1 changed the `vec0` schema (text primary key, `metadata` column, distance
  metric). An old `vec0_{ns}` table gives SQL errors. Fix: detect the old
  schema on `open` and migrate it, or fail with a clear message.
- **`TurboPufferVectorStore` is tested only against a mock.** The mock
  follows the request and response types of the official Python client
  (`turbopuffer` 2.14.0), because the documentation site was not reachable.
  Two details are not proven against the live service: that `deleted_ids`
  lists only ids that existed (`delete` reports `NotFound` from it), and
  that `include_attributes: ["vector"]` returns the vector. Fix: an opt-in
  test against a real namespace (API key from the environment).
- **`TurboPufferVectorStore` does not retry HTTP 429.** `TurboPuffer` throttles
  writes with 429 (backpressure). The store reports it as an error. Fix: retry
  with backoff, with the attempts and delays on a config.
- **The `sql-store` feature does not build for wasm32.**
  `cargo check --target wasm32-unknown-unknown -p foundation_vectors
  --features sql-store` fails in `foundation_core` (`idle_wake` missing, a
  `NoopMutex` `PoisonError` mismatch). The crate takes `foundation_db` with
  its default features on every target, so `foundation_core/multi` is on for
  wasm. This was so before spec 50 F05 T1. CI builds `sql-store` only for
  native, so it does not see it. Fix: take `foundation_db` without default
  features on wasm32, then add `sql-store` to the crate's `wasm-runs` in
  `ci/packages.toml`.
- **`hybrid_search` gives the reranker no texts** (`src/fusion.rs`). It calls
  `Reranker::rerank(query, fused, &[])`, so a reranker cannot read the
  candidate documents. Fix: take a text lookup (id → text) as an argument, or
  change `Reranker` to fetch the texts it needs.
- **The SQL-backed stores scan a whole namespace per search**
  (`SqlVectorStore::search`): memory is bounded (`TopK`), but time is
  O(n·d) per query. Expected for the portable fallback; large namespaces
  should use `LibsqlVectorStore` (native index) or a dedicated vector backend.

## 18. ewe_channels (crates/channels)

- **`ArcWake::wake_by_ref` prints a debug line** (`src/executor.rs`):
  `println!("Sent wake action")` fires on every task wake. Fix: delete the
  `println!` (use `tracing::debug!` if a trace is wanted).
- **`DEFAULT_TASK_PENDING_CAPACITY` is a hard-coded limit** (`src/executor.rs`,
  `const DEFAULT_TASK_PENDING_CAPACITY: usize = 10`). Limits are configuration,
  not constants (rule 8). Fix: add a capacity field to the executor config with
  a documented default.
- **`ReceiveChannel::is_closed` reports `Ok(false)` while its receive side is
  open, even after every sender has dropped** (`src/mspc.rs`). It checks only
  whether this receive side was closed, not the underlying channel. Fix: reflect
  `async_channel::Receiver::is_closed`, or rename the method to match its meaning.

## 19. ewe_domain (crates/domain)

- **`DEFAULT_SUBSCRIBER_START_CAPACITY` is a hard-coded limit**
  (`src/servicer.rs`, `const DEFAULT_SUBSCRIBER_START_CAPACITY: usize = 10`).
  Limits are configuration, not constants (rule 8). Fix: move it to a config
  field with a documented default.

## 20. foundation_deployment_huggingface

- **Uploads are buffered whole.** `repo_create_commit` reads each added file
  into memory (`std::fs::read`) and builds the whole multipart body before
  sending it. Fix: a streamed multipart body (`SendSafeBody::Stream` over the
  file chunks) with the length computed up front.
- **`list_*` and `repo_list_tree` read one page only.** They ignore the Hub's
  `Link: rel="next"` pagination header, so a listing stops at the first page
  (the docs say "with pagination"). Fix: follow `Link` lazily as the stream
  is pulled.

## 21. Wasm-only warnings (seen in `cargo check --target wasm32-unknown-unknown -p foundation_keychain`)

- **`foundation_keychain`**: `rmpv` is an unused dependency on wasm32
  (`Cargo.toml`; only native code uses it), and `src/server/signalr_do.rs:29`
  imports `notifications::self` without using it, and binds `mut resp` at
  line 74 that is never mutated. Fix: move `rmpv` to the native target
  section, drop the `self` import and the `mut`.
- **`foundation_deployment_cloudflare`**: `src/workers/durable_object.rs:24`
  imports `worker::Error` without using it. Fix: drop it.

## 22. foundation_keychain

- **`poll_ready` single-polls a handler future with a noop waker.**
  `src/server/native.rs` `poll_ready` (called from `Serve::serve`) drives the
  async `provisioning_route` and `route` handlers by one `poll` with a noop
  waker, and panics on `Poll::Pending`. It works only while the handlers finish
  without yielding. Any real async I/O (a Turso query that is not ready, a
  channel wait) panics at runtime. Fix: return `ConnectionResult::Async`
  wrapping `async move { … route(..).await … }`, and delete `poll_ready` and
  `noop_waker`, so the executor polls with a real waker.

## 23. examples/todo

- **`src/lib.rs` holds only a dead `fn main`.** A library has no `main`
  entry point, so the function is never called and the crate publishes
  nothing. The example is a stub. Fix: delete `fn main` and add a real
  public item, or remove the example.
- **Ten dependencies are unused** (`Cargo.toml`): `anyhow`,
  `console_error_panic_hook`, `serde`, `serde_json`, `serde_with`,
  `thiserror`, `tracing`, `tracing-subscriber`, `wasm-bindgen-futures` and
  `web-sys`. The stub `src/lib.rs` uses none of them. Fix: add code that
  uses them, or remove them.

## 24. bin/platform templates (web-app)

- **`web-app` scaffolding is dead code and produces warnings in the generated
  project.** `src/app.rs` (`pub struct App {}`), `src/config.rs` (`DevServer`,
  `AppServer`, `ServerConfig`) and the private re-exports in `src/lib.rs`
  (`use app::*; use config::*;`) are never used by the app or dev binaries.
  Fix: deserialize the config into `ServerConfig` in the dev binary and wire
  `App` into the app binary, or delete the unused scaffolding.

## Paused

- Windows/macOS VM images (#69).

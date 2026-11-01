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

- **`thiserror` is unused** (Cargo.toml line 42). Nothing in the crate uses it
  (netio uses `derive_more` for errors). The Linux CI build flags it as an
  unused dependency. Fix: remove it. (`rustls-pemfile` is also flagged unused,
  but that is a false positive: it is optional and used by the `quic` and
  `ssl-rustls` features in `quic/quinn_driver.rs`.)

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

- **`docker_restart_container_defaults` fails on macOS under the `uat` profile.**
  The mock Docker daemon binds a Unix socket under `target/uat`'s deep directory;
  the path is longer than `SUN_LEN` (104 bytes), so the bind fails. This is test
  infrastructure, not feature 12's code. Fix: bind the mock socket in a short
  path, such as a per-test temp directory.
- **`foundation_deployment_cloudflare` has an unused `urlencoding` dependency.** It appears only in `Cargo.toml`, never in source or tests. Fix: remove the line.

## 7. foundation_deployment_platform

- **Inline tests remain** in `docker/wireguard.rs` (rule 7).
- **Warnings:**
  - deprecated `SimpleHttpClient`, used in `wait_for.rs` and in `foundation_deployment/src/provider_client.rs`;
  - unused import `PortMapping` in `docker/wireguard.rs:21`.

## 8. foundation_openapi / foundation_codegentools

- **D1 `read_replication` update bodies are untyped**
  (`foundation_deployment_cloudflare/src/generated/workers/mod.rs`).
  `D1DatabaseUpdateRequestBody.read_replication` and
  `D1DatabaseUpdatePartialRequestBody.read_replication` are
  `HashMap<String, serde_json::Value>`, while the create and details types use
  `D1ReadReplicationDetails`. Code that turns read replication on (`mode: auto`)
  would have to write untyped JSON (rule 5). Fix the generator so the update
  bodies use the same typed struct, then regenerate.
- **Boolean query parameters are generated as `Option<String>`** (for example `ContainerStatsArgs.stream` / `one_shot`), probably for every endpoint.
  - Generate `Option<bool>` and serialise it.
  - Related: Docker `image_build` can only send one tag, because `t` isn't treated as an array.
- **Inline tests remain** in `unified/emit.rs` and `unified/analyzer.rs` (rule 7).
- **genapi's flags are ignored in two places.**
  - Its post-step `cargo fix` always runs in the dev profile, ignoring `--profile`.
  - A plain `generate hetzner` ignores the crate's declared operation selection and generates the full API.

## 9. foundation_proxy

- **`forward_http` relays on a dedicated thread and returns `Take`.** Feature 08
  made the h1 `Serve` handler able to return `ConnectionResult::Async`, so a
  long-lived handler parks instead of holding a worker. The proxy still relays on
  a throwaway thread and returns `Take`. It can move to `Async` later once its
  relay future is `'static`.
- **gRPC health probe leaks a thread + H2 connection when its inner timeout hangs.**
  - `health.rs::probe_grpc_health` drives `grpc_health_check` on a throwaway thread and bounds it with `recv_timeout` as a backstop, because the inner `select` + valtron `sleep_async` timer could fail to fire (fixed on `fix/foundational-backlog`: `sleep_async` now runs on valtron's timer thread) and `H2Transport`'s pump parks on the socket read with no read timeout.
  - When the backstop trips, the inner `block_on_future` (and its H2 pump task) is abandoned — a per-occurrence thread/connection leak. Rare (the timer only fails under pool contention, e.g. proxy shutdown), but a flapping or non-responsive backend could repeat it every probe interval.
  - Likely fix: give `H2Transport` a real read/keepalive timeout so the pump self-terminates, then drop the backstop thread (the timer half of the cause is gone).
- **Backend-state persistence can lose an update.** Each control-socket
  connection runs on its own thread, and `ProxyStateStore::save_backend_state`
  / `save_tls_cert` (`src/persistence.rs`) load the whole persisted state,
  change one entry and save it back. Two admin commands at once can each save
  over the other's change. Fix: serialise the read-modify-write behind a
  mutex in `ProxyStateStore` (or store each backend's state as its own record).
- **The ACME client does not follow RFC 8555's account flow** (`src/acme.rs`).
  `create_account` returns the newAccount directory URL instead of the
  account URL from the `Location` header (it only checks that the answer has
  an `orders` field), and `provision` discards it, so `key_id` is never set
  and every later request is signed with the `jwk` instead of the `kid`
  (§6.2 requires the `kid` once the account exists). The nonce is fetched
  fresh for every request instead of taken from each answer's
  `Replay-Nonce`. `provision` waits for DNS with `std::thread::sleep(5s)`
  inside an `async fn` (it blocks the executor; the 5 s is a constant, not a
  setting). The mock CA in `tests/acme_provisioning_tests.rs` sends no
  `Location`, so the tests miss all of this. Fix: return and use the account
  URL as the `kid`, carry the nonce from each answer, wait with an async
  sleep (or poll the challenge status) under a configured timeout, and make
  the mock CA strict.
- **The SSL redirect listener hard-codes its limits** (`src/server.rs`,
  `ssl_redirect_loop`): a 4096-byte head buffer, a 2 s read timeout and a
  10 ms accept poll are constants, and a head longer than one read is cut
  (rule 8). Fix: fields on the redirect config with documented defaults.
- **`tests/diag_client_tap.rs` is a "TEMPORARY diagnostic", not a test.**
  It is `#[ignore]`d, only `eprintln!`s what the client sent and received, and
  asserts nothing, so it can never fail. Fix: turn it into a real test of
  `forward_http`'s outgoing request (assert the `Host`, `X-Forwarded-*` and
  `Connection` headers the fake upstream receives and the 200 status) and run
  it un-ignored, or delete it if `proxy_integration_tests.rs` already covers it.
- **The `mut` on `h3_local_addr` warns without the `h3` feature**
  (`src/server.rs`, `ProxyServer::start`). Fix: build it from a `cfg`-gated
  expression instead of assigning inside the `cfg` block.

## 10. foundation_ai

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

- **`WebAuthnService.config` is never read.** `src/server/services/webauthn_service.rs`
  stores `config: Arc<IdpConfig>` but only uses it in `new()` to build the
  `Webauthn` (issuer URL / origin). The field is dead in both the
  `server-native` and the non-`server-native` variants. Fix: drop the field and
  use the `config` argument locally in `new()`.
- **The five `shared` auth clients cap the error body at a hard-coded constant.**
  `discovery`, `jwks`, `introspection`, `userinfo` and `upstream_client`
  (`src/shared/`) call `read_error_text(.., DEFAULT_MAX_ERROR_BODY)`. Only the
  native OAuth client's cap is configurable (`OAuthConfig::max_error_body`);
  these five have no config field, so their 64 KiB cap cannot be changed. Fix:
  add a `max_error_body` field to each client's config (or builder) to match
  `foundation_deployment`'s `BodyLimits::max_error_body`.

## 13. foundation_nativeapis

- **The blob-backed D1/R2 deltas can't list.** `BlobDelta`
  (`src/native/vfs/blob_delta.rs`, behind `D1Delta`/`R2Delta`) keeps each
  file, its metadata and its whiteout as separate blobs, and a blob store has
  no key listing: `list_async` and `list_whiteouts_async` return nothing and
  `reset_async` is unsupported. Fix: keep a per-directory index blob (or use
  D1's key-value listing, `list_keys`, for the D1 delta).
- **A blob-delta write rewrites the whole file.** Each `write_at_async` reads
  the blob, splices the bytes in and writes it back, so a large file written
  in small pieces is O(n²). Fix: chunked storage (the unused
  `d1_delta::chunking` module sketches it) with a configurable chunk size.
- **The VFS shim never uses the D1/R2 deltas.** `src/native/vfs/shim/mod.rs`
  logs "impls pending" and falls back to the memory delta for
  `DeltaBackend::D1` / `R2`. The overlay is async now, so the deltas plug in
  directly (`DynFs::from_async(OverlayFileSystem::new(base, d1_delta))`).
  Fix: build them from the shim config.
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

## 15. foundation_cedar

- **Pre-existing clippy `pedantic` warnings (24).** `cargo clippy --no-deps`
  still reports 24 warnings in this crate. The library has 18: nine
  `missing_errors_doc` (`engine.rs`, `entity_provider.rs`, `policy_set.rs`,
  `request.rs`), four `must_use` (`engine.rs`, `request.rs`), two
  `redundant_closure_for_method_calls` (`engine.rs`, `response.rs`), two
  `manual Debug does not include all fields` (`file_store.rs`, `traits.rs`)
  and one `struct_field_names` (`file_store.rs`). The test targets have six
  more: three `unnecessary hashes around raw string literal` and three
  `assert!`-on-non-empty (`tests/engine.rs`, `tests/file_store.rs`,
  `tests/resolver.rs`, `tests/traits.rs`). None are on feature-21 code.
  Fix: add the missing doc sections, drop the redundant `#[must_use]`, complete
  the Debug impls, and clean the test helpers, then delete this entry.
- **Future-incompat `recursion_depth_exceeding_limit` in the `sql_store` test
  (`--features db`).** The `turso::Builder::build()` async fn drives deep `Send`
  auto-trait resolution, so the compiler warns it will become a hard error.
  Fix: add `#![recursion_limit = "256"]` to the `sql_store` test target, or a
  manual `Send` impl for the intermediate types.

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

- **The Hub client API is sync.** `client::{whoami, list_*, create_repo, …}`
  and `repository::*` (`src/client.rs`, `src/repository.rs`) block on the
  native client's sync `HttpClient::send`, so they freeze a valtron worker
  and cannot run on wasm. Fix: async functions over `send_async`
  (`collect_bytes_async` for the downloads), with `.block()` at the callers
  (`foundation_ai`'s candle/gguf providers, `foundation_testing`).
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

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

- Clippy warnings (pre-existing, `cargo clippy -p foundation_connectrpc --profile uat --all-targets --all-features`), grouped by lint:
  - `clippy::doc_markdown` ("item in documentation is missing backticks") across `src/` (lib.rs, error/code.rs, auth.rs, …), `tests/`, and `examples/` — the largest set.
  - `clippy::cast_possible_truncation` / `cast_possible_wrap` / `cast_sign_loss` in `tests/` (h1_pool_parking_tests.rs, router_dispatch_tests.rs, arrow_codec_tests.rs, codec_tests.rs, conformance_tests.rs, grpc_tests.rs, server_socket_tests.rs, …) and `src/shared/transport/conn.rs`, `src/shared/client.rs`.
  - `clippy::needless_pass_by_value`, `clippy::must_use`, `clippy::struct_excessive_bools`, `clippy::similar_names`, `clippy::unused_async` (examples: code_first_service).
  - Fix in a dedicated warning-cleanup pass for the crate.

## 2. foundation_netio

- **Clippy `semicolon_if_nothing_returned` in `native/connection.rs`.** File:
  `backends/foundation_netio/src/native/connection.rs:1440`. Problem: the last statement of a
  block has no `;`, and clippy warns (`cargo clippy -p foundation_netio --no-deps --all-targets`).
  Likely fix: apply the suggestion (add the `;`). Found in feature 69 (spec 70); not changed there.
- **Clippy `needless_pass_by_value` in `expect_continue_timeout_tests.rs`.** File:
  `backends/foundation_netio/tests/simple_http/expect_continue_timeout_tests.rs:89`. Problem: `check`
  takes `status: Status` by value and does not use it by move. Likely fix: take `&Status`. Found in
  feature 69; not changed there (feature 43 file).

## 3. foundation_http

- `backends/foundation_http/src/shared/serve/mod.rs` (`respond::*`, `render_response`): the
  synchronous helpers write with `write_all` to any `impl Write`. On a non-blocking socket a body
  larger than the send buffer, or a slow peer, fails with `WouldBlock` after a partial write and
  the response is truncated. In `foundation_http` the handlers (`StaticFileHandler`,
  `StaticAssetHandler`, `HealthHandler`) now use `native::writer::send_*`. Still on the
  synchronous path: callers in other crates (`foundation_proxy/src/handler.rs`, the web-app and
  static-html-app templates, `examples/web/wello`), and the small closing or rejection writes in
  `native/server/{connection,protocol_detect,mod}.rs` and the upgrade heads in
  `native/upgrade/mod.rs`, which write a few hundred bytes into an empty send buffer. Likely fix:
  move each handler caller to `native::writer::send_*` (or `ResponseWriter`), then deprecate the
  synchronous `respond::*` for sockets.
- `backends/foundation_http/tests/h2_parking_tests.rs` (`idle_h2_connection_uses_no_cpu`): fails
  about half the runs on the dev VM, on master as well (54 ms of process CPU in 1 s against a
  50 ms cap). The cap measures the whole process, so any other thread's work counts. Likely
  fix: measure only the connection task's CPU (or the reactor and worker threads by thread id),
  or find what still wakes the process while idle; do not raise the cap blindly.

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

(no open entries)

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
- **The Cloudflare per-group generator emits `lib.rs` shims that are not
  rustfmt-clean.** The 43 `backends/foundation_cloudflare_*/src/lib.rs` shims
  have one long `#![allow(...)]` attribute and `pub use` items that rustfmt
  would wrap and reorder. The build script in
  `foundation_deployment_cloudflare` byte-compares the committed files against
  a fresh generation and regenerates on any difference, so the shims cannot be
  reformatted by hand. Feature 39 excludes them with the `ignore` list in
  `rustfmt.toml`. Fix: make the generator emit rustfmt-clean shims (wrap the
  `allow` attribute, sort the `use` items), regenerate, and drop the `ignore`
  entries.

## 9. foundation_proxy

- **`foundation_proxy` clippy and test warnings are open.** `cargo clippy -p
  foundation_proxy --all-targets` reports about 50 warnings (for example
  `acme.rs`, `h2_proxy.rs`, `server.rs`, `control.rs`; doc backticks, `must_use`,
  redundant closures, `format!` appended to a `String` at `forward.rs`). The
  tests `load_balancer_tests.rs` (unused `Arc`, `Duration`, `STICKY_COOKIE`) and
  `h2_integration_tests.rs:318` (unused `mut`) warn on build. Scheduled as
  feature 49 E7.
- **`passthrough::splice_bidirectional` is a polling splice that only a test
  calls.** It sleeps 1 ms when idle. `completion_splice_tests.rs` uses it on two
  io_uring completion sockets. Fix: wrap the completion sockets as `RawStream`,
  run the test through `aio::splice`, and delete the function.
- **The io_uring upstream dial is blocking.** `aio::dial_tcp` uses
  `foundation_iogate::connect_completion` for the non-`Std` modes, whose `connect`
  blocks the worker for one round trip. Fix: an async connect in iogate.
- **The H3 proxy forward collects the whole response body.** `h3_proxy.rs`
  `forward` reads the upstream body into one `Vec` before it answers, which breaks
  rule 4 (stream bodies). Its waits are now timer parks, not thread sleeps. Fix:
  stream the body into H3 DATA frames.
- **A response writer's send timeout counts from its creation, not from its last
  byte, for long streams.** `forward_http` and the translators use one
  `ResponseWriter` for the whole response, so a gRPC stream idle for longer than
  `relay_send_timeout` times out at its next blocked write. The tunnel splice
  avoids this with a writer per chunk. Fix: feature 63 E (restart the idle timer
  when a write starts).

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
- **`FileStateStore::set` writes in place, so a reader can see a torn file.**
  `core/state/file.rs` calls `std::fs::write`, which truncates and then writes. A
  `get` from another store object or process in that gap reads empty or partial JSON
  and fails to decode. `foundation_proxy` now locks its read-modify-write (feature 45),
  but its plain `load` is not locked. Fix: write to a temp file in the same directory
  and `rename` it over the target. The trait also has no conditional write (version or
  etag), which is why the proxy uses a lock file.
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

- **Turso store rejects overlapping calls: "Backend error: concurrent use forbidden".**
  `turso_sdk_kit` allows one use of a connection at a time, and the Turso
  `StorageProvider` shares one connection. Now that the keychain server runs
  handlers as `ConnectionResult::Async` (features 44 and 64), two requests that
  reach the store at the same moment make one fail with HTTP 500. Seen in
  `foundation_keychain/tests/streamed_body.rs` when two slow uploads finish
  together. Fix: a connection per call or a small pool, or serialise calls in
  the Turso backend (feature 51 owns the shared-connection work).

## 12. foundation_auth

- **The `server`/`server-native` feature tree has over 200 clippy warnings.**
  `cargo clippy -p foundation_auth --features server-native` reports 212 lints
  across `src/server/` (`config.rs`, `handlers/*`, `services/*`): redundant
  closures, missing `#[must_use]`, docs missing backticks, `too_many_lines`,
  and more. The CI clippy step runs default features only (`runs[0] = ""`), so
  none of these are enforced. Fix: a dedicated clippy-cleanup pass over the
  server tree, then extend the CI clippy run to `server-test` so the debt
  stays cleared.

## 16. Workspace / process

- **`foundation_testbed` declares `deno_v8` as an unused dependency** (cargo
  nightly 1.100.0 `unused_dependencies` lint, `Cargo.toml:118`). `deno_v8` is
  listed only to enable its V8 backend for `deno_core` (feature unification);
  no source `use`s it. Fix: silence the lint for that dependency (for example
  a `[lints.cargo]` override or a `use` that marks it used).

- **`foundation_reactor` wasm dead-code warning: `rearm_readable` is never used.**
  `Poll::rearm_readable` (`src/poll/mod.rs:322`) is `pub(crate)` and only called
  from `src/reactor.rs:565`, which is a native (kqueue/poll) path. On wasm32 the
  call site is compiled out, so `cargo check --tests --target wasm32` on
  foundation_wireguard/foundation_netio emits `method rearm_readable is never
  used`. Fix: gate the method on the same `cfg` as its call site.
- **`foundation_db` dead-code warnings in test builds**: `core/rate_limit.rs`
  `CREATE_TABLE_SQL`, `RECORD_SQL`, `SELECT_SQL`, `sql_check`, `sql_record`,
  `sql_reset` are unused in the feature set the connectrpc test builds pull
  in. Fix: gate them on the features that use them.
- **`foundation_reactor` publish fails tarball verification: `foundation_core` is stale on crates.io.**
  `foundation_reactor` 0.0.1 (a new crate) depends on `foundation_core` 0.0.6, but its source uses
  `foundation_core::valtron::request_executor_resume` (`reactor.rs:840`) and
  `EventReadiness::clear` (`fd_readiness.rs:143`) that were added to the local tree after 0.0.6 was
  published. `release-plz release` verifies the tarball against the published 0.0.6, so it fails with
  E0425/E0407. The 0.0.7 bump has landed (`foundation_core` and `foundation_macros` are now
  0.0.7 on `fix/io-35-turso-async-trait-overflow`); the crates.io publish still needs to
  happen at release time so 0.0.7 publishes before `foundation_reactor`.
## 17. foundation_vectors

- **The SQL store test tree hits `recursion_depth_exceeding_limit`.**
  `tests/contract/mod.rs` plus `make_store` over `TursoStorage` forces a
  `Send` check through the `turso` crate's deeply nested async chain, so
  `sql_vector_store_tests`, `libsql_vector_store_tests` and
  `sqlite_vec_store_tests` each emit the future-incompatible warning. It
  becomes a hard error in a later Rust release. Fix: add
  `#![recursion_limit = "256"]` to those test crates, the same as
  `foundation_db/tests/turso_storage_tests.rs`.
- **Pre-existing clippy deny in the `code-graph` test tree.**
  `tests/lang_config_tests.rs:123` uses `assert!(ext.raw_calls.len() >= 0)`
  (`clippy::absurd_extreme_comparisons`, a deny-by-default lint), so
  `cargo clippy -p foundation_vectors --profile uat --all-targets
  --all-features` fails. CI does not run `code-graph`, so it never sees it.
  Fix: replace the comparison with `assert!(!ext.raw_calls.is_empty())` or
  delete the assertion, then keep the whole `code-graph` tree clippy-clean.

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

## 21. Wasm-only warnings (seen in `cargo check --target wasm32-unknown-unknown -p foundation_keychain`)

- **`foundation_deployment_cloudflare` has three `unused_dependencies` manifest
  lints under wasm32** (cargo nightly 1.100.0): `foundation_errstacks` and two
  `foundation_netio` entries. `foundation_errstacks` (types.rs) and
  `foundation_netio` (client.rs, dns_ops.rs) are used only in
  `#[cfg(not(target_family = "wasm"))]` modules, so the wasm build flags them.
  The two truly-unused deps (`base85rs`, `sha2`) were removed when the generated
  Cloudflare groups split into per-group crates (feature 22 E3). Fix: move the
  native-only deps into the native target section and drop the wasm-only
  `foundation_netio` entry (nothing uses it on wasm).

## 22. foundation_keychain

- **`foundation_keychain` clippy debt.** `cargo clippy -p foundation_keychain
  --profile dev --no-deps` reports 174 warnings in the lib (for example
  `manual_let_else` in `provisioning_route`) and more in the tests. Fix: follow
  the per-crate clippy workflow in `.agents/AGENTS.md`.

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

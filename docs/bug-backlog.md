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

- **`forward_http` relays on a dedicated thread and returns `Take`.** Feature 08
  made the h1 `Serve` handler able to return `ConnectionResult::Async`, so a
  long-lived handler parks instead of holding a worker. The proxy still relays on
  a throwaway thread and returns `Take`. It can move to `Async` later once its
  relay future is `'static`.
- **The `cloudflare` feature pulls in `foundation_deployment_cloudflare` but no
  code uses it.** The dependency is named only in doc comments (`config.rs`,
  `tls.rs`, `acme.rs`), so `cargo check --features cloudflare` reports an
  `unused_dependencies` lint. Fix: use the dependency (for example, drive the
  ACME DNS-01 Cloudflare path through it) or drop the feature.

## 10. foundation_ai

- **Published `foundation_ai` panics in gemm on a CPU with `fma` but no `avx2`.**
  `gemm-common` 0.19.0 chooses its `fma` kernel from the `fma` CPU feature alone,
  but that kernel needs AVX2. On emulated CPUs (Docker Desktop `linux/amd64`)
  `V3::try_new()` returns `None` and the unwrap panics. We fix it with
  `patches/gemm-common` and `[patch.crates-io]`, which applies only inside this
  workspace, so a user of the published `foundation_ai` still gets the panic. We
  cannot rename `gemm-common` (candle depends on it). Fix: send the two-line patch
  in `patches/gemm-common/UPSTREAM.md` to https://github.com/sarah-ek/gemm/, then
  drop the patch when a fixed release is out and candle accepts it. (The wasm32
  `boringtun` patch no longer exists: `backends/foundation_boringtun` is a
  published fork.)

## 11. foundation_db and its sync callers

Follow-ups from making the I/O backends async-only (branch `db/async-stores`).
- **Auth tables mix timestamp units.** The `created_at` defaults in the core
  migrations are milliseconds (`strftime('%s','now') * 1000`), while
  `AsyncStorageCleanup` compares `expires_at` with Unix seconds and the IdP
  server writes milliseconds for its own rows. Fix: pick one unit per column,
  document it in the migrations, and make cleanup follow it.
- **D1 request parameters are sent as JSON numbers and null; the hosted D1
  REST endpoint is not proven to accept them.** Cloudflare's `OpenAPI` spec types
  `d1_params` as an array of strings. `foundation_db` binds `DataValue::Integer`,
  `Real` and `Null` as JSON numbers and `null`, and the overlay
  `artefacts/cloud_providers/cloudflare/overlays/d1-query.json` widens the
  generated type to match. The local wrangler worker (`worker-stub.js`) accepts
  both, but that is the D1 *binding*, not Cloudflare's hosted REST API (it also
  binds a whole JS number as a `REAL`). Fix: an opt-in test against a real
  Cloudflare account that sends an integer, a real and null to `/query` and reads
  back `typeof()`; if the hosted API rejects them, drop the overlay and convert
  values to strings in `d1_kvstore.rs`.
- **D1 `Real` parameters that are NaN or infinite silently become `NULL`.**
  `data_value_to_json` in `src/native/d1_kvstore.rs` maps `serde_json::Number::
  from_f64` failure to `Value::Null`, so `execute(sql, [Real(NaN)])` writes NULL
  without an error. Fix: return `StorageError::SqlConversion` for a non-finite
  value (and give `data_value_to_json` a `Result` type).
- **D1 `/query` responses are read whole, with no size cap.**
  `D1Store::post_query` calls `collect_string_async` on the body, so a large
  `SELECT` is buffered in memory in full (CLAUDE.md rule 4). Fix: add a response
  limit field on the store's config (rule 8; nginx's `proxy_buffer` and Go's
  `http.MaxBytesReader` use finite caps), read under it, and return a clear
  error when it is exceeded; stream rows when the D1 API allows.
- **The Cloudflare wire-type check takes about 160 s in a `dev` build.**
  `foundation_codegentools/tests/cloudflare_types.rs::cloudflare_types_are_generated`
  regenerates all 43 group crates into a scratch tree, like the build script
  does. Fix: let `Codegen::check` compare only the shared types crate when asked
  (`check_shared_only`), and keep the full compare in the build script.

## 16. Workspace / process

- **`foundation_boringtun` (fork of upstream boringtun) keeps upstream's inline tests and nine bare `#[ignore]`s.**
  `src/device/integration_tests/mod.rs` has nine `#[ignore]` tests (lines 466, 475,
  543, 590, 625, 661, 700, 750, 801) that need root, a TUN device or the `wg` tool,
  and other source files carry `#[cfg(test)] mod tests`. They are upstream's, left
  unchanged so the fork diff stays small. Fix: move the tests into
  `backends/foundation_boringtun/tests/` (made runnable in CI, with the root-only
  ones gated by a runtime check and not by `#[ignore]`) when we next change that
  source.

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

- **`poll_ready` single-polls a handler future with a noop waker.**
  `src/server/native.rs` `poll_ready` (called from `Serve::serve`) drives the
  async `provisioning_route` and `route` handlers by one `poll` with a noop
  waker, and panics on `Poll::Pending`. It works only while the handlers finish
  without yielding. Any real async I/O (a Turso query that is not ready, a
  channel wait) panics at runtime. Fix: return `ConnectionResult::Async`
  wrapping `async move { … route(..).await … }`, and delete `poll_ready` and
  `noop_waker`, so the executor polls with a real waker.
- **`src/bin/keychain.rs` hits the `recursion_depth_exceeding_limit` lint.**
  The `future_incompatible` lint warns on `DrivenStreamIterator<FutureTask<..>>:
  IntoIterator` at `src/bin/keychain.rs:28`. Fix: raise the crate recursion
  limit, or break the valtron stream chain into a smaller type.

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

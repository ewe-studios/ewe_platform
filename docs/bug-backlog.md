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

- **`foundation_netio` native build warnings** (seen in `cargo check -p foundation_wireguard --tests`):
  `backends/foundation_netio/src/native/raw_stream.rs:4` has `use std::sync::Arc;` that is unused,
  and `raw_stream.rs:414` has `if let Self::AsPlain(inner, _) = self`, an irrefutable pattern
  (the `if let` always matches). Fix: delete the import; replace the `if let` with a `let`
  binding (keep the behaviour the same and check it with the netio tests).

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

- **`foundation_deployment_docker` clippy debt.** About 231 lints (`format!` inline
  variables, trailing commas, `::default()` calls, missing `# Errors` sections).
  `foundation_openapi` and `foundation_codegentools` are clean (spec 70 feature 55).
  Fix: clear the docker lints in feature 49, then keep them at zero.

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
  but that kernel needs the whole V3 SIMD type (avx2 too). On emulated CPUs (Docker
  Desktop `linux/amd64`) `V3::try_new()` returns `None` and the unwrap panics. We
  fix it with `patches/gemm-common` and `[patch.crates-io]`, which applies only
  inside this workspace, so a user of the published `foundation_ai` still gets the
  panic. We cannot rename `gemm-common` (candle depends on it). The upstream fix
  (dispatch on `V3::try_new()` / `V4::try_new()`) is prepared on branch
  `fix-fma-without-avx2` of the sarah-ek/gemm clone; the issue and PR link are
  pending (see `patches/gemm-common/UPSTREAM.md`). Fix: file the issue and PR, then
  drop the patch when a fixed release is out and candle accepts it. (The wasm32
  `boringtun` patch no longer exists: `backends/foundation_boringtun` is a
  published fork.)

- **`gemm-f16` fix is vendored but not yet upstream.** `patches/gemm-f16` (0.19.0,
  `[patch.crates-io]`) applies the same `V3::try_new()` / `V4::try_new()` dispatch
  as `gemm-common`. It applies only inside this workspace, so a user of the
  published `foundation_ai` still gets the unpatched f16 path. The upstream PR is
  prepared on branch `fix-fma-without-avx2` of the sarah-ek/gemm clone; link pending
  (see `patches/gemm-common/UPSTREAM.md`). Fix: file it, then drop
  `patches/gemm-f16` when a fixed release is out and candle accepts it.

## 11. foundation_db and its sync callers

Follow-ups from making the I/O backends async-only (branch `db/async-stores`).
- **`foundation_db` has clippy debt, so `clippy-deny-warnings` is off for it.** With
  `cargo clippy -p foundation_db --all-targets --no-deps -- -D warnings`: 12 errors in default
  features, 25 with `libsql` (`i32`/`usize` cast truncation and sign loss, missing `# Errors`
  sections, others). Fix: clear them, then set `clippy-deny-warnings = true` in
  `ci/packages.toml` (with `clippy-runs = ["", "libsql"]`).
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

## 16. Workspace / process

- **`foundation_wireguard` warnings in its tests and manifest.** `cargo check -p foundation_wireguard --tests`
  reports: unused imports at `tests/relay_tests.rs:325` (`MemberState`) and `tests/tunnel_tests.rs:186`
  (`Duration`, `Instant`); an unused variable `pk` at `tests/tunnel_tests.rs:190`; and unused `must_use`
  return values at `relay_tests.rs:192` (`RelaySelector::failover`) and `relay_tests.rs:295`
  (`HolePuncher::drain_syns`). The manifest (`backends/foundation_wireguard/Cargo.toml`)
  has unused dependencies `foundation_errstacks` (line 20), `foundation_nativeapis` and `foundation_netio`
  (the wasm check), and `web-sys` (line 74) for wasm32. Fix: remove the unused imports and variable, and
  either use or drop the `must_use` results; drop the unused dependencies after checking that no
  `cfg` path needs them. Do not `#[allow]` them.

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
- **`foundation_reactor` publish fails tarball verification: `foundation_core` is stale on crates.io.**
  `foundation_reactor` 0.0.1 (a new crate) depends on `foundation_core` 0.0.6, but its source uses
  `foundation_core::valtron::request_executor_resume` (`reactor.rs:840`) and
  `EventReadiness::clear` (`fd_readiness.rs:143`) that were added to the local tree after 0.0.6 was
  published. `release-plz release` verifies the tarball against the published 0.0.6, so it fails with
  E0425/E0407. The 0.0.7 bump has landed (`foundation_core` and `foundation_macros` are now
  0.0.7 on `fix/io-35-turso-async-trait-overflow`); the crates.io publish still needs to
  happen at release time so 0.0.7 publishes before `foundation_reactor`.
## 17. foundation_vectors

- **Code-graph function and method ids ignore the class, so a second method with the same
  name is dropped.** `extract_function` (`src/code_graph/lang_config.rs`, and the Rust and Go
  walkers) builds the node id from the file stem and the bare name (`make_id(stem, name)`),
  then skips the node when the id is already in `seen_ids`. Two classes in one file that both
  define `handle` give one node, and the calls inside the second body are never extracted
  (found with the TypeScript source in `tests/lang_config_tests.rs`: an abstract `handle` in
  `BaseController` hides `UserController::handle` and its `this.repo.findAll()` call). Fix:
  put the parent (class or impl) in the id, then give `ts_raw_calls_extraction` the two-class
  source back.

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

## 21. foundation_deployment_cloudflare on wasm32

- **`foundation_core` does not build for wasm32 with neither `std` nor `alloc`**
  (`cargo check -p foundation_core --target wasm32-unknown-unknown --no-default-features`):
  `valtron::Ctx` is unresolved in `executors/collect_next.rs` and `dependent_lift.rs`. CI
  never runs that configuration (its wasm runs all add `std`). Fix: either gate the
  executors that need `Ctx` on `alloc`, or declare in `Cargo.toml` docs that `alloc` is the
  minimum and make a bare build fail with a `compile_error!` that says so.

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

## 25. foundation_auth

Found while building the ToS login gate (spec 70 feature 68, 2026-11-03) and its security follow-ups (feature 73). Feature 73 fixed the atomic take, cache expiry, TOTP enrollment/encryption/replay, social open redirect, logout and failed-attempt reset.

1. `src/server/handlers/core.rs` (`register`), `foundation_db` `users` table: a user created without `preferred_username` gets a value that breaks `users.username` UNIQUE, so the second such user gets a 500 (`UNIQUE constraint failed: users.username`). Fix: store NULL (or make the column nullable-unique) when no username is given.
2. `src/server/idp_server.rs`, `services/session_service.rs`: the default `SessionService` is in-memory with a random key, so sessions end when the process stops. An app built with `register_routes` only (no `http_app`) gets one session store per route, so a login session is not valid on other routes. Fix: a persistent `SessionManager` in `IdpServer`, and take the session service from the app in `register_routes`.
3. `src/server/handlers/core.rs` (`dispatch`): routing is by `ends_with`/`contains` in a fixed order (this made `login` unreachable until feature 68; only that case was fixed). Fix: route on method and exact path.
4. `src/server/handlers/core.rs` (`social_callback`): when the ToS gate applies, the 206 JSON answer is returned on a browser `GET` redirect flow, so the browser shows JSON. Fix: redirect to a ToS page with the code, or a front-channel flow for social login.
5. `foundation_db` `src/core/schema/sql`: 28 SQL files but 27 entries in `MIGRATIONS`; `023_create_vectors.sql` is not registered. Register it or delete it (it may be deliberate; check the vectors crate).
6. `foundation_db` `KeyValueStore::take` (`KVWasmStorage`, Workers KV): Workers KV has no conditional delete, so `take` there is the documented non-atomic read-then-delete. A ToS code or MFA challenge on a KV-only cache can be used twice by two simultaneous requests. Fix: keep single-use state in D1 or a Durable Object, not in Workers KV.
7. `foundation_auth` TOTP (`src/shared/two_factor.rs`): codes are HMAC-SHA256. Common authenticator apps (for example Google Authenticator) only do SHA-1 and ignore the `algorithm` parameter of the `otpauth_uri`, so those apps make codes the server rejects. Fix: support SHA-1 (RFC 6238 default) next to SHA-256, chosen at enrollment.
8. `foundation_auth` TOTP enrollment (`handlers/core.rs`): `enroll/confirm` allows one code attempt per `enroll/start`, but a session can call start again without limit, so a stolen session can guess first codes. Fix: count failed confirms per user and lock out, like the password attempts.
9. `foundation_auth` `session_service.rs`/`shared/session.rs`: the session cookies are not `Secure` (the cookie builder sets `HttpOnly` and `SameSite=Lax` only). Fix: a `secure` field on `SessionConfig`, on by default.
10. `foundation_auth` `HandlerStorage::sweep_expired`: nothing schedules it; the operator must call `IdpServer::sweep_expired` on a timer. Fix: a timer task started with the server.
11. `foundation_auth` `social_callback`: the authorization code made after a social login has an empty `client_id` (the social session does not keep the client that `social_authorize` now checks). Fix: keep `client_id` in the upstream auth session and pass it to the code.

## Paused

- Windows/macOS VM images (#69).

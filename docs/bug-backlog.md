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
- **`foundation_netio` native build warnings** (seen in `cargo check -p foundation_wireguard --tests`):
  `backends/foundation_netio/src/native/raw_stream.rs:4` has `use std::sync::Arc;` that is unused,
  and `raw_stream.rs:414` has `if let Self::AsPlain(inner, _) = self`, an irrefutable pattern
  (the `if let` always matches). Fix: delete the import; replace the `if let` with a `let`
  binding (keep the behaviour the same and check it with the netio tests).
- **Clippy `too_many_lines` in `SimpleHttpBody::extract`.** File:
  `backends/foundation_netio/src/shared/http/impls.rs:6045`. Problem: the function has 109 lines and
  the limit is 100. `cargo clippy --profile uat -p foundation_netio --all-targets` warns. Fix: move
  each content-coding branch into its own helper function, then keep the function below the limit.
  Do not add `#[allow(clippy::too_many_lines)]`. Seen in the Linux CI run of feature 80 (2026-11-05).
- **Unused optional dependency `rustls-pemfile`.** File: `backends/foundation_netio/Cargo.toml:75`.
  Problem: the `ssl-rustls` feature enables the dependency, but only the `quic` module calls it. The
  default feature set enables `ssl`, so the default build warns (`unused dependency`,
  `cargo::unused_dependencies` is `warn`). Fix: remove `dep:rustls-pemfile` from the `ssl-rustls`
  feature list and keep it in `quic`. Seen in the Linux CI run of feature 80 (2026-11-05).

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
- **`foundation_http` fails `cargo clippy --all-targets -- -D warnings`.** Seen on
  `spec70/f77-f62-f78-readiness-ci` (2026-11-05, feature 62): 32 errors in the lib and the
  tests. Examples: missing `# Errors` sections in `shared/serve/mod.rs` (7) and
  `shared/router/method.rs`; `doc_markdown` in `native/server/mod.rs`, `native/server/connection.rs`
  and eight test files; `must_use` on `shared/middleware/compression.rs`, `shared/web_conn.rs` and
  `shared/middleware/cors.rs`; `too_many_lines` in `native/server/mod.rs` (`serve_loop`, 126/100)
  and `native/server/connection.rs` (147/100); `needless_continue`,
  `redundant_closure_for_method_calls` and `map_unwrap_or` in `native/server/mod.rs`. Fix: clear
  each lint and set `clippy-deny-warnings` for the package. Another agent cleans the lane A
  crates on a separate branch; check that branch before starting.
- **`ConnectionHandler::idle_timeout` (`native/server/connection.rs:173`) is the size-based
  read timeout of a streaming context, 100 ms (`TimeoutConfig::min_read_timeout`) when the
  body is unsized** (the same mistake that made the SSE client reconnect a quiet stream, fixed
  in feature 80 with `sse_idle_timeout`). `idle_exceeded` compares the time since the connection
  went idle with it. Check whether a keep-alive connection that is quiet for 100 ms is closed;
  if so, give the server an idle timeout of its own in `TimeoutConfig` (default from nginx
  `keepalive_timeout`, 75 s). The WebSocket task has the same call for its sleep
  (`websocket/native/task.rs:882`, only a delay there).
- **`foundation_http` test `http3_server_echoes_a_request_over_quic` is flaky** (about 1 to 3 runs
  in 100 alone, with and without feature 80: 3 of 100 on `b3f0aa6a1`, 1 of 60 on
  `wip/f80-valtron-wake`). The client's `poll_headers` gets no response headers within its
  deadline ("response headers" `expect` at `tests/http3_server_tests.rs:209`, after a 15 s
  wait). Not a valtron wake: the same rate before the change. Likely a lost datagram or a
  handshake timing race between the test's client driver and the server `H3AcceptTask`.
  Fix: capture the QUIC and H3 state of a failing run (`tracing`), then close the race.

## 4. foundation_core / valtron

- **`foundation_core` and `foundation_nostd` fail `clippy -D warnings`.** Seen on
  `fix/io-41-50-lane-a` (2026-11-04), `cargo clippy -p foundation_core --no-deps
  --profile dev --lib -- -D warnings`: 19 errors, none in feature 46 code. Files:
  `executors/background.rs:283,299` and `task.rs:214` (`double_must_use`),
  `executors/local.rs:374,375,391,392` and `future_task.rs:616` (`arc_with_non_send_sync`),
  `future_task.rs:936`, `task_iters.rs:127,449` (`question_mark`),
  `extensions/streams/non_sendable.rs:1296,1306` (`while_let_on_iterator`),
  `pipe.rs:114` (`new_ret_no_self`), `pipe_mapped.rs:43,81` (`result_unit_err`,
  `len_without_is_empty`), `io/ioutils/mod.rs:950,1542,1705`. Without `--no-deps` the run
  stops earlier on 5 `redundant_closure_for_method_calls` errors in
  `foundation_nostd/src/comp/fair_gate.rs:33,35,52`. Fix: clean each lint, then keep
  the crate at zero.
- **The bare `--no-default-features` build prints its `compile_error!` and then 24 more
  errors.** `foundation_core` now refuses the configuration with a clear message (feature 78
  E3), but `pub mod valtron` and its executors still compile and emit unresolved-import errors
  for `Ctx`. The first error is the actionable one; the rest is noise. Fix: gate `pub mod
  valtron` on `std`, or put the executors behind the same `cfg`, so only the named error
  remains.
- **The idle CPU tests measure the threads of other tests.** File:
  `backends/foundation_core/tests/valtron/readiness_deadline_tests.rs:300-400`. Problem: the
  tests read `/proc/self/task` and keep every thread whose name ends in `valtron::readin`, which
  is the 15-character truncation of the module path. Pool worker threads inherit the name of the
  thread that starts them, so the workers of another test in the same binary match too. A full
  run under load showed 9 wake-ups and 1.8 ms of CPU on threads of another test, and the bound of
  2 ms then failed. Likely fix: name each pool's workers after the pool (or the test), then match
  that name, or compare the two phases of one test only, as
  `worker_with_event_driven_sleeper_uses_less_cpu_than_a_polling_one` does.

## 5. foundation_nostd / foundation_wasm / foundation_iogate

- **`foundation_iogate` clippy doc-backtick warnings are open.** `cargo clippy -p
  foundation_iogate --no-deps` reports about ten `doc_markdown` warnings (items
  such as `io_uring` and `CompletionSocket` without backticks) in `lib.rs`,
  `shared/mod.rs` and `native/`. Fix: add the backticks.
- **Two `foundation_nativeapis` doc examples are `ignore`d pseudo-code.**
  `src/native/fd/completion.rs:20` uses an undefined `decoder` and
  `src/valtron/file_watcher.rs:142` an undefined `collect_one` stream setup, so
  `cargo test --doc` reports both as ignored and never compiles them. Fix: make each a
  real example with a local task or type, as feature 80 did for `foundation_core`, or
  delete the code block and keep the prose. Seen in feature 78.
- **`foundation_nativeapis` `signal/linux.rs` delivers the signal from the signal handler
  itself.** `signal_handler` calls `deliver_signal`, which locks `SignalBus` mutexes and
  pushes to queues (allocation, `EventQueue` wake). That is not async-signal-safe: a signal
  that interrupts a thread holding the subscriber lock deadlocks. Fix: the handler writes one
  byte to the eventfd (or a self-pipe) and a thread or task reads it and delivers.

## 6. foundation_deployment_docker / foundation_deployment

(no open entries)

## 7. foundation_deployment_platform

(no open entries)

## 8. foundation_openapi / foundation_codegentools

- **`foundation_deployment_docker` clippy debt.** About 231 lints (`format!` inline
  variables, trailing commas, `::default()` calls, missing `# Errors` sections).
  `foundation_openapi` and `foundation_codegentools` are clean (spec 70 feature 55).
  Fix: clear the docker lints in feature 49, then keep them at zero.

- **`foundation_cloudflare_types` has one pedantic clippy warning in generated code.**
  `cargo clippy -p foundation_cloudflare_types --profile uat --all-targets -- -D warnings`
  reports "calling `D1SingleQuery::default()` is more clear than this expression" in
  `src/generated.rs`. Fix the generator's default-value emission (or allow the lint in the
  generated header), regenerate, and keep the crate at zero. Found while rustfmt-ing the
  hand-written re-export list (feature 55 E1).

## 9. foundation_proxy

- **`foundation_proxy` clippy and test warnings are open.** `cargo clippy -p
  foundation_proxy --all-targets` reports about 50 warnings (for example
  `acme.rs`, `h2_proxy.rs`, `server.rs`, `control.rs`; doc backticks, `must_use`,
  redundant closures, `format!` appended to a `String` at `forward.rs`). The
  tests `load_balancer_tests.rs` (unused `Arc`, `Duration`, `STICKY_COOKIE`) and
  `h2_integration_tests.rs:318` (unused `mut`) warn on build. Scheduled as
  feature 49 E7.
- **The idle back-off in `block_on` could outlast a wake.** Fixed in feature 75 by
  bounding the `SpinWait` sleep with `sleeper_wait()`; keep that bound if the
  executor loop is refactored.
- **`foundation_proxy` `control.rs` has an inline `#[cfg(test)] mod tests`.**
  Rule 7 forbids it. Fix: move its tests into `tests/` (scheduled as feature 50).
- **A response writer's send timeout counts from its creation, not from its last
  byte, for long streams.** `forward_http` and the translators use one
  `ResponseWriter` for the whole response, so a gRPC stream idle for longer than
  `relay_send_timeout` times out at its next blocked write. The tunnel splice
  avoids this with a writer per chunk. Fix: feature 63 E (restart the idle timer
  when a write starts).

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

- **`BashTool` stores a `timeout` and never reads it.** `src/agentic/tools/files.rs:264`
  keeps `timeout` ("Max wall-clock time before the command is killed"), but the build and
  run paths never read the field, so a bash command has no time limit (the lib clippy run
  reports it as dead code). Fix: kill the child at the timeout, or delete the field and its
  configuration. Seen in the Linux CI run of feature 81 (2026-11-05).
- **`foundation_ai` has clippy debt in every test and example target.** The Linux CI run of
  feature 81 counted 163 warnings in `foundation_ai_tests`, 40 in `provider_tests`, 36 in
  `tool_tests`, 33 in `agentic_tests`, 25 in `candle_tests`, 19 in `model_tests`, 2 in
  `memory_tests`, 1 to 10 in each example, and 1 in the lib. Most are pedantic style
  (`doc_markdown`, `collapsible_match`, `manual_repeat_n`). None come from the feature 81
  files. Fix: a cleanup pass per target, then keep the zero-warning rule.

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
- **Turso store rejects overlapping calls: "Backend error: concurrent use forbidden".**
  `turso_sdk_kit` allows one use of a connection at a time, and the Turso
  `StorageProvider` shares one connection. Now that the keychain server runs
  handlers as `ConnectionResult::Async` (features 44 and 64), two requests that
  reach the store at the same moment make one fail with HTTP 500. Seen in
  `foundation_keychain/tests/streamed_body.rs` when two slow uploads finish
  together. Fix: a connection per call or a small pool, or serialise calls in
  the Turso backend (feature 51 owns the shared-connection work).
- **`foundation_proxy` can drop its state file lock.** `set_if_version` (compare on
  `updated_at`) is now atomic on the file, libsql, D1, R2 and namespaced stores (feature 76 E4
  and E4b). The proxy keeps a `flock` around its read-modify-write; it can use
  `set_if_version` instead (lane A owns the proxy). `D1Store`'s version is tested only on a
  hosted or wrangler D1 (`CF_INTEGRATION_TEST`).
- **workerd's local R2 binding enforces `onlyIf` but does not report a failed precondition.**
  In wrangler 4.156.2 the object PUT with `onlyIf: { etagMatches }` or
  `etagDoesNotMatch` leaves an existing object unchanged and resolves the promise, instead of
  rejecting. `R2WasmStorage::put_blob_conditionally` therefore keeps the trait's documented
  non-atomic check-then-write, which reports `PreconditionFailed`. The E0 harness tests that
  contract; an atomic wasm conditional put needs a workerd that rejects. Re-check on a newer
  wrangler.

## 12. foundation_auth

- **The `server`/`server-native` feature tree has over 200 clippy warnings.**
  `cargo clippy -p foundation_auth --features server-native` reports 212 lints
  across `src/server/` (`config.rs`, `handlers/*`, `services/*`): redundant
  closures, missing `#[must_use]`, docs missing backticks, `too_many_lines`,
  and more. The CI clippy step runs default features only (`runs[0] = ""`), so
  none of these are enforced. Fix: a dedicated clippy-cleanup pass over the
  server tree, then extend the CI clippy run to `server-test` so the debt
  stays cleared.

- **R2 range read and conditional put have no test yet.** Wrangler's local explorer API does not
  serve them; they need the wasm-testbed harness on the real R2 binding (feature 76 E0).

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

- **`foundation_testbed` clippy debt outside the wasm runner.** `cargo clippy -p
  foundation_testbed --profile uat --all-targets --no-deps -- -D warnings` still reports
  31 warnings in the lib and 1 in `tests/fwt_discovery_tests.rs` after feature 76 fixed
  the three in `wasm/wrangler.rs` and `wasm/wasm.rs`: `qemu_control.rs` (missing
  `# Errors`/`# Panics`, casts, identical match arms, redundant closure), `browser.rs`,
  `build.rs` (collapsible `if`, long function, case-sensitive extension check),
  `fwt_runner.rs`, `init.rs`, `wasm/mod.rs` (module name) and the test's doc backticks.
  Fix: a clippy pass over the crate, then `clippy-deny-warnings = true` in
  `ci/packages.toml`.

- **`platform-docker-tests.yml` fails on every run, on master too.** All three
  jobs fail. `platform-unit` (ubuntu-latest, no CI container) runs
  `cargo test -p foundation_platform`, whose Linux dependency tree includes
  `tauri` → `gtk` → `gobject-sys`, so `gobject-sys`' build script stops on a
  missing `gobject-2.0` pkg-config file (no `libgtk-3-dev` or `libglib2.0-dev`
  on the runner). `windows-tests` and `macos-tests` fail in the same workflow.
  Fix: install the GTK development packages in the job, or gate the GUI
  dependency behind a feature the job does not enable; then find the cause of
  the other two jobs. Found while reading the spec 70 pull request checks
  (2026-11-05); it is not caused by them.

- **`foundation_reactor` publish fails tarball verification: `foundation_core` is stale on crates.io.**
  `foundation_reactor` 0.0.1 (a new crate) depends on `foundation_core` 0.0.6, but its source uses
  `foundation_core::valtron::{EventReadiness::register, ReadinessNotifier, ReadinessWaker}`
  (`reactor.rs`, `shutdown_signal.rs`, `fd_readiness.rs`) and `EventReadiness::clear`
  (`fd_readiness.rs`) that were added to the local tree after 0.0.6 was
  published. `release-plz release` verifies the tarball against the published 0.0.6, so it fails with
  E0425/E0407. The 0.0.7 bump has landed (`foundation_core` and `foundation_macros` are now
  0.0.7 on `fix/io-35-turso-async-trait-overflow`); the crates.io publish still needs to
  happen at release time so 0.0.7 publishes before `foundation_reactor`.

## 17. foundation_vectors

## 21. Wasm-only warnings (seen in `cargo check --target wasm32-unknown-unknown -p foundation_keychain`)

## 22. foundation_keychain

- **`foundation_keychain` clippy debt.** `cargo clippy -p foundation_keychain
  --profile dev --no-deps` reports 174 warnings in the lib (for example
  `manual_let_else` in `provisioning_route`) and more in the tests. Fix: follow
  the per-crate clippy workflow in `.agents/AGENTS.md`.

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

## 25. foundation_auth

(no open entries; features 68, 73 and 74 fixed the ones found in the ToS gate work)

## 26. foundation_platform

- **The crate is not clippy-clean.** `cargo clippy --profile uat -p
  foundation_platform --all-targets --no-deps` reports 146 warnings in the
  library (61 after duplicate removal) and more in the existing test targets.
  The main lints are `doc_markdown`, `must_use_candidate`, `missing_errors_doc`
  and `redundant_closure_for_method_calls`, in `src/session.rs`,
  `src/backend/`, `src/capability.rs`, `src/codegen.rs`, `src/multi_app.rs` and
  others. Found in feature 59 when its inline tests moved out; the moved tests
  themselves are clean. Fix: run the per-crate clippy workflow in
  `.agents/AGENTS.md`, then set `clippy-deny-warnings` for the package in
  `ci/packages.toml`.

## 27. foundation_packager

- **The crate is not clippy-clean.** `cargo clippy --profile uat -p
  foundation_packager --all-targets --no-deps` reports about 80 warnings in the
  vendored `src/tinytemplate/` copy (format-argument, doc-backtick and legacy
  lint items) and about 16 in `src/files.rs` and `src/package.rs` (missing
  `# Errors`/`# Panics` sections, `needless_pass_by_value`, `too_many_lines`,
  `assigning_clones`). `foundation_conditional` is used by features only, so
  cargo also reports it as an unused dependency. Found in feature 59; the moved
  test files are clean. Fix: clean the vendored copy by upgrading it from
  upstream TinyTemplate, clean the two source files, and set
  `clippy-deny-warnings` in `ci/packages.toml`.

## 28. infrastructure_llama_cpp

- **A worktree without submodule content cannot build this crate.** A worktree
  has no `tools/llama.cpp/llama.h`, so `infrastructure_llama_bindings`' build
  script stops with `fatal error: 'llama.h' file not found`. The main checkout
  has the submodule: the seven moved tests compile and pass there (2026-11-05).
  Fix: run `git submodule update --init tools/llama.cpp` in a worktree before
  building this crate.

## 29. foundation_jsonschema

- **Accepted exception: the crate is a vendored copy of the upstream
  `jsonschema` crate, so its inline tests stay.** About 40 source files carry
  `#[cfg(test)] mod tests`. Feature 59 (rule 7) does not move them, because the
  copy tracks upstream and moving them would grow the fork diff. The owner
  must confirm this exception. If the crate is no longer a vendored copy, move
  the tests and delete this entry.

## 30. Proc-macro crates (foundation_macros, crates/html-macro, crates/template-macro)

- **Their inline tests cannot move to `tests/` as they stand.** A
  `proc-macro = true` crate can export only macros, so integration tests can
  reach only the macros. The inline modules in `foundation_macros`
  (`wasm_modes.rs`, `wasm_logs.rs`, `connectrpc_service.rs`,
  `html_macro/{parser,scoped_css}.rs`, 33 tests) call private helpers
  (`wasm_bin`, `wasm_service`, `wasm_logs`, `parse_template`,
  `transform_scoped_css`). `crates/html-macro` and `crates/template-macro` have
  a trybuild harness each. Fix: move the helpers into a normal library crate
  that the proc-macro crate calls (the feature 59 guidance), then test them
  from `tests/`.

- **`crates/template-macro`'s trybuild harness is broken.** Its
  `test/jinja/main.rs` fixture imports `foundation_packager::minijinja`, but
  `foundation_packager` does not re-export `minijinja`, and
  `foundation_packager` is not a dev-dependency of the macro crate. The
  `jinja` pass case fails with `E0432: unresolved import`. The crate is excluded
  from the root workspace (`crates/*`), so the harness never ran. Found in
  feature 59 when the move was attempted; the move was reverted. Fix: re-export
  `minijinja` from `foundation_packager`, make the macro crate a standalone
  workspace root, then move the harness to `tests/`.

- **`crates/html-macro`'s trybuild pass fixtures need `dioxus`.** The
  `test/attribute/passes.rs` fixture uses `dioxus::prelude::*`, but `dioxus` is
  not a dev-dependency. The crate is excluded from the workspace and cannot
  build. Found in feature 59; its harness stays inline. Fix: add a `dioxus`
  dev-dependency (choose a version that matches the `rsx!` output) and move the
  harness, or record why the macro is no longer supported.

## 31. Lane B clippy debt found while moving tests (feature 59)

These warnings are in code the feature did not change. The moved test files
are clean. Each item needs the per-crate clippy workflow in
`.agents/AGENTS.md`, then `clippy-deny-warnings` in `ci/packages.toml`.

- **`foundation_connectrpc_codegen`**: `src/generator.rs` has 76
  `clippy::format_push_string` warnings (use `write!` into the output
  `String`), one `too_many_lines`, one `missing_panics_doc` and one
  doc-backtick warning. The generator tests assert the generated output, so
  they guard the rewrite.
- **`foundation_testing`**: the existing `tests/scenarios/barrier_debug.rs`
  (4), `tests/scenarios/integration_tests.rs` (5) and
  `tests/integration_tests.rs` (1) have style warnings.
- **`foundation_codegen`**: `tests/wasm_roundtrip_tests.rs` (1) and
  `tests/units/{foundation_codegen_visitor_tests,foundation_codegen_file_walker_tests}.rs`
  (1 each) have style warnings; the manifest reports one unused dependency.
- **`foundation_repl`**: `tests/wrapping.rs` and `tests/layout.rs` have one
  warning each.

## 32. foundation_toolings

- **The dev builder task cannot see a second file change.** `ProjectBuilderTask` parks on
  its change and result readinesses, but the watch events arrive on `change_rx` (an
  `mpp::Receiver`), and nothing wakes the task when a sender calls `send`. The task also
  never pops `change_queue`, so after the first queued change the composite readiness stays
  ready and the executor re-runs the task in a hot loop (`builder/mod.rs`,
  `change_readiness` and `result_readiness`). Feature 81 converted the queues to
  `EventQueue` and did not change these two faults. Fix: push each watch event into
  `change_queue` (its push wakes the task) and pop it when consumed, or park on the
  receiver's own readiness.
- **`FileChangeEventPump` has no user and duplicates its readiness.** No crate or test calls
  it, and its `ready` field is a second copy of `queue.readiness()` (`watcher/mod.rs`).
  Fix: delete the pump, or delete `ready` and build the readiness from the queue on
  demand.
- **`DurationSleeper` reads the wall clock directly.** `runner/mod.rs` calls
  `SystemTime::now()`, which project rule 11 forbids in core logic: clocks must come from
  the `Env` bundle in `Ctx`. Fix: read a clock trait from `Ctx`, and use a simulated clock
  in the tests.

## Paused

- Windows/macOS VM images (#69).

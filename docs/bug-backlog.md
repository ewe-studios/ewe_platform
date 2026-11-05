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

- **The `read_wake_source` pattern is irrefutable without a TLS backend.** With
  `--no-default-features` (no `ssl-*` feature) `RawStream` has only the `AsPlain`
  variant, so `if let Self::AsPlain(inner, _) = self` warns as an irrefutable
  pattern. Keep the `if let`: the TLS feature sets add the other variants. Found
  in feature 49; the default and all-feature sets are warning-free.

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

- **`foundation_nostd` `test_wait_duration_zero_duration` has a too-tight
  bound.** `tests/primitives_wait_duration_tests.rs:35` requires a zero-duration
  wait to return in under 1 µs. On the amd64 OrbStack emulation it takes about
  34 µs, so the test fails there; on native amd64 it may pass. Fix: measure the
  real floor on native CI and set a bound that still catches a park (for
  example a few hundred microseconds), or gate the strict bound to native
  hardware. Found in feature 49.

- **`foundation_nativeapis` `signal/linux.rs` delivers the signal from the signal handler
  itself.** `signal_handler` calls `deliver_signal`, which locks `SignalBus` mutexes and
  pushes to queues (allocation, `EventQueue` wake). That is not async-signal-safe: a signal
  that interrupts a thread holding the subscriber lock deadlocks. Fix: the handler writes one
  byte to the eventfd (or a self-pipe) and a thread or task reads it and delivers.
- **`foundation_nostd` has 19 ignored doc examples.** `cargo test -p foundation_nostd
  --features std --doc` reports 19 ignored examples in `src/primitives/noop.rs` and
  `src/macros.rs`. They are pseudo-code (`let x = Nop::new()` style), so they never
  compile. Fix: turn each into a real doctest with a local value and an assertion, as
  feature 80 did for `foundation_core`. Found in feature 48.
- **`foundation_nativeapis` clippy debt.** On Linux, `cargo clippy -p foundation_nativeapis
  --all-targets --no-deps -- -D warnings` reports 191 warnings with default features and 643
  with the full VFS feature set (the CI run includes `vfs-ptrace`). The lints are
  `borrow_as_ptr`, `cast_sign_loss`, `doc_markdown` and related mechanical lints. Fix: a
  clippy-cleanup pass per feature set, then set `clippy-deny-warnings = true` in
  `ci/packages.toml`. Found in feature 47.
- **The ptrace spawn tests cannot run under amd64 emulation.** On an Apple Silicon host that
  runs the linux/amd64 CI image with OrbStack, `PTRACE_GETREGS` returns `EIO` for every
  traced amd64 process. A minimal C program shows the same result, so the cause is the
  emulation, not the tracer. Fix: run `ptrace_vfs` on a native amd64 kernel (GitHub CI). A
  local `scripts/ci/linux.sh foundation_nativeapis` reports 8 spawn-test failures for this
  reason. Found in feature 47.

## 6. foundation_deployment_docker / foundation_deployment

(no open entries)

## 7. foundation_deployment_platform

- **CI does not lint the `vms` feature tree.** The CI clippy and build steps run
  default features only (`runs[0] = ""`); `vms` compiles only under
  `cargo test --features vms`, which does not lint. The `vms` tree is now at
  zero clippy warnings (cleaned 2026-11-02), but nothing keeps it there.
  Fix: extend the CI clippy run to the `vms` feature so this debt stays zero.

## 8. foundation_openapi / foundation_codegentools

- **The docker generator must emit clippy-clean results.** Feature 49 cleared
  the `foundation_deployment_docker` warnings, including the generated endpoint
  modules, but the generator still emits the old shapes. A regeneration brings
  the warnings back. Fix: update the generator templates to match the fixed
  generated code, then regenerate and diff.
- **The `foundation_wireguard` mobile modules had no declaration.** `src/ios`
  and `src/android` were not declared in `lib.rs`, so their code and tests never
  compiled, while `completeness.rs` referred to them. Feature 50 declared them
  for their target OS and for `spec55-complete`. Still open: no iOS/Android
  toolchain run covers them; add one when the mobile build joins CI.


## 9. foundation_proxy


- **Lane B crates still build `QueueReadiness::new` over a plain queue, which keeps the
  1 ms re-check.** Feature 80 made `EventQueue` (its push, pop and close wake the parked
  task) the way to park on a queue and deprecated `QueueReadiness::new`,
  `QueueVacancyReadiness::new` and their `event_driven` aliases. Remaining users, all
  lane B: `foundation_toolings` (`runner/mod.rs:83`, `watcher/mod.rs:63`,
  `builder/mod.rs:70,72`), `foundation_ai` (`agentic/steering.rs:169,176`),
  `foundation_netio` `shared/http/pushable_body.rs` and `quic/traits.rs` (doc mentions).
  Fix: feature 81 switches each queue to `EventQueue`, then deletes the deprecated
  constructors.
- **The idle back-off in `block_on` could outlast a wake.** Fixed in feature 75 by
  bounding the `SpinWait` sleep with `sleeper_wait()`; keep that bound if the
  executor loop is refactored.
- **`foundation_proxy` `control.rs` has an inline `#[cfg(test)] mod tests`.**
  Rule 7 forbids it. Fix: move its tests into `tests/` (scheduled as feature 50).
- **CI does not set `EWE_REQUIRE_IO_URING=1`.** The `foundation_iogate` and
  `foundation_proxy` io_uring tests now fail when that variable is `1` and io_uring
  is missing, and skip with a message otherwise. CI runs every package in a Docker
  `container:` job (`.github/workflows/check.yaml`); Docker's default seccomp
  profile can block `io_uring_setup`, and the CI image kernel was not confirmed to
  allow it, so the variable is not set. Fix: run one CI job with the variable and
  keep it if the tests pass. The `foundation_nativeapis` `uring_completion_*` and
  `completion_source_seam` tests still skip without the variable check.
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
- **`StateStore::set_if_version` is not atomic on `R2Store`, and the proxy still has its file lock.**
  `set_if_version` (compare on `updated_at`) is atomic on the file, libsql, D1 and namespaced
  stores. `R2Store` uses the read-compare-write default: the R2 conditional put needs the
  object's `ETag`, which the state API does not carry. `foundation_proxy` can drop its flock
  where it uses a store with an atomic `set_if_version` (lane A owns the proxy). `D1Store`'s
  version is tested only on a hosted or wrangler D1 (`CF_INTEGRATION_TEST`).

- **`foundation_nativeapis` `vfs-preload` does not compile.** `src/native/vfs/shim/mod.rs` (around
  lines 163 to 190) uses `guard` after it moved (`E0382`), so `tests/vfs_shim_vfs` cannot run.
  Found while moving the CF tests to the local explorer API (feature 76).
- **R2 range read and conditional put have no test yet.** Wrangler's local explorer API does not
  serve them; they need the wasm-testbed harness on the real R2 binding (feature 76 E0).

## 16. Workspace / process

- **`foundation_wireguard` clippy debt in its source and tests.** `cargo clippy
  -p foundation_wireguard --all-targets --no-deps` reports 180 warnings:
  `doc_markdown` (72), `must_use_candidate` (14), `missing_errors_doc` (8),
  `ip_constant` (7), casts, `unused_imports` (5) and the rest. The feature 49 E8
  work fixed only the manifest. Fix: a clippy-cleanup pass, then set
  `clippy-deny-warnings = true`. Found in feature 50.
- **`foundation_reactor` clippy debt.** `cargo clippy -p foundation_reactor
  --all-targets --no-deps` reports 124 warnings: `must_use_candidate` (33),
  `missing_errors_doc` (23), `doc_markdown` (17), `cast_possible_wrap` (13) and
  the rest. Found in feature 50. Fix: a clippy-cleanup pass.


- **`foundation_boringtun` (fork of upstream boringtun) keeps upstream's inline tests and nine bare `#[ignore]`s.**
  `src/device/integration_tests/mod.rs` has nine `#[ignore]` tests (lines 466, 475,
  543, 590, 625, 661, 700, 750, 801) that need root, a TUN device or the `wg` tool,
  and other source files carry `#[cfg(test)] mod tests`. They are upstream's, left
  unchanged so the fork diff stays small. Fix: move the tests into
  `backends/foundation_boringtun/tests/` (made runnable in CI, with the root-only
  ones gated by a runtime check and not by `#[ignore]`) when we next change that
  source.



- **`foundation_reactor` wasm dead-code warning: `rearm_readable` is never used.**
  `Poll::rearm_readable` (`src/poll/mod.rs:322`) is `pub(crate)` and only called
  from `src/reactor.rs:565`, which is a native (kqueue/poll) path. On wasm32 the
  call site is compiled out, so `cargo check --tests --target wasm32` on
  foundation_wireguard/foundation_netio emits `method rearm_readable is never
  used`. Fix: gate the method on the same `cfg` as its call site.
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

## 21. foundation_deployment_cloudflare on wasm32

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


- **`foundation_core` does not build for wasm32 with neither `std` nor `alloc`**
  (`cargo check -p foundation_core --target wasm32-unknown-unknown --no-default-features`):
  `valtron::Ctx` is unresolved in `executors/collect_next.rs` and `dependent_lift.rs`. CI
  never runs that configuration (its wasm runs all add `std`). Fix: either gate the
  executors that need `Ctx` on `alloc`, or declare in `Cargo.toml` docs that `alloc` is the
  minimum and make a bare build fail with a `compile_error!` that says so.


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

## Paused

- Windows/macOS VM images (#69).

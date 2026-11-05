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

## 2. foundation_netio

- **`silent_server_gets_body_when_the_timer_fires` is timing-flaky under load.**
  A full `foundation_netio` run on 2026-11-05 failed this test once while two other
  cargo test suites built and ran on the same machine. It passed 10 of 10 solo
  reruns. The test expects the server to wait about its 300 ms
  `Expect: 100-continue` timer, and asserts the wait is under 2 s
  (`tests/simple_http/nonblocking_client_tests.rs:459`). Load stretches the wait past
  that bound. Unrelated to feature 82. Fix: drive the timer from a simulated clock, or
  widen the bound only with a measured justification.
- **The `read_wake_source` pattern is irrefutable without a TLS backend.** With
  `--no-default-features` (no `ssl-*` feature) `RawStream` has only the `AsPlain`
  variant, so `if let Self::AsPlain(inner, _) = self` warns as an irrefutable
  pattern. Keep the `if let`: the TLS feature sets add the other variants. Found
  in feature 49; the default and all-feature sets are warning-free.

## 3. foundation_http

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
  The same run with `--all-targets` also reports warnings in the examples
  (`examples/valtron_traffic_light.rs`, `valtron_state_task.rs`, `valtron_orchestration.rs`
  and `valtron_streaming.rs`): `clippy::drop_non_drop` on the `PoolGuard` the
  `#[valtron]` attribute expands to, and `clippy::needless_pass_by_value` on `Ctx`
  parameters. Seen 2026-11-05 (feature 114). The package does not set
  `clippy-deny-warnings`, so CI reports them and passes. Fix: the macro must not emit
  `drop()` for its guard, and the examples take `Ctx` by reference.
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

- **Two `foundation_nativeapis` doc examples are `ignore`d pseudo-code.**
  `src/native/fd/completion.rs:20` uses an undefined `decoder` and
  `src/valtron/file_watcher.rs:142` an undefined `collect_one` stream setup, so
  `cargo test --doc` reports both as ignored and never compiles them. Fix: make each a
  real example with a local task or type, as feature 80 did for `foundation_core`, or
  delete the code block and keep the prose. Seen in feature 78.
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
- **The ptrace `open`/`openat` path ignores `O_TRUNC` on an existing virtual file.**
  `handle_open` and `handle_openat` in
  `backends/foundation_nativeapis/src/native/vfs/ptrace/syscall_dispatch.rs` call
  `fs.create` whenever `O_CREAT` is set. `MemoryFs::create` returns `AlreadyExists` for an
  existing path, while `open(2)` with `O_CREAT` (without `O_EXCL`) must open that file and
  truncate it. A traced shell redirect such as `echo x > /virtual/existing` therefore fails.
  Fix: when `O_CREAT` is set and the file exists, open it and call `truncate(0)`. Found in
  feature 83.
- **The ptrace `dup2` handler does not close a real fd at the destination.** `handle_dup`
  inserts the virtual entry at the new number and skips the kernel call. If the new number
  held a real fd (the common case is fd 1), that kernel fd stays open, while `dup2` must
  close it. Fix: close the real destination fd once the virtual entry owns the number, or
  keep the kernel call and intercept the writes by table lookup. Found in feature 83 while
  fixing the shell-redirect dispatch.
- **The ptrace spawn tests need a native amd64 kernel.** On an Apple Silicon host that runs
  the linux/amd64 CI image with OrbStack, `PTRACE_GETREGS` returns `EIO` for every traced
  amd64 process. A minimal C program shows the same result, so the cause is the emulation,
  not the tracer. Native coverage comes from GitHub CI: the `foundation_nativeapis` `package`
  job runs on the `ubuntu-latest` x86_64 runner, inside the linux/amd64 image, which shares
  the runner kernel. Its `ptrace probe` step prints `ptrace probe: available`, and the named
  `vfs-native,vfs-ptrace` run executes the suite (feature 83). CI run 42040473182 on
  `e01e83d80` confirms this: the `foundation_nativeapis` job is green with
  `EWE_REQUIRE_PTRACE=1`. A local `scripts/ci/linux.sh foundation_nativeapis` still reports
  the spawn-test failures for this reason; that is the recorded emulation limit, not a code
  defect. Found in feature 47, updated in feature 83.

## 6. foundation_deployment_docker / foundation_deployment

(no open entries)

## 7. foundation_deployment_platform

(no open entries)

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


- **`foundation_cloudflare_types` has one pedantic clippy warning in generated code.**
  `cargo clippy -p foundation_cloudflare_types --profile uat --all-targets -- -D warnings`
  reports "calling `D1SingleQuery::default()` is more clear than this expression" in
  `src/generated.rs`. Fix the generator's default-value emission (or allow the lint in the
  generated header), regenerate, and keep the crate at zero. Found while rustfmt-ing the
  hand-written re-export list (feature 55 E1).

## 9. foundation_proxy

- **The idle back-off in `block_on` could outlast a wake.** Fixed in feature 75 by
  bounding the `SpinWait` sleep with `sleeper_wait()`; keep that bound if the
  executor loop is refactored.
- **`foundation_proxy` `control.rs` has an inline `#[cfg(test)] mod tests`.**
  Rule 7 forbids it. Fix: move its tests into `tests/` (scheduled as feature 50).
- **`tests/proxy_features_tests.rs::websocket_upgrade_relays_and_tunnels_bytes`
  fails on macOS.** The client receives the `101` and writes its frame, but
  `read_exact` times out (`WouldBlock`) waiting for the echo (line 464). It fails
  the same way with the feature 63 writer changes reverted for the run, and the
  proxy's upgrade path (`forward_upgrade` + `aio::splice`) is unchanged, so it
  predates that feature. Likely a Darwin read-wakeup or timing problem in the
  upstream→client pump. Reproduce with
  `cargo test -p foundation_proxy --test proxy_features_tests websocket_upgrade_relays_and_tunnels_bytes`.
  Fix: trace the splice pumps on macOS, or gate the test to Linux until the pump
  is fixed.

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

- **`foundation_shell` is an empty placeholder crate.** `backends/foundation_shell/src/lib.rs`
  holds only `add(left, right)`, and nothing in the workspace depends on it. It still has a CI
  package entry, so every run builds it for no benefit. Spec 59 puts the shell work in a new
  `foundation_shell_repl` crate instead. Fix: delete the crate and its `ci/packages.toml`
  entry, or give it the real implementation. Found while auditing the errors for feature 67
  (2026-11-05).
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



- **`foundation_testbed` clippy debt outside the wasm runner.** `cargo clippy -p
  foundation_testbed --profile uat --all-targets --no-deps -- -D warnings` still reports
  31 warnings in the lib and 1 in `tests/fwt_discovery_tests.rs` after feature 76 fixed
  the three in `wasm/wrangler.rs` and `wasm/wasm.rs`: `qemu_control.rs` (missing
  `# Errors`/`# Panics`, casts, identical match arms, redundant closure), `browser.rs`,
  `build.rs` (collapsible `if`, long function, case-sensitive extension check),
  `fwt_runner.rs`, `init.rs`, `wasm/mod.rs` (module name) and the test's doc backticks.
  Fix: a clippy pass over the crate, then `clippy-deny-warnings = true` in
  `ci/packages.toml`.

- **No CI job builds `examples/platform_ios` for `aarch64-apple-ios`.** The Platform
  Docker Tests workflow used to run this check with `docker exec`, which reaches the
  container's Linux side, never the macOS guest: it installed a Linux Rust toolchain
  (`x86_64-unknown-linux-gnu`) and then failed to find the repository at `/shared`. That
  step was removed on 2026-11-05, and the workflow now smoke-tests the VM host only. The
  iOS example still needs a build on real macOS: add `rustup target add
  aarch64-apple-ios` to the macOS VM image, then `cargo check --manifest-path
  examples/platform_ios/src-tauri/Cargo.toml --target aarch64-apple-ios` to the `run:`
  block of the `macos-ssh` job in `.github/workflows/platform-vms.yaml`. The example
  needs `cargo tauri ios init` first. Found while fixing the spec 70 pull request
  checks (2026-11-05).

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

- **`tests/streamed_body.rs::slow_request_body_does_not_hold_worker` fails on macOS.**
  The slow clients' `read_response` helper panics with `WouldBlock` at
  `tests/streamed_body.rs:123` (`read response: Os { code: 35, kind: WouldBlock }`).
  The fast request before it returns `200`, so the server keeps both uploads open.
  The helper already sets a 10 s read timeout, and on macOS an expired
  `SO_RCVTIMEO` reports `WouldBlock`, so the read waited the full 10 s for a 2 s
  upload. Measured 2026-11-05: every run fails while the machine's load average is
  44 on 10 cores, and the `foundation_keychain` job is green on the Linux CI
  container. Most likely the machine, not the server.
  Fix: make the helper report a timeout as a timeout (distinguish it from a
  non-blocking socket), and make the assertion measure progress — the third
  request answers while the uploads are open — rather than an absolute 2 s.
  Reproduce with
  `cargo test -p foundation_keychain --test streamed_body slow_request_body_does_not_hold_worker`.
  Confirmed on `spec70/io-hardening-final` with this feature's changes reverted, so
  it predates feature 67.

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
- **`foundation_wireguard`**: `cargo clippy -p foundation_wireguard --profile uat
  --all-targets --no-deps -- -D warnings` reports pedantic debt (about 134
  warnings, mostly `must_use_candidate` and `missing_errors_doc` in
  `src/wasm/mod.rs` and the tests/examples). The crate has no
  `cli-allow`-style exception and no `clippy-deny-warnings` entry in
  `ci/packages.toml`. Fix: apply the per-crate clippy workflow, then add
  `clippy-deny-warnings = true`. Feature 67 did not introduce these warnings.
- **`foundation_sshkit`**: the same `-D warnings` run reports 16 pedantic
  warnings (doc backticks in `src/lib.rs`, `missing_errors_doc` on the
  `Backend` trait in `src/backends/mod.rs`, `must_use_candidate`,
  `redundant_closure_for_method_calls` and `default_trait_access` in
  `command.rs`/`host.rs`). All predate feature 67. Fix: the same workflow,
  then `clippy-deny-warnings = true`.
- **`foundation_arrow`**: the same run reports 19 pedantic warnings, mostly
  `missing_errors_doc` on the public `ipc.rs` functions (they returned
  `Result` before feature 67 too), plus style warnings in `dom_ops.rs` and
  `traits.rs`. Fix: add `# Errors` sections and clear the rest, then
  `clippy-deny-warnings = true`.
- **`foundation_nativeapis`**: the same run reports 188 pedantic warnings in the
  default feature tree. The crate has no crate-level allow list and no
  `clippy-deny-warnings` entry. Fix: clear the tree feature by feature (start
  with the default tree), then deny warnings. Feature 67 changed only the
  error derives.

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

## 33. foundation_browser

- **Clippy debt: 56 errors under `-D warnings` in untouched files.** Command:
  `cargo clippy --profile uat -p foundation_browser --all-targets --no-deps -- -D warnings`.
  Found while feature 82 changed `src/test/stream.rs`; that file is now clean. The
  rest: `bidi/backend.rs` 11, `cdp/launch.rs` 6, `browser.rs` 6, `bidi/mod.rs` 6,
  `lib.rs` 4, `jsonrpc/engine.rs` 4, `geometry.rs` 4, `test/server.rs` 3,
  `supervisor.rs` 3, `backend.rs` 3, `locator.rs` 2, `jsonrpc/mod.rs` 2,
  `test/mod.rs` 1, `test/harness.rs` 1. Kinds: missing doc backticks (36),
  `cloned` where `copied` applies (4), `f64`-to-`i64` casts (4), needless
  pass-by-value (3), identical match arms (2), and one each of single-pattern
  `match`, missing `# Panics`, missing `# Errors`, doc list indentation, clamp
  pattern, `map(...).unwrap_or(...)` on a `Result`. Fix: run the crate hygiene
  plan/execute pass before the next feature that touches this crate.

## 35. foundation_nativeapis

- **The `vfs-preload` interposers take the mode as a fixed argument.** `open`,
  `open64`, `openat` and `__openat64_time64` are libc variadic functions. Rust
  defines them in `src/native/vfs/shim/mod.rs` with a fixed third argument,
  because stable Rust cannot define a variadic `extern "C"` function. The
  caller writes that value where a fixed third argument lives on x86-64, x86,
  AArch64 and RISC-V, so those targets are correct. 32-bit ARM passes variadic
  arguments on the stack, so `O_CREAT` would read a wrong mode there. Fix:
  build the shim only for the targets above, or define the entry points with
  `c_variadic` once it is stable.
- **Clippy debt under `-D warnings`.** `cargo clippy --profile uat
  -p foundation_nativeapis --all-targets --no-deps -- -D warnings` reports 191
  warnings, and the same command with `--features vfs-preload` reports 465. The
  `vfs-preload` code accounts for the difference, mostly `non_std_lazy_statics`,
  `doc_markdown` and raw-pointer lints. CI does not deny warnings for this
  package, so the debt does not fail a run. Fix: a hygiene pass, then set
  `clippy-deny-warnings = true`.

## Paused

- Windows/macOS VM images (#69).

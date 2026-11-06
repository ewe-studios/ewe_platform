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

- **`H2Transport::unix` dials with a blocking `UnixStream::connect`.** File
  `src/native/transport/h2.rs` (`H2Transport::start_dial`). The TCP path now starts a
  non-blocking connect (feature 127), but the Unix path still blocks the valtron worker
  that calls `open()` and has no connect timeout at all, so a peer that never accepts
  holds the worker for as long as the kernel waits. `UnixStream::connect` has no
  non-blocking form on every platform. Fix: on unix, create the socket with `socket2`,
  set it non-blocking, connect to the `SockAddr`, and settle it from `SO_ERROR` in
  `PumpPhase::Connecting` exactly as the TCP path does (the reactor already watches the
  fd). Found in feature 127.
- **`CancelSignal` keeps one waker per facade wait until it fires.** File
  `src/shared/context/cancel.rs`. Each `receive()` on a stream builds a fresh
  `cancelled()` future, and each poll registers a `ReadinessWaker` built with
  `ReadinessWaker::from_waker`, whose `is_live()` is always true (it has no
  `ParkState`). The signal's `ReadinessNotifier` therefore cannot drop a waker
  from a finished wait, so a long call that returns thousands of messages keeps
  thousands of wakers. The count is unchanged by feature 138; the old vector had
  the same growth plus one clone per poll. Fix: register the call's waker once
  (for example keep one `cancelled()` future per call and share it), or give the
  notifier a liveness token for a bridged waker. Found in feature 138;
  scheduled as feature 140.

## 2. foundation_netio

- **The `read_wake_source` pattern is irrefutable without a TLS backend.** With
  `--no-default-features` (no `ssl-*` feature) `RawStream` has only the `AsPlain`
  variant, so `if let Self::AsPlain(inner, _) = self` warns as an irrefutable
  pattern. Keep the `if let`: the TLS feature sets add the other variants. Found
  in feature 49; the default and all-feature sets are warning-free.
- **`RawStream::read_wake_source` registers a new readiness token on every call.**
  File `src/native/raw_stream.rs:480`. Each call builds a fresh `FdReadiness` (a new
  token) for the same fd, so two live sources on one fd can replace each other's
  selector registration, and the first one then never wakes. No current caller holds
  two sources at once, so no test hits it. Fix: build the source once per stream
  (cache it on `RawStream`) and hand out clones of the same `Arc`, or refuse a second
  call with a typed error. Found in feature 129; scheduled as feature 137.

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

- **Executor parking stays on the real monotonic clock.** Feature 115 makes the
  clock in `Ctx` replaceable, so a task reads an injected clock from
  `ctx.clock()`. The executor's own waits do not read that clock: the single
  executor parks with `std::thread::park_timeout`, and the multi workers back
  off on `thread::sleep`. A simulated clock therefore does not drive a sleep or
  a deadline. A test that advances a simulated clock past a deadline must also
  wake the worker (feature 108's `Expect: 100-continue` test does this with a
  second poll). Fix: route the executor's idle wait and timer deadlines through
  the injected `Clock`, behind a wake path that a simulated clock can trigger.
  Found in feature 115.
- **`#[task]` forces the transition's context by value.** The macro's generated
  `StateTask` uses the function pointer type `fn(Ctx, T)`, and its validation
  accepts only a bare `Ctx` type (`backends/foundation_macros/src/state_task.rs`,
  `type_is(ctx_ty, "Ctx")`). A transition that reads the context but does not
  consume it therefore raises `clippy::needless_pass_by_value`, and no example or
  test can take `&Ctx`. Feature 116 marks each such transition with a
  function-level `#[expect(clippy::needless_pass_by_value, reason = ...)]`. Fix: let
  the macro accept `&Ctx` and generate the matching call, then remove the expects.
  Found in feature 116.
- **`dual!` requires an `async fn` body even when the body never awaits.** The
  macro in `tests/valtron/async_first_api.rs` calls `$body().await` in both test
  wrappers, so a body with no `.await` raises `clippy::unused_async`. Feature 116
  marks those three bodies with an `#[expect]`. Fix: add a sync-body form of the
  macro, or make the wrapper call the body without `.await` in a sync form.
  Found in feature 116.
- **`clippy::assert_is_empty` suggests code that does not compile for a slice.**
  Its machine-applicable fix rewrites `assert!(!b.is_empty())` to
  `assert_ne!(b.as_slice(), [])`, and the empty array has no element type, so the
  test target fails with `E0282`. Feature 116 uses a typed empty slice
  (`&[] as &[u8]`) instead. This is an upstream clippy limitation; record it so a
  later automated fix does not reintroduce the failing suggestion.
- **`foundation_core` hides the pedantic lint group at crate level.**
  `src/lib.rs` starts with `#![allow(clippy::pedantic)]`,
  `#![allow(clippy::too_many_lines)]`, `#![allow(clippy::cognitive_complexity)]`
  and `#![allow(clippy::type_complexity)]` (commit `f6916c465`, 2026-03-26).
  Feature 116's zero count is measured with these allows in place, so the
  library's pedantic debt is not visible. Fix: remove the blanket allows, fix or
  carry one `#[expect]` per remaining pedantic lint, then keep the two clippy
  runs green. Found in feature 116. Scheduled as feature 118.
- **The idle CPU tests measure the threads of other tests.** File:
  `backends/foundation_core/tests/valtron/readiness_deadline_tests.rs:300-400`. Problem: the
  tests read `/proc/self/task` and keep every thread whose name ends in `valtron::readin`, which
  is the 15-character truncation of the module path. Pool worker threads inherit the name of the
  thread that starts them, so the workers of another test in the same binary match too. A full
  run under load showed 9 wake-ups and 1.8 ms of CPU on threads of another test, and the bound of
  2 ms then failed. Likely fix: name each pool's workers after the pool (or the test), then match
  that name, or compare the two phases of one test only, as
  `worker_with_event_driven_sleeper_uses_less_cpu_than_a_polling_one` does.
- **A spurious readiness wake costs no poll turn, so a test cannot observe
  it.** File: `backends/foundation_core/src/valtron/executors/local.rs`
  (`Sleepable::is_ready`) and
  `backends/foundation_connectrpc/tests/h2_pump_parking_tests.rs`. Problem: the
  executor gates a woken sleeper on `(park.take_woken() || due) &&
  signal.is_ready(dur)`. A notifier that fires while its readiness stays false
  wakes the executor thread but never re-polls the task. The H2 pump's old
  `ReceiverGone` adapter therefore measured the same 3 pump turns as the new
  close-only signal, and the feature's turn-count test cannot fail on the old
  adapter. The removed cost is the thread interrupt, and no public counter
  shows it. Likely fix: add a `#[doc(hidden)]` wake counter beside
  `h2_pump_poll_count` (on `ReadinessNotifier` or `ExecutorWake`), then assert
  that a pop does not wake a parked producer. Found in feature 126.
- **A blocking call inside a task poll starves the pool silently.** Files:
  `backends/foundation_core/src/valtron/executors/future_task.rs:268,325,398,437,841,935`
  and `state_task.rs:171,224` (the poll sites; none measures the poll);
  `backends/foundation_connectrpc/tests/client_deadline_tests.rs` (the case that
  exposed it). Problem: `thread::join()`, a blocking socket read or a blocking
  file call inside an async body holds the valtron worker that polls the body.
  With a 2-worker pool the H2 pump then never runs. CI failed
  `a_timed_out_receive_cancels_the_call` twice; a local pool with
  `threads = 1` and `--test-threads=2` failed 3 of 3 runs, and `threads = 2`
  passed. Each test now waits with a bounded async poll, but nothing reports
  the next one. Fix: a `PoolConfig::slow_poll_warning` threshold that names the
  task and the duration, on by default in the test harness, plus the rule in
  the valtron skill. Found in feature 131; scheduled as feature 139.

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

## 5b. foundation_reactor

- **The crate is not clean under `cargo clippy -- -D warnings` with the pedantic
  group.** `cargo clippy --profile uat -p foundation_reactor --all-targets
  --no-deps -- -D warnings` reports 120 errors, mostly
  `clippy::must_use_candidate`, `clippy::doc_markdown`,
  `clippy::missing_errors_doc` and `clippy::cast_lossless`, in
  `reactor.rs`, `fd_readiness.rs`, `ready.rs`, `shutdown_signal.rs` and
  `poll/**`. HEAD and `master` report the same count, so feature 122 added none.
  `ci/packages.toml` does not set `clippy-deny-warnings` for the package, which
  is why CI reports them instead of failing. Fix: clear the crate, then add the
  package to the `clippy-deny-warnings` set. Found in feature 122.
- **The kqueue read-edge suppression is a latent lost wake for every socket user.**
  File `src/poll/sys/unix/selector/kqueue.rs:458-489`. macOS reports `EVFILT_READ`
  level-triggered for pipes despite `EV_CLEAR`, so the selector delivers a readable
  event only when the fd's byte count grows, and re-arms only through
  `Reactor::clear` → `Poll::rearm_readable`. A socket that drains N bytes and then
  receives a smaller frame (M < N) produces no new edge, so a task that forgot
  `FdReadiness::clear` before it parked never wakes. Feature 129 fixed the one such
  caller in foundation_netio; the rule stays latent for every other readiness user,
  and the clear-before-register protocol is easy to miss. Fix: re-arm the read edge
  inside `FdReadiness::register_waker` and `EventReadiness::register` so no caller
  can lose a later edge, keep `clear` for the latched flags, and add a selector test
  that drains N bytes then delivers M < N with no explicit re-arm. Found in feature
  129; scheduled as feature 136.

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
- **`rpc_proxy_e2e_tests::connect_server_stream_via_h2_proxy` stalled on Linux
  and failed on its 5 s deadline.** GitHub run 42154487867, head `d9cf916ec`, job
  `foundation_proxy`, 2026-11-06T03:57:51Z. The proxy logged
  `Get intro from stream - got: Intro(OK, HTTP11, Some("Ok"))` and then nothing
  for 5 s. The client panicked at `tests/rpc_proxy_e2e_tests.rs:349` with
  `open server stream: Error: [deadline_exceeded] call deadline exceeded`. Both
  later runs of the branch were cancelled, so the failure was never retested.
  Fix: retest on the current head, then, if it recurs, trace the response-head
  path from the proxy's H1 leg to the client's H2 pump. Found in feature 121.
  It recurred on macOS under load. Feature 130 ran the full proxy suite 20 times
  on 2026-11-06 with two effective CPUs (`--test-threads=2` plus eight CPU
  burners on a 10-CPU host). Ten runs failed, each in
  `tests/rpc_proxy_e2e_tests.rs`: `grpc_web_server_stream_via_proxy` (4),
  `grpc_web_server_stream_via_grpc_backend_carries_status` (3),
  `connect_server_stream_via_grpc_backend` (3),
  `connect_client_stream_via_grpc_backend` (3) and
  `connect_server_stream_via_h1_proxy` (1). Each hit `CALL_DEADLINE` = 5 s with
  `[deadline_exceeded]`, after `[DEPENDS] Task Entry { id: 1, gen: 2 } returned
  already-ready signal (violation 1/4096)`. Feature 121's 30/30 runs in the
  Linux image stayed green, so the stall is macOS-specific or load-dependent
  there. Fix: run feature 121's Design 2 (watchdog and reactor/registry dump) on
  the macOS run and find the park that never wakes. The feature 136 kqueue audit
  is a likely co-fix. Found in features 121 and 130.
- **A panic inside a pool-driven future surfaces as a misleading panic.** File
  `backends/foundation_core/src/valtron/executors/sendables.rs:900-907`
  (`block_on_future` through `sync_collect_one`). When a future that the pool
  polls panics on a worker, the task produces no `Ready`, `sync_collect_one`
  returns `None`, and `block_on_future` panics with `the future produced no
  result (pool not initialized or torn down early)`. The original panic and its
  payload are lost. Feature 130's run 3 printed the real deadline panic on a
  worker and then this second panic on the `#[valtron_test]` timeout thread
  (`backends/foundation_macros/src/valtron_entry.rs`). The worker catches the
  unwind in `backends/foundation_core/src/valtron/executors/multi/mod.rs:1925`
  and sends `ThreadActivity::Panicked`, but no caller reads the payload. Fix:
  carry the payload to the driving caller, for example as a stream item or a
  wake of the result channel, and `resume_unwind` it in `block_on_future`. Found
  in feature 130.

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

- **The time-source gate skips the vendored `foundation_boringtun` fork.** File:
  `scripts/ci/check_time_sources.py` (the `SKIP_CRATES` set). The fork has 15
  direct reads (`1` wall, `14` monotonic) in `backends/foundation_boringtun/src`.
  The gate skips it because it is a published upstream fork, like
  `check_errstacks.py` does for the error shape. The umbrella feature 117 goal
  ("the allowlist holds only `foundation_compact` and `MonotonicClock`") therefore
  does not cover the fork. Fix: decide whether the fork keeps upstream's clock
  calls on purpose, or route them through a host hook. Found in feature 117.

- **The `Free runner disk` step stalls and leaves the run `in_progress`.** The step runs
  `find /runner-free -mindepth 2 -maxdepth 2 -exec rm -rf {} +`
  (`.github/workflows/check.yaml:121` and `:308`). On 2026-11-05 the
  `foundation_buildtools` job was cancelled at that step (run 42048334017), and in run
  42040473182 the `foundation_wireguard` job stayed at it for over an hour, so the whole
  run never completed. The step deletes whatever the runner image left at depth 2, which
  can be a large tree. Fix: delete only the paths the job needs freed, or bound the step
  with `timeout`, and print the size before the delete. Found on 2026-11-05.
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

- **`foundation_macros` has 62 pedantic clippy diagnostics.** The workspace sets
  `clippy::pedantic = "warn"` and the crate inherits it, but the crate is not in
  `ci/packages.toml`, so CI never lints it. `cargo clippy -p foundation_macros
  --all-targets --no-deps -- -D warnings` fails with 62 errors: 20 missing
  backticks in docs, 6 single-pattern `match`, 4 wildcard matches, 4 by-value
  arguments, 4 similar binding names, 3 `format!` inline arguments, 6
  `too_many_lines`, and smaller groups. Four are in `valtron_entry.rs`; the rest
  span the other macro modules. The manifest also warns that `chrono` is
  unused. Found in feature 115, which left them in place
  because they predate it and are outside its scope. Fix: run the per-crate
  clippy workflow in `.agents/AGENTS.md`, then add the crate to
  `ci/packages.toml` with `clippy-deny-warnings = true`. Scheduled as
  feature 120.
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
- **The ptrace dispatch reads the `open(2)` mode from the flags argument.**
  `flags_to_perm` in `src/native/vfs/ptrace/syscall_dispatch.rs` takes
  `(flags >> 16) & 0o777`, but the mode is a separate syscall argument: `arg2`
  of `open` and `arg3` of `openat`. A created file can therefore get wrong
  permission bits. Fix: pass the mode register to the open path and delete
  `flags_to_perm`. Found in feature 111.
- **The ptrace `dup2`/`dup3` dispatch ignores close-on-exec.** `handle_dup`
  copies the source entry, including its `flags`, so `O_CLOEXEC` survives a
  `dup2`, while `dup2(2)` must clear it. `dup3(2)` must set it from its `flags`
  argument, and the dispatch ignores that argument. Fix: clear `O_CLOEXEC` on a
  `dup`/`dup2` copy and set it from `arg2` for `dup3`. Found in feature 111.
- **The ptrace `dup2`/`dup3` dispatch does not validate the new fd number.**
  `handle_dup` accepts any 64-bit `arg1`, inserts an entry at that number and
  returns it. `dup2(2)` must return `EBADF` when the number is negative or not
  below `RLIMIT_NOFILE`. Fix: read the tracee's `RLIMIT_NOFILE` and reject an
  out-of-range number. Found in feature 111.
- **The ptrace dispatch ignores `O_APPEND`.** `open_virtual_path` in
  `src/native/vfs/ptrace/syscall_dispatch.rs` stores the open flags, but
  `handle_vfs_write` always writes at the tracked offset. A traced `>>`
  redirect therefore overwrites from the start of the file. Fix: when the
  entry has `O_APPEND`, move the offset to the file size before the write.
  Found in feature 111.
- **The ptrace loop ignores `PTRACE_SETREGS` failures.** `ptrace_loop` in
  `src/native/vfs/ptrace/nix_backend.rs` calls `ptrace::setregs(pid, r).ok()`
  for `Skip` and `SkipError`. If the call fails, the tracee sees the kernel's
  own result instead of the emulated result, and the tracer does not report it.
  Fix: return the error, as the `CloseThenSkip` arm does. Found in feature 111.
- **The `vfs-preload` shim `ftruncate` accepts a negative length.** In
  `src/native/vfs/shim/mod.rs`, `ftruncate` converts `length` to `usize` with
  `try_from(length.cast_unsigned()).unwrap_or(usize::MAX)`. A negative length
  therefore becomes a huge size, so the shim tries a huge allocation instead of
  returning `EINVAL`. Fix: reject `length < 0` with `EINVAL` before the resize.
  Found in feature 113.

## Paused

- Windows/macOS VM images (#69).

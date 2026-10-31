# Spec 70 learnings

_What an agent learned that the next agent needs. Newest first._

- 2026-10-31 (CI fix): a version bump must move the whole RustCrypto set together. The
  `pull/163/merge` inherited master's incomplete Dependabot bump of `sha2` from 0.10 to 0.11.
  `sha2` 0.11 uses `digest` 0.11.3; `hmac` 0.12, `pbkdf2` 0.12, and `argon2` 0.5 still use
  `digest` 0.10.7. The `CoreProxy` trait moved between these `digest` versions. The build then
  failed with `Sha256: hmac::digest::core_api::CoreProxy not satisfied`. Fix: revert `sha2` to
  0.10 in the 13 crates the bump touched. Keep `sha2` 0.11.0 in the lock: `rust-embed-utils`
  0.12 needs `^0.11`, reached through `rust-embed` 0.11 and then `age` 0.10 in
  `foundation_keychain`. When a Dependabot bump moves one RustCrypto crate, check
  `sha2`/`hmac`/`pbkdf2`/`argon2` together.

- 2026-10-31 (CI driver): a local run had no kill switch. `ci.py run` called `subprocess.run`
  with no timeout, so a step that looped (a hung test or a deadlocked build) never returned;
  only GitHub's job-level `timeout-minutes` stopped it. Fix: `run_package` computes one
  deadline from the package's `timeout-minutes` and passes the time still left to each command;
  `TimeoutExpired`, or a spent deadline, is fatal for the package. Also pass `--nocapture` to
  every `cargo test`, so a failing or hanging test leaves its own output trace instead of a
  bare `FAILED` line.

- 2026-10-31 (CI fix): a crate feature that enables `dep:<crate>` and also calls into that
  crate's feature-gated API must forward the crate's feature too. `foundation_nativeapis`
  `uring` enabled only `dep:io-uring`; its send path calls `Reactor::submit_send*` and imports
  `foundation_reactor::{send_pool, poll::sys::unix::selector::{bufring, uring_completion}}`,
  all gated behind `foundation_reactor/uring`. Symptom on Linux: "no method named submit_send"
  plus unresolved `bufring`/`uring_completion`/`send_pool`. Fix: `uring =
  ["dep:io-uring", "foundation_reactor/uring"]`.

- 2026-10-31 (CI fix): an optional platform dependency must sit in the right target section.
  `foundation_reactor`'s `io-uring = { version = "0.7", optional = true }` sat inside
  `[target.'cfg(target_os = "windows")'.dependencies]`, so `dep:io-uring` in `uring` enabled
  nothing on Linux. Symptom: "unresolved import io_uring" (9 errors) on Linux. Fix: move it to
  `[target.'cfg(target_os = "linux")'.dependencies]`.

- 2026-10-31 (CI fix): a platform-gated `Event` type must exist for every target a crate
  declares. `poll::event::Event` had no variant for `not(unix, windows)` targets; the shell
  selector (`poll::sys::shell`) imports `Event` and `Events` references it, so the wasm
  `cargo check` failed with "unresolved import". Fix: add a neutral shell `Event` gated
  `#[cfg(not(any(unix, target_os = "windows")))]`, and drop the now-unused `use
  super::super::event::Event;` from the shell selector.

- 2026-10-31 (CI fix): do not assert wall-clock time on a fast in-memory path. The connectrpc
  `oversized_envelope_prefix_is_rejected_from_its_header` test asserted `elapsed < 1s`. The
  reader checks the declared length against `read_max_bytes` on the 5-byte header before the
  completeness check, so the rejection is deterministic; the `contains("resource_exhausted")`
  and `contains("exceeds read_max_bytes")` asserts already pin it (a buffering or truncation
  regression would answer differently). The wall-clock assert flaked under parallel-test CPU
  contention (emulated amd64). Assert the observable outcome, not the elapsed time.

- 2026-10-31 (feature 11): "test counts must not drop" means the runnable count, not the
  source-function count. `foundation_netio/src/native/tls_verification.rs` was never wired in
  (`native/mod.rs` has `// pub mod tls_verification;` commented out), so its five
  `#[cfg(test)]` tests were never compiled or run. Moving them would have been pointless; the
  right action is to remove them and record the dead module. When you compare counts before
  and after, count only tests that actually compiled and ran under the crate's CI feature set.

- 2026-10-31 (feature 11): a test that only asserts a private field (auth `config.*`,
  body-limit `max_bytes`/`error_message`, logger `config.level`) should be rewritten to assert
  public behaviour, not to expose the field. Rewrite through `RequestMiddleware::handle` and
  check the `MiddlewareResult` with `matches!` (the enum has no `Debug`, and `SendSafeBody`
  has no `PartialEq`). Add a real accessor (`LoggerMiddleware::level`) only when the value is
  a genuine part of the API. The private base64 helper's edge cases are covered indirectly by
  a valid-token test and an invalid-token test through `handle`.

- 2026-10-31 (feature 11): `foundation_http` has a flat `tests/` directory and no wasm-bindgen
  test target. Its wasm-gated types (`CfConn`) are plain `#[test]` code, not
  `#[wasm_bindgen_test]`. Move them to `tests/cf_conn_tests.rs` gated by
  `#![cfg(feature = "wasm-bindgen-http")]`. Under default features the binary compiles to zero
  tests, and the CI wasm step is a `cargo check --features wasm` compile-only gate, so those
  tests still never run natively — the same behaviour as before the move.

- 2026-10-30 (feature 33): a sans-IO driver that parks on `Depends` must also be woken for
  **outgoing** data, not only incoming. Feature 28 taught the QUIC driver to park on socket
  read readiness + the next timer, but a stream handle that queues outgoing data (a reset,
  write, finish, stop, close, a datagram, or `read`'s flow-control `finalize`) never woke the
  driver. The result was a flaky integration test: the reset was queued but not transmitted
  until an unrelated socket edge or timer. The fix is a shared `BoolSignal` latch, cleared
  before each pass and composed into the `Depends`, set by every queued-outgoing-data path.
  Key invariant: clearing the latch before the pass can never lose data, because
  `pump_transmit` re-discovers pending transmits each pass and the loss-detection timer already
  covers a transmit the congestion controller cannot send yet.

- 2026-10-30 (feature 33): to test "a parked driver becomes runnable" deterministically, do not
  re-step the driver. Capture the `TaskStatus::Depends(r)` it returns when idle, then assert
  `r.is_ready(None)` flips after the triggering action. `r` is an `AnyReadiness` over the fd
  edge, the deadline, and the self-wake latch, and each child reads a shared atomic, so the
  assertion is free of timing. The manual `next_status()` stepping in the QUIC suite cannot
  expose the bug on its own: re-stepping always re-polls, so the driver would transmit the
  queued data anyway.

- 2026-10-30 (feature 32): a config value that must reach a **process-global singleton** cannot
  be tested for round-trip inside the crate's shared `tests/mod.rs` binary. In that binary the
  `shutdown_signal` wake test calls `Reactor::get()` and wins the first-call, so a later
  `init_with_config(…, 250 ms)` returns the existing 100 ms instance and the assertion fails.
  Fix: give the singleton-touching test its own test target (`[[test]] name = "config" path =
  "tests/config/mod.rs"`), so it runs in its own process. This mirrors how
  `foundation_nativeapis` isolates its reactor tests, and keeps the value-observing assertion
  deterministic.

- 2026-10-30 (feature 32): rule-8 fields on a `OnceLock` singleton need a new entry point, not a
  signature change. The reactor's `get()`/`init()` are called across the workspace, so they keep
  their signatures and delegate to a new `init_with_config(preference, config)`; `init_inner`
  gains the config parameter. First-call-wins means a later `init_with_config` ignores its config
  — the same semantics `init` already has for its backend preference, so the refactor is
  behaviour-preserving.

- 2026-10-30 (feature 31): changing the public `serve`/`serve_with_listener`/`serve_h3`
  shutdown-parameter type is a whole-workspace change, not a single-crate one. The feature's R4
  scoped it to `foundation_http` + `foundation_browser`, but the signature change cascaded to
  ~30 files across `foundation_connectrpc`, `foundation_auth`, `foundation_proxy`,
  `foundation_keychain`, `foundation_deployment_docker`, and `foundation_testbed`. The compiler
  is the authoritative scope, not the spec's file list: fix the `E0308` `expected
  &Arc<ShutdownSignal>, found &Arc<OnSignal>` errors one crate at a time, and do not rename
  `OnSignal` where it never reaches `serve*` (proxy `passthrough.rs`, the valtron kill signal).

- 2026-10-30 (feature 31): `ShutdownSignal` is `Send + Sync` even though it stores
  `OnceLock<Arc<Reactor>>` and `Reactor` owns a `JoinHandle`. `Arc<Reactor>: Sync` is already
  proven by the crate's `static REACTOR: OnceLock<Arc<Reactor>>` compiling, so the reactor wake
  link does not break `Arc<ShutdownSignal>` sharing across the accept-loop threads.

- 2026-10-30 (feature 31): a flat `tests/foo_tests.rs` next to `tests/mod.rs` is compiled twice,
  because Cargo auto-discovers both as test targets. Put the test file in a subdirectory and
  register it from `tests/mod.rs` (`mod foo;` → `tests/foo/mod.rs` → `mod foo_tests;`), matching
  the `foundation_core` layout, so it runs once.

- 2026-10-30 (feature 31): the wake test must not assume the drain thread is silent. With no fds
  registered, the drain loop bumps `events_gen` only when `latched > 0`, so a reactor with no fds
  stays at a stable generation and the parked thread is woken only by `turn_on`'s
  `wake_waiters`.

- 2026-10-30 (feature 30): a `pub(crate)` config field is not observable from an integration
  test, and a builder-only field (no getter) is not either. T4 then splits: a field with a
  public getter or a public field gets a round-trip test (build with a non-default value, read
  it back); a field without one gets a default-value assertion against the named `DEFAULT_*`
  const and a comment saying why. R6 (`delivery_capacity`) is `pub(crate)` and is consumed by
  the `sequenced` delivery terminal before the queue capacity is hidden; R1
  (`readiness_poll_quantum`) and R7 (`wait_check_interval`) drive `block_on` internals with no
  getter. None is cheaply observable, so all three assert the documented default.

- 2026-10-30 (feature 30): a `self` in a `macro_rules!` argument resolves in the macro's
  module scope, not the caller's, so passing `self.config.delivery_capacity` as a
  `$capacity:expr` metavariable fails with "a macro invocation can only access identifiers it
  receives from parameters". Write `self` inside the macro body and pass only a literal
  (`$bounded:literal`) that chooses between the bounded and unbounded arms.

- 2026-10-30 (feature 30): R8's "review" resolved to keep the consts, not move them. A config
  field for `wait_cycle`/`park_duration` would be a second, redundant knob over the existing
  optional parameter (a `Some` already overrides the default). Rule 8 is still met by routing
  the `unwrap_or` fallback through `StreamConfig::default()` so the const is referenced only
  by the config's `Default` impl, not by the code path.

- 2026-10-30 (feature 27): `SleepIterator::next` returned `Pending(0 ns)` when `completed_at == now`,
  because `Instant::checked_duration_since` returns `Some(Duration::ZERO)` at equality, not `None`.
  The `Some(diff) => Pending` arm then reported a reached deadline as still pending, which flaked
  `zero_duration_sleep_iterator_finishes_immediately` under load. Fix: extract the classification
  into a pure function `remaining(completed_at: Option<Instant>, now: Instant) -> Option<Duration>`
  that returns `Some(diff)` only when `diff > Duration::ZERO` (past, exactly-now and overflow all
  give `None` = done), and have `next` match on it. The pure function is `pub` so the boundary cases
  are tested from `tests/` without `Instant::now` advancing (rule 7). The exact-boundary test
  (`Some(now)`, `Some(now ± 1 ms)`, `None`) is deterministic. 20/20 in both feature sets; CI green.

- 2026-10-30 (bug-backlog §3): `WaitingForBody` must arm the connection's read waker **before**
  probing, then park on `AnyReadiness::either(signal, DeadlinePassed(deadline))`. The order is the
  fix for both halves of the old race: arming first means a body that lands while we probe still
  wakes the parked task (the h2 handler's `arm_wakers`-before-read order), and the deadline keeps a
  hung client from holding the connection forever (the overlay's 100 Continue + body round trip is
  up to ~400 ms, longer than the old 50 ms × 5 ≈ 155 ms fixed-delay budget). The deadline is a new
  config field, `ExpectContinueConfig.body_timeout` (3 s), not a constant (rule 8). A blocking
  transport returns `false` from `register_read_waker`, so the arm is a cheap no-op and it keeps the
  old `base_delay`/`max_attempts` countdown — the overlay path is deadline-driven (`deadline:
  Option<Instant>` in the state), the blocking path is attempt-driven, and the two never mix. The
  `overlay_connector_http_round_trip` test (spec-55 F11, `Expect: 100-continue` enabled) passed
  20/20 after the fix.

- 2026-10-30 (feature 28): a `TaskStatus::Depends` sleeper has no deadline of its own, so a task
  parked purely on an fd readiness signal never wakes for timers. The QUIC driver must compose
  the socket signal with a deadline signal — `AnyReadiness::either(fd, DeadlinePassed(next_timeout))`
  — or loss detection, idle timeout and keep-alive silently stall on a lossy link. `next_timeout`
  is the min `quinn_proto::Connection::poll_timeout()` across connections (which takes `&mut self`,
  so the lock guard must be `mut`).

- 2026-10-30 (feature 28): do not measure parking with process-wide `getrusage(RUSAGE_SELF)`; the
  full ~1100-test `quic` run runs every test in one process, so a sibling's CPU contaminates a 1 s
  idle window. Count `next_status` calls instead (`CountingDriver` wrapper): a parked driver stays
  at ~0 polls/s, a 1 ms poller fires ~1000/s, so `count < 20` over 1 s is a deterministic, load-proof
  assertion. Complement it with a direct `matches!(next_status(..), TaskStatus::Depends(_))` test.

- 2026-10-30 (feature 10): `#[valtron_test]` emits its own `#[test]`, so two of them cannot
  sit on one function — the name would collide. Its `seed`/`threads`/`timeout` args are parsed
  as `syn::Expr` and emitted as `(#expr)`, so a `macro_rules!` macro can emit the `threads = 4`
  and `threads = 8` variants from one `$body:block` with `seed = $seed + 1000`. That is how the
  8-thread conformance variants share a body without copying it.

- 2026-10-30 (feature 10): doctests can use the crate's regular dependencies (the netio
  examples call `serde_json::from_str` and `serde::Deserialize`), but a generic enum's default
  type parameters do not apply to a variant written by its value path — `FetchPending::Connecting
  { .. }` needs `FetchPending<&str, String>`. And `rustls::ClientConfig::builder()` panics without
  a global default provider, so a doc example that builds a rustls config must use
  `default_client_config()` (which uses `builder_with_provider`) instead.

- 2026-10-30 (feature 10): the three formerly-ignored netio test files are hermetic now: they
  bind loopback sockets and, for TLS, generate a CA and server cert at test start (`rcgen`) and
  trust the CA in the client config. They live in the `tcp_and_tls` module of the combined
  `tests/mod.rs` binary, so the 20-run flake check filters with `cargo test -p foundation_netio --
  tcp_and_tls` (libtest matches the `tcp_and_tls::` path prefix).

- 2026-10-30 (feature 09): a config field read in only one place can look correct while
  another code path ignores it. `max_yield_duration` capped the idle re-check in `run_until`
  (the single-threaded executor) but `block_on`'s multi-worker `Wait` arm still slept
  `no_work_yield`, so a pool built with a short `max_yield_duration` did not re-poll a `Wait`
  task any faster under `multi`. Fix: `yield_for(no_work_yield.min(max_yield_duration))`. When
  adding a config knob, grep every consumer of the old constant and wire each one.

- 2026-10-30 (feature 09): tracing-subscriber 0.3.23 has no blanket `Layer` impl for
  `Arc<L>` (only `Box<L>`). A log-capture test that reads captured lines after the fact must
  put the `Arc<Mutex<Vec<..>>>` *inside* the layer struct and pass the layer by value into
  `tracing_subscriber::registry().with(layer)`; keep a separate `Arc` clone for the
  assertions. A `#[valtron_test]` already sets a global subscriber, so use the thread-local
  `tracing::subscriber::with_default` to avoid the `#[traced_test]` "global default set"
  panic.

- 2026-10-30 (feature 09): the §3 `rg` check does not go green by itself. L1-L11 moved the
  named constants onto configs, but the same pattern still returns hard-coded limits that
  were never in scope: `READINESS_POLL_QUANTUM` (1 ms), `DEFAULT_OP_READ_TIME` (100 ms),
  `DEFAULT_NOTIFY_QUEUE_WAIT_TIMEOUT`, `DEFAULT_NOTIFY_QUEUE_MAX_SPINS`,
  `DEFAULT_READINESS_WAIT` (10 ms), `DEFAULT_DELIVERY_CAPACITY` (8) and
  `JS_WAIT_CHECK_INTERVAL` (4 ms). These are a new backlog entry and the seed for a follow-on
  feature. `DEFAULT_PARK_DURATION`/`DEFAULT_WAIT_CYCLE` are `unwrap_or` defaults for optional
  parameters, so they need review, not necessarily a config field.

- 2026-10-30 (feature 09): `SpinWaiter` calibration is what makes the sub-millisecond test
  slow, not the wait itself. `SpinWaiter::new(10_000_000)` (10 M spins/ms) made 100 × 500 µs
  waits take ~5.6 s in the unoptimized `uat` profile; `SpinWaiter::embedded()` (1 M/ms) is
  ~0.56 s and still ~11× above the 500 µs lower bound, so no custom calibration is needed.

- 2026-10-30 (feature 08): `H1Body`/`H1Sink` are polled inside the `ConnectionResult::Async`
  future, which the connection task drives under a native executor (tokio/smol), not as a valtron
  task. So their socket wait must register a `std::task::Waker` (`FdReadiness::register_waker` for a
  kernel fd, a one-shot `ConnWaker` for the overlay), not valtron's `EventReadiness::is_ready`
  (which only answers "ready now"). This is the same waker-capable seam feature 29 built for the
  client body. `ConnIo` (connectrpc) was rebuilt on `ConnReadiness` + `FdReadiness`; `block_on` and
  its `Wake`/wake-socket driver are gone.

- 2026-10-30 (feature 08): a hangup watcher cannot treat `READABLE` as "the peer hung up": a
  pipelined request also makes the fd readable. `FdReadiness::ready()` was added so the watcher can
  read the reactor's latched `Ready` bits and close only on `READ_CLOSED`/`WRITE_CLOSED`/`ERROR`.
  Without it, the `Async` future would cancel a live call as soon as the next pipelined request
  arrived.

- 2026-10-30 (feature 08): `dispatch_to` had to take `sink: Box<dyn ResponseSink>` (owned), not
  `&mut sink`, so the future it returns is `'static`. A borrowed sink ties the future to the borrow
  inside `serve`'s `async move` block and it cannot be boxed into `ServeFuture`. The handler owns
  the sink and the `H1Sink` owns a cloned `SharedByteBufferStream` plus `Arc<ConnIo>`.

- 2026-10-30 (feature 08): `futures::future::select` needs both sides `Unpin`. `BoxFuture`
  (`Pin<Box<dyn Future>>`) is `Unpin`, but a plain `async move` block is not, so the `hangup` side
  is `Box::pin`-ed. The deadline uses valtron's `timer::sleep_until` (`timer::Sleep`), a
  process-wide timer thread that parks without holding a worker — essential to the
  `h1_streaming_calls_do_not_exhaust_the_pool` test's correctness.

- 2026-10-30 (feature 08): the repo's committed code is NOT `rustfmt`-clean (hand-aligned). Running
  `cargo fmt -p <crate>` reformats ~140 untouched files. Format only the specific files changed
  (`cargo fmt -- <files>`), or skip `fmt` and match the surrounding hand-alignment by eye.

- 2026-10-29 (feature 07): `foundation_reactor` (moved out of netio in feature 29) has no
  `poll/event` backend for `wasm32-unknown-unknown` (`Event` is linux/kqueue/windows only), yet
  netio declared it as an unconditional dependency. That broke the netio wasm gate
  (`cargo check --target wasm32 --no-default-features --features wasm-fetch`), even though the only
  use is `native/raw_stream.rs` (a `#[cfg(not(target_family = "wasm"))]` module). Fix: move the
  dependency into `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`. Any netio dependency
  used only by a `native` module must be target-gated the same way, or the wasm build fails first.

- 2026-10-29 (feature 07): a wasm SSE read error and a clean end both set `done`, so the consumer
  cannot tell them apart. The error must travel in a slot next to `done`
  (`Arc<Mutex<Option<String>>>`), and `QueueSseEvents` must check that slot at every end path
  (`Ok(None)` drained and `WouldBlock` drained), not just the parser-error arm. `end_or_fail` takes
  the slot (`.take()`) so the `Failed` progress is emitted exactly once, then the `failed` flag makes
  the next poll return `None`. A broken feed never recovers, so waiting on it would stall the
  consumer forever.

- 2026-10-29 (feature 07): a WebTransport-over-HTTP/3 session lives exactly as long as its CONNECT
  stream (draft-ietf-webtrans-http3 §5 "Session Termination"): a FIN, a reset, or a
  `CLOSE_WEBTRANSPORT_SESSION` capsule ends it. Accept must therefore keep the CONNECT stream open
  after the 200 and hand it to `WtSession` (`WtAccept::Session { connect }`), not `poll_finish` it.
  `WtSession::poll_connect` drains buffered capsules first, then reads; a clean FIN and a reset both
  map to `Closed` → `Ok(true)`. `close` sends the close capsule and then FINs the stream.

- 2026-10-29 (feature 07): the DATA-frame cap must apply on the length alone, before allocating the
  payload. `FrameDecoder` reads the VarInt length, and if it exceeds `max_frame_size`, rejects with
  `H3_FRAME_ERROR` (`0x0108`) rather than growing a buffer to that size. Streaming within a frame is
  a `State::Data { remaining }` that yields `read_chunk`-sized pieces from `AccumulatingBuffer`
  without buffering the whole frame; `has_partial()` must include `State::Data` so a paused read
  is not lost.


- 2026-10-29 (feature 06): `conn_io` blocks its own thread, so it cannot reuse
  `EventReadiness::is_ready`, which answers "ready now" without blocking. It got a
  purpose-built `ConnReadiness` (`foundation_netio::native::connection`):
  `Fd(fd, Option<Arc<dyn TlsInterest>>)` for kernel sockets and
  `Overlay(Box<dyn OverlayReadWrite>)` for the WireGuard overlay.
  `TlsInterest::corrected` maps the application's direction to the wire direction
  the TLS session needs (`wants_write` first, then "still handshaking"), mirroring
  the async `RustlsStream::readiness`. The overlay is woken by one-shot `ConnWaker`
  callbacks that write a byte to the driver's wake pair, not by `poll(2)`.

- 2026-10-29 (feature 06): `OverlayStream` derived `Clone` but its `Drop` removed
  its smoltcp socket from the shared `SocketSet`, so `conn_readiness` cloning the
  overlay stream (via `OverlayReadWrite::clone_box`) removed the socket twice. The
  second remove panics ("handle does not refer to a valid socket") and corrupts the
  connection. Fix: socket removal is now ref-counted through an `OverlayStreamOwner`
  `Arc`; clones share it and the socket is removed once, on the last drop.
  `OverlayStream` itself has no `Drop`.

- 2026-10-29 (feature 06): the overlay's `readable()`/`writable()` probe the full
  predicate, not just "buffered data": `readable` = `can_recv || (!may_recv &&
  !still_connecting)` (data or EOF), `writable` = `can_send || !may_send` (space,
  or the error surfaces). This lets `conn_io` answer "ready now" without consuming
  a byte.

- 2026-10-29 (feature 06): `HttpClient::open_exchange` built a fresh
  `HttpConnectionPool` with `SystemDnsResolver`, silently dropping a connector set
  with `with_connector` (WireGuard overlay, SSH dial-stdio). The async exchange
  path now reuses `self.pool` (falling back to a fresh pool from `self.resolver`),
  matching `send`/`send_async`.

- 2026-10-29 (feature 06): the foundation_http server's `Expect: 100-continue`
  `WaitingForBody` retries `probe()` on fixed delays (30 ms) and 408s after
  ~120 ms, before the overlay's mesh delivers the body (up to 200 ms). This is out
  of feature 06's crate list, so the overlay RPC test sets `expect_continue(false)`
  and the race is tracked in the backlog. It is a real bug: `WaitingForBody` must
  park on the read wake source, not fixed delays. Fixed 2026-10-30 on
  `fix/io-01-valtron-parking` (see the bug-backlog §3 entry above).

- 2026-10-28 (feature 05): dropping the pipe producer is a graceful end, not an error. The
  spec's `pushed_body_producer_drop_fails_the_request` assumed a dropped producer errors the
  request, but the code path uses `PipeSender`; dropping it calls `close_queue()` = a close
  event, which the pipe delivers as a clean end-of-body (zero-length chunk). The test is
  therefore renamed `pushed_body_producer_drop_ends_the_body` and asserts a completed request
  with the pushed bytes, not a failure. Only a mid-stream body *error* (not a drop) fails the
  request.

- 2026-10-28 (feature 05): `body_readiness: None` busy-spins as `State::Pending(None)`. A
  producer that pushes nothing yet (empty pipe) must park with a real readiness source, not
  `Pending(None)`, which valtron treats as "retry at once" and the old `producer_stalls`
  back-off was masking. The fix registers the pipe's wake source first, repolls once, then
  parks with `TaskStatus::Depends(readiness)`; a push or a close wakes it.

- 2026-10-28 (feature 05): the accept loop's shutdown cannot "wake at once" from `OnSignal`.
  `OnSignal` (`foundation_core::synca::signals.rs`) is a bare `Arc<AtomicUsize>` with only
  `probe`/`turn_on`/`turn_off` — no fd write and no condvar notify. `FdReadiness::is_ready(Some(..))`
  is also non-blocking (`SharedReadiness::is_ready` ignores the `dur` and only checks the
  reactor's latched token). So neither spec option (§2.2: block in `is_ready(Some(timeout))`,
  nor `poll(2)` on a shutdown-written wake fd) is reachable without changing those types. The
  accept loop instead snapshots `events_generation()`, runs a non-blocking `accept`, then
  `wait_for_events(gen, accept_shutdown_poll)` — the only blocking primitive. A connection
  wakes it at once (a reactor event advances the generation); shutdown is detected within
  `accept_shutdown_poll` (10 ms default), a poll rather than a literal wake. Recorded as a
  backlog entry: add a purpose-built wake-capable shutdown signal (do not overload `OnSignal`).

- 2026-10-28 (feature 04): `connect_completion` in `foundation_iogate`
  (`src/native/accept.rs:140`) was read and **not** reused. Two reasons. First, it calls the
  blocking `TcpStream::connect(addr)` (line 141) and only sets `O_NONBLOCK` *after* the
  connect finishes, so it holds the worker for the exact reason feature 04 exists. Second,
  iogate sits *above* netio in the dependency graph (`iogate → {netio, nativeapis}`), so netio
  cannot call it at all — the same cycle that feature 03/29 hit. Feature 04 therefore uses
  `PendingConnect::start(addr)` (socket2) directly inside netio, which creates the socket
  non-blocking *before* `connect(2)`.

- 2026-10-28 (feature 04): the work landed in `request_redirect.rs`, not the spec's
  `request_intro.rs` (§2.3). The redirect task is the real HTTP/1.1 connection opener (it has
  the checkout/resolve/connect sequence and the `Connecting`/`TlsHandshake` states); the
  `request_intro.rs` calls in §1 are only the sync constructors' boundary (`SystemDnsResolver`
  + `HttpClientConnection::connect`), which stay blocking on purpose. Branch note: the work is
  on `fix/io-01-valtron-parking`, not the spec frontmatter's `fix/io-04-nonblocking-connect-tls`.

- 2026-10-28 (feature 04): the connect and the TLS handshake park a valtron task with
  `TaskStatus::Depends(SocketReadiness)`, not a `std::task::Waker`. That is the right tool
  here: the valtron task itself is parking, and `Depends` + a deadline is exactly what it
  needs. The `Waker`-capable `FdReadiness` from feature 03/29 is only required when a
  `futures_core::Stream` (an async body consumer) must be woken across the task boundary; the
  connect/handshake never cross it. `SocketReadiness::poll_now()` is a zero-timeout `poll(2)`
  used only to test "writable now?" before parking.

- 2026-10-28 (feature 04): the TLS handshake uses `RustlsStream::drive_handshake()`, which
  loops `conn.complete_io(sock)` until `is_handshaking()` is false, `WouldBlock`, or a real
  error. On `WouldBlock` the task parks on `readiness()`, which picks the direction from the
  TLS session (`wants_write` first, then `is_handshaking` → readable, then the caller's
  interest). This matters because a plaintext *read* can stall on TLS output and a plaintext
  *write* can stall waiting for the server's reply; parking on the wrong direction either
  never wakes or spins. The handshake must be its own step (not driven by the first request
  write) so its wait uses the connect deadline, not the body's idle deadline.

- 2026-10-28 (feature 29): do not measure parking with process-wide `getrusage(RUSAGE_SELF)`.
  `#[valtron_test]` gives each test a fresh pool and serializes pool use process-wide, so a
  parked body leaves the polling worker thread almost idle. Measure that thread's CPU inside
  the stream's `poll_next` with `clock_gettime(CLOCK_THREAD_CPUTIME_ID)` (first poll = baseline,
  last poll = end). Process-wide CPU sums every concurrent test's CPU and flaked the chunked-body
  test at 81 ms under a full parallel run while it passed at 0 ms in isolation. The H3
  `idle_h3_connection_uses_no_cpu` test still uses process-wide CPU and fails the same way under
  load (1588 ms at 1 s) but passes in isolation — pre-existing, feature 02/28, not this feature.

- 2026-10-28 (feature 29): the body's wake source has three transport cases, and
  `RawStream::supports_nonblocking_io()` is the clean discriminator. Overlay: register a
  one-shot `ConnWaker` via `set_read_waker`. Non-blocking kernel socket (plain TCP/Unix, rustls
  over those): `FdReadiness::register(fd)` then `register_waker(&Waker)`. Blocking socket and
  io_uring completion socket: return `None` — a blocking `WouldBlock` is a timeout (a failure),
  not a wait, and a completion socket reads from an inbox, not fd readiness. Register-then-check:
  `AsyncSendSafeBody::poll_next` registers the waker, pulls the iterator once more, then returns
  `Pending`, so no wake is lost between the first pull and the registration.

- 2026-10-28 (feature 03/29): `foundation_netio` cannot depend on `foundation_iogate` or
  `foundation_nativeapis`. The graph is `nativeapis → db → netio` and `iogate → {netio,
  nativeapis}`, so iogate sits above netio on purpose. The client body reader therefore cannot
  reach `FdReadiness` (iogate) even though the connection is right there where the body is
  built (`impls.rs` `HttpReadState::Body`, `api.rs` `send_async`). The connection's
  `readiness()` for `Tcp`/`Unix` is `SocketReadiness` (`connection.rs` around line 1170), a
  zero-timeout `poll(2)` `EventReadiness` — it parks a valtron task via `Depends` but cannot
  wake a `std::task::Waker`. Only `Overlay` and HTTP/3 register a real waker inside netio. So a
  plain-TCP `futures_core::Stream` that returns `Pending` has no in-netio way to be woken.
  Feature 03 §3.2's premise (the connection "already has the shared reactor") is wrong for
  kernel sockets; see feature 29 for the bridge.

- 2026-10-28 (feature 02): an idle TCP socket is always writable. If an fd is registered with
  both READABLE and WRITABLE, the kqueue `EVFILT_WRITE` filter fires forever and the reactor
  drain thread busy-spins. `FdReadiness` therefore registers READABLE only, and callers turn
  WRITABLE on only while they have output queued (`set_writable`). The h2 handler calls
  `set_writable(conn.has_pending_writes())` in `park_or`.

- 2026-10-28 (feature 02): the CPU test exposed a pre-existing valtron executor busy-spin. A
  worker that hosts only a readiness-parked task (`Depends` with no deadline) returns
  `ProgressIndicator::CanProgress(None)` from `schedule_and_do_work` every loop (empty processing
  queue + a sleeping task). `LocalThreadExecutor::block_on` treated `CanProgress` as "keep going"
  with no yield, so the worker spun at 100% CPU. The `run_until` loop already guards this
  (`CanProgress` + `has_runnable_work()`), but `block_on` did not. Fix: in `block_on`'s
  `CanProgress` arm, sleep `READINESS_POLL_QUANTUM` (1 ms) when `!has_runnable_work()` and
  `number_of_sleepers() > 0`. The H3 driver did not hit this because it returns `Delayed(1 ms)`,
  a `Timable` sleeper, which the executor reports as `SpinWait` (a real condvar sleep).

- 2026-10-28 (feature 02): the `QuicDriver` already has `with_fd_readiness`, but nothing calls
  it, and `EventReadiness` has no `clear`, so the driver cannot clear edge-triggered readiness
  before `pump_recv`. Tracked as feature 28.

Feature 01 (valtron parking) is on ewe-studios/ewe_platform#163. The two blocking findings and
the minor finding from review are now fixed on `fix/io-01-valtron-parking`, each with a test:

- `TaskStatus::from_stream` and the async stream lenses no longer register a callback per poll.
  They announce an async consumer with the new idempotent `WakeableStream::announce` (no callback)
  before the first pull; the real wake is still registered only when a pull comes back empty.
- A producer waiting on a full queue now replaces its space wake with `WakeRegistry::replace`
  (clear then push one), so stale callbacks no longer pile up across retries of a full push.
- `NotifyQueue::pop` gates `space.fire()` on `WakeRegistry::has_waiters()`, so a pop with no
  waiting producer takes no wake lock.

`announce` is forwarded by every forwarding combinator and by `DrivenStreamIterator` (multi);
buffering combinators keep the default no-op. Three new tests in `tests/valtron/wakeable_stream.rs`
pin each fix: `from_stream_does_not_accumulate_noop_wakes`, `register_space_wake_replaces_not_accumulates`,
and `pop_wakes_a_waiting_producer`.

What the earlier work fixed: the smol/tokio hang (default features only, `DrivenStreamIterator::wake`),
`ThreadedIterFuture` parking, and the first empty read no longer blocking.

The agent also recorded one older limitation in the backlog: a task that stays parked for a long
time can still leave an async consumer with nothing to wake it. The old code had the same problem.

- 2026-10-28 (feature 01 review fixes): the wake registry now keeps a `pending: AtomicUsize`
  count updated under the same mutex as the callback vec, so `has_waiters`/`pending_len` never
  disagree with the list. `register` and `replace` both update the count while holding the lock;
  `fire` stores 0. `replace` is `clear` + `push`, which is what stops a full-queue producer from
  accumulating stale callbacks. `announce` is the idempotent, no-callback twin of `wake`: it is
  forwarded wherever `wake` is forwarded, and left as a no-op on buffering combinators.
- 2026-10-28 (feature 01): run valtron tests in BOTH feature sets: `--features multi` and the
  default (no `multi`). The `smol/tokio_drives_parked_future_task` hang only shows without
  `multi`. Cause: the single-thread `DrivenStreamIterator` (`executors/non_sendables.rs`) drives the
  engine inside `next`. Priming it (registering a wake before the first pull) switched the inner
  `NotifyQueueStreamIterator` to non-blocking mode, so an empty pull answered `Wait`; the consumer
  then parked on a queue wake. But a parked task is made runnable by a foreign thread flipping a
  readiness signal, which pushes nothing and fires no queue wake, and nobody called `next` again.
  Evidence: a trace showed two engine drives within 30 us, then silence while the 30 ms thread
  fired. Fix: that `wake` registers on the queue only and leaves the inner blocking. The deeper
  fix (readiness sources that can register a wake) is in `docs/bug-backlog.md` section 4.
- 2026-10-28 (feature 01): `ThreadedIterFuture` cannot have a full queue when the future's `Err` is
  pushed (nothing is pushed before it; the smallest queue holds 2), so the old `let _ = push(Err)`
  could not lose the error in practice; the error test pins delivery only.

- 2026-10-28 (from the PR #160 work): `H2Connection::flush_write` used `write_all`, so a
  `WouldBlock` part-way re-sent the already-written prefix and corrupted the HTTP/2 frame
  stream. Any non-blocking writer must track how much it wrote.
- 2026-10-28: the shared reactor's drain thread must request an executor resume after it latches
  events, or parked tasks wait for the idle back-off (up to 100 ms).


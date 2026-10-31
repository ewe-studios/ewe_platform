# Spec 70 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

- 2026-10-31: feature 11 done on `fix/io-01-valtron-parking`. Every `#[cfg(test)]` block in
  the three crate source trees is gone (rule 7). foundation_netio moved its tests per area
  (http/tasks, native, shared, event_source, websocket, wasm). foundation_http moved the five
  middleware suites and the `CfConn` wasm suite. foundation_connectrpc moved the wasm-fetch
  capabilities test into `transport_tests.rs`. Test counts did not drop. Four items became
  public with docs because they are real API (`CompressionAlgorithm::from_str`,
  `CompressionMiddleware::select_algorithm`, `CorsConfig::is_origin_allowed`,
  `LoggerMiddleware::level`); the auth and body-limit tests that used private fields or the
  private base64 helper were rewritten through the public `handle()` behaviour. The
  `tls_verification.rs` module is dead code (never declared in `native/mod.rs`), so its five
  former tests never compiled and are removed, not moved. Verification:
  `git grep '#[cfg(test)]'` over the three `src/` trees returns nothing, and the native plus
  wasm CI gates for the three crates are green. PR: see the PR.

- 2026-10-30: feature 33 done on `fix/io-01-valtron-parking`. A stream handle that queued
  outgoing data (a reset, write, finish, stop, close, a datagram, or a flow-control update)
  did not wake the QUIC driver. The driver parked on socket readiness and the next timer
  (feature 28); with no incoming packet and no pending timer, the queued data was never
  transmitted. The flaky `http3_server_rejects_a_request_it_has_no_handler_for` test failed
  about 15 times in 20 runs because its `H3_REQUEST_REJECTED` reset was queued but never sent.
  Fix: the driver holds a `BoolSignal` self-wake and shares a clone into every `ConnState` as
  `driver_wake`. The driver clears it before each pass and composes it into its `Depends`
  readiness (socket edge, next timer, or the latch). Every queued-outgoing-data path calls
  `ConnState::wake_driver()`. A new deterministic test
  (`queued_reset_wakes_a_driver_parked_on_readiness`) parks a server driver on an injected
  readiness, queues a reset, and asserts the parked readiness becomes ready. The H3 reject test
  passes 20 runs in a row (was 15/20 fail); the QUIC suite is 8/8. The §3 flaky-test backlog
  entry is deleted. PR: see the PR.

- 2026-10-30: feature 32 done on `fix/io-01-valtron-parking`. The reactor drain poll timeout
  is configuration, not a constant (rule 8). `const DRAIN_TIMEOUT_MS` is gone; its value lives
  in `pub const DEFAULT_DRAIN_POLL_TIMEOUT: Duration = 100 ms`, referenced only by
  `ReactorConfig::default()`. New `ReactorConfig` holds `drain_poll_timeout` with a builder
  (`with_drain_poll_timeout`), a getter, and a documented default. A new entry point
  `Reactor::init_with_config(preference, config)` threads the config into `init_inner` and the
  drain loop; `get()` and `init()` delegate to it with the default config, so their behaviour is
  unchanged. The reactor stores `drain_poll_timeout` and exposes it via
  `Reactor::drain_poll_timeout()`. Tests: a new `config` test target (its own process, so it
  owns the process-global singleton) with default, builder round-trip, and
  `init_with_config`-stores-the-value assertions. Verification: `foundation_reactor` CI green
  (config 3/3, shutdown_signal 4/4, clippy + fmt clean). The §2 `DRAIN_TIMEOUT_MS` backlog entry
  is deleted. PR: see the PR.

- 2026-10-30: feature 31 done on `fix/io-01-valtron-parking`. The accept loop now wakes at
  once on shutdown. `Reactor::wake_waiters()` (new) bumps `events_gen` and calls `notify_all`,
  so a thread parked in `wait_for_events` returns without a real fd event. `ShutdownSignal`
  (new, `foundation_reactor/src/shutdown_signal.rs`) is a cross-platform `AtomicBool` flag with
  a unix-only `OnceLock<Arc<Reactor>>` wake link; `turn_on` sets the flag then wakes the linked
  reactor on the false-to-true change, and `register_wake` links the shared reactor (best-effort).
  It is re-exported through `foundation_reactor`, `foundation_iogate`, and `foundation_http`.
  The http server's shutdown type changed from `OnSignal` to `ShutdownSignal`, which cascaded to
  every `serve*`/`serve_h3` caller: `foundation_connectrpc`, `foundation_auth`,
  `foundation_proxy` (src + tests, except `passthrough.rs` whose signals never reach `serve*`),
  `foundation_keychain`, `foundation_deployment_docker`, and `foundation_testbed`.
  `serve_loop` calls `shutdown.register_wake()` after the listener fd is registered;
  `accept_shutdown_poll` is now a safety bound only (default unchanged at 10 ms). New
  `foundation_reactor/tests/shutdown_signal/` (4 tests: flag semantics + a wake test that parks
  a thread in `wait_for_events` and asserts `turn_on` returns it promptly). Verification:
  reactor 4/4, http suite green, `cargo check --workspace --all-targets` clean. The §4
  `OnSignal` backlog entry is deleted; a pre-existing flaky H3 test and the reactor
  `DRAIN_TIMEOUT_MS` rule-8 const are recorded. PR: see the PR.

- 2026-10-30: feature 30 done on `fix/io-01-valtron-parking`. The remaining valtron limits
  became configuration (rule 8), one field each: `readiness_poll_quantum` (R1, 1 ms) and
  `wait_check_interval` (R7, 4 ms) on `LocalThreadExecutor`; `op_read_time` (R2, 100 ms) on
  `PoolConfig`; `notify_queue_wait_timeout` (R3, 10 ms) on `ThreadedIterFutureConfig`;
  `max_spins` (R4, 100 native / 1 wasm) on a new `NotifyQueueConfig`; `readiness_wait` (R5,
  10 ms) on the executor state (via `Sleepers`); and `delivery_capacity` (R6, 8) on
  `TaskSpawnConfig`. R8's `DEFAULT_PARK_DURATION` (1 ms) and `DEFAULT_WAIT_CYCLE` (300 µs)
  stay as documented optional-parameter defaults, now read from `StreamConfig::default()` so
  the consts are referenced only by that `Default` impl. New
  `tests/valtron/limits_config_tests.rs` (8 tests) asserts the observable fields round-trip
  through their builders and the non-observable ones match their documented defaults. The §4
  backlog entry is deleted. PR: see the PR.

- 2026-10-30: feature 27 done on `fix/io-01-valtron-parking`. `SleepIterator::next` returned
  `Pending(0 ns)` when its deadline equalled the current time, because
  `Instant::checked_duration_since` gives `Some(Duration::ZERO)` at equality. The reached-deadline
  classification is now a pure `pub fn remaining(completed_at, now) -> Option<Duration>` that returns
  `Some(diff)` only for `diff > Duration::ZERO`, and `next` matches on it — a past, exactly-now or
  overflowed deadline is `Done`. A new boundary test (`Some(now)`, `Some(now ± 1 ms)`, `None`) is
  deterministic and no longer depends on `Instant::now` advancing. The flake test
  `zero_duration_sleep_iterator_finishes_immediately` passed 20 runs in a row in both feature sets;
  `foundation_core` CI green (716 + 753 tests). The backlog §4 `SleepIterator` entry is deleted.
  PR: see the PR.

- 2026-10-30: bug-backlog §3 fixed on `fix/io-01-valtron-parking`. The HTTP/1.1 server's
  `WaitingForBody` state no longer retries `probe()` on fixed 30 ms delays and 408s at ~155 ms.
  It now arms the connection's read waker before each probe and, for a transport that reports
  readiness (the WireGuard overlay), parks on `AnyReadiness::either(read_signal,
  DeadlinePassed(deadline))` — the body wakes it the moment it arrives, and a hung client still
  gets 408 once the new `ExpectContinueConfig.body_timeout` (default 3 s) elapses. Blocking
  transports keep the old `base_delay`/`max_attempts` countdown unchanged (their `register_read_waker`
  returns `false`). The `overlay_connector_http_round_trip` test (spec-55 F11, `Expect: 100-continue`
  enabled) now passes 20/20; the four server-side 100-continue integration tests stay green.
  `foundation_http` CI green (both runs) and `foundation_netio` CI green. The backlog §3 entry is
  deleted. PR: see the PR.

- 2026-10-30: feature 28 done on `fix/io-01-valtron-parking`. The idle H3 endpoint now parks on
  the QUIC socket instead of polling at 1 ms. `EventReadiness` gained a default `clear()` and
  `FdReadiness` overrides it (forwards to its existing inherent `clear`); `QuicDriver::next_status`
  clears edge-triggered readiness before `pump_recv` (the H2 handler's clear-before-read order)
  and, when a socket signal is present, returns `Depends(AnyReadiness::either(fd,
  DeadlinePassed(next_timeout)))` instead of `Delayed(IDLE_POLL)`. The deadline composition is a
  new correctness requirement the spec did not state: a `Depends` sleeper has no deadline, so
  parking only on the fd would starve QUIC loss-detection/idle-timeout/keep-alive timers;
  `next_timeout` is the earliest `poll_timeout()` across connections. `serve_h3` wraps the
  driver's UDP socket in `FdReadiness` via `watch_driver_socket` (falls back to polling on
  non-unix or if the reactor refuses). The flaky process-wide `getrusage(RUSAGE_SELF)` CPU
  assertion is replaced with a deterministic poll count (`CountingDriver`): the idle driver
  parks at ~0 `next_status` calls/s instead of ~1000, asserted over a 1 s idle window, plus a
  direct `driver_parks_on_fd_readiness_not_delayed` test (no signal → `Delayed`, signal →
  `Depends`). Verification: netio default + `quic` green (the `quic` run is 1108/1108), http,
  connectrpc, iogate, core, reactor all green, `cargo check --workspace --all-targets` clean,
  and the idle test passed 20 runs in a row. Two backlog entries deleted (the 1 ms poll and the
  process-CPU measurement). PR: see the PR.

- 2026-10-30: feature 10 done on `fix/io-01-valtron-parking`. The three ignored netio test
  files became hermetic: `http_external_validation.rs` (8) and `http_server_integration.rs`
  (6) drive the local `TestHttpServer` on loopback sockets, and `tls_integration.rs` (3) runs
  a local TLS server with a test CA generated at start (`rcgen`). All 37 netio doc examples
  now compile — `ignore` is gone and `no_run` stays only where the example opens a real
  network connection (`cargo test --doc -p foundation_netio` passes 54/54). Every conformance
  test gained an 8-thread twin through a small `macro_rules!` macro that emits the
  `threads = 4` and `threads = 8` copies from one body (`seed + 1000`); the suite is 36/36.
  The formerly-ignored tests and the 8-thread variants each passed 20 runs in a row.
  Verification: netio default 1267/1267, connectrpc default and `h3` green, the `rg` checks
  return 0; the netio `quic` run still fails only the pre-existing
  `idle_h3_connection_uses_no_cpu` (feature 28, process-CPU measurement) — out of scope.
  `unshare` is not available on macOS, so the no-network check could not run. Backlog entries
  #27, #31 and #33 deleted. PR: see the PR.

- 2026-10-30: feature 09 done on `fix/io-01-valtron-parking`. Limits became
  configuration (rule 8), one item each in its own commit. L1-L3 (valtron): the pool config
  gained `max_yield_duration` (100 ms), `yield_wait_time` (4 s) and four back-off knobs;
  `ThreadedIterFutureConfig` documents the queue size and the back-pressure park timeout. A
  real fix in this feature: `block_on`'s multi-worker `Wait` arm re-polled at
  `no_work_yield`, so `max_yield_duration` governed only `run_until` (single-threaded); it
  now caps the re-poll at `min(no_work_yield, max_yield_duration)`. L4:
  `BackgroundJobRegistry` takes a capacity (default 1024); a full queue returns an error and
  `new` returns `Result`. L5-L8: `BytesPoolConfig` bounds `max_pooled_buffers` (64) and
  `max_buffer_size` (64 KiB); `acquire_with_capacity` counts one outcome; `PooledBuffer::owned`
  holds no pool; `Debug` shows length and capacity only. L9: `BatchReader` logs `WouldBlock`
  at `trace`. L10: `SpinWaiter` keeps nanosecond precision with named defaults. L11: a
  release-plz version group publishes the macros and core crates together. Six backlog
  entries deleted; a new entry records the remaining hard-coded valtron limits that were out
  of scope (`READINESS_POLL_QUANTUM`, `DEFAULT_OP_READ_TIME`, `DEFAULT_NOTIFY_QUEUE_*`,
  `DEFAULT_READINESS_WAIT`, `DEFAULT_DELIVERY_CAPACITY`, `JS_WAIT_CHECK_INTERVAL`). PR: see
  the PR.

- 2026-10-30: feature 08 done on `fix/io-01-valtron-parking`. `Serve` gained a fourth result,
  `ConnectionResult::Async(ServeFuture)` (`ServeFuture = Pin<Box<dyn Future<Output =
  ConnectionResult> + Send + 'static>>`), so a long-lived handler returns its continuation at once
  and stops holding a worker. The connection handler (`connection.rs`) gained
  `HandlerState::Awaiting`, which polls the future once per wake with `queue_waker(queue, 0)` and
  parks on `AsyncReadiness` (the `FutureTask` wake bridge ORed with the shutdown signal); `Keep`/
  `Take`/`Close` resolve to the same states as before, and an `Async`-inside-`Async` is a logged
  programming error that closes. The wasm `serve_writer` path treats `Async` as a clear error.
  `ConnectRpcServe::serve` now returns `Async(Box::pin(fut))` and has no `block_on`. The socket wait
  was rebuilt on the readiness seams: `ConnIo` owns a `ConnReadiness` plus an `FdReadiness`
  (kernel fd), `IoReady` parks a `std::task::Waker` (`FdReadiness::register_waker` or a one-shot
  `ConnWaker`) plus a valtron `timer::Sleep` for the deadline, and `Hangup` reads the reactor's
  latched `READ_CLOSED`/`WRITE_CLOSED`/`ERROR` flags so it does not mistake a pipelined request
  for a close. The deadline timer is the existing `ThreadTimer` as a `static SERVE_TIMER`, and
  `dispatch_to` now takes `Box<dyn ResponseSink>` (owned) so the future is `'static`. Regression
  tests: `h1_streaming_calls_do_not_exhaust_the_pool` (8 server-streaming RPCs open on a 2-worker
  pool; a unary call finishes under 100 ms) and `async_serve_tests` (`keep` serves the next
  request, shutdown drops the future, nested `Async` closes). Both pass 20 runs in a row; the
  connectrpc conformance tests stay green. Backlog §1 entry deleted and a §9 proxy follow-up line
  added. PR: see the PR.

- 2026-10-29: feature 07 done on `fix/io-01-valtron-parking`. Three fixes, one commit each.
  §1 (HTTP/3 DATA frames): `FrameDecoder` gained `State::Data { remaining }` and a configurable
  `read_chunk` (default 16 KiB), so a large DATA frame streams chunk-by-chunk instead of buffering
  whole; `H3Connection::max_frame_size`/`read_chunk` thread both onto `FramedRecv`/`H3Request`, and
  a frame whose length exceeds the cap is refused with `H3_FRAME_ERROR` before any allocation
  (`frame_over_configured_cap_is_rejected`). §2 (WebTransport lifetime): `poll_accept_webtransport`
  no longer FINs the CONNECT stream after the 200; it hands the open stream to
  `WtSession::new_with_connect`, whose `poll_connect` drains capsules then treats a FIN/reset as the
  session end (draft-ietf-webtrans-http3 §5 "Session Termination"); `close` sends a
  `CLOSE_WEBTRANSPORT_SESSION` capsule then FIN. Tests:
  `webtransport_session_stays_open_after_accept`, `closing_connect_stream_ends_session`. §3 (wasm
  SSE read error): `spawn_stream_reader` records the JS `ReadableStream` read error in a shared slot
  beside `done`; `QueueSseEvents::end_or_fail` turns it into `Pending(SseProgress::Failed)` then
  `None` instead of a clean end. Native test `queue_sse_events_surface_a_producer_read_error` and a
  wasm-bindgen test `sse_read_error_surfaces_failed_not_a_clean_end`. A bug found and fixed along the
  way: `foundation_reactor` was an unconditional netio dependency with no wasm32 poll backend, so the
  wasm check failed; it is now gated `not(target_arch = "wasm32")` (used only by `native/raw_stream`).
  PR: see the PR.

- 2026-10-29: feature 06 done on `fix/io-01-valtron-parking`. `Connection` and `RawStream`
  gained `conn_readiness()` → a purpose-built `ConnReadiness` in netio: `Fd(fd,
  Option<Arc<dyn TlsInterest>>)` for kernel sockets and `Overlay(Box<dyn OverlayReadWrite>)`
  for the WireGuard overlay. `TlsInterest::corrected` maps an application direction to the
  wire direction rustls needs (`wants_write` first, then "still handshaking"), so the
  opposite-direction stall cannot park on the wrong fd event. The overlay reports the full
  predicate (`readable` = data or EOF, `writable` = space or surfaced error) and is woken by
  one-shot `ConnWaker` callbacks, not `poll(2)`. `conn_io` (connectrpc's blocking HTTP/1.1
  driver) now parks on `ConnReadiness` through a wake socket pair. Two real bugs fixed along
  the way: `OverlayStream`'s `Clone` removed its smoltcp socket twice on drop (now ref-counted
  via `OverlayStreamOwner`), and `HttpClient::open_exchange` silently dropped a
  `with_connector` connector by building a fresh pool (now reuses `self.pool`). Regression
  tests: `overlay_readiness_tests.rs` (20/20, `expect_continue(false)` to dodge the
  out-of-scope foundation_http 100-continue race) and `rustls_opposite_direction.rs` (20/20,
  uses the new `refresh_traffic_keys()` to set `wants_write` deterministically). Inline
  `#[cfg(test)]` blocks in `connection.rs` and `rustls.rs` moved to `tests/tcp_and_tls/`
  (rule 7); two third-party-behaviour tests dropped. Remaining gap tracked in the backlog:
  io_uring completion socket, openssl, and native-tls still report `Unsupported`. PR: see the PR.

- 2026-10-28: feature 05 done on `fix/io-01-valtron-parking`. Pushed request body: the
  `WritingBody` state in `request_redirect.rs` parks on the pipe via
  `TaskStatus::Depends(readiness)` (register-then-check), and the `producer_stalls` back-off
  plus `MAX_PRODUCER_BACKOFF_MS` are deleted. Accept loop: `serve_loop` registers the
  listener fd with `FdReadiness` and blocks in `wait_for_events`, waking at once on a
  connection; shutdown detection is bounded by the new `accept_shutdown_poll` config (10 ms),
  because `OnSignal` is a passive atomic with no wake primitive (see learnings + backlog).
  `would_block_sleep` stays only on the non-unix fallback. Regression tests:
  `pushed_body_waits_without_polling` (poll count under 20), `pushed_body_producer_drop_ends_the_body`
  (renamed from the spec's `_fails_the_request`: dropping the producer closes the pipe = a
  graceful end), and three `accept_loop_parking_tests`. PR: see the PR.

- 2026-10-28: feature 04 done on `fix/io-01-valtron-parking`. The TCP connect and the
  rustls handshake no longer block a valtron worker. The task path
  (`request_redirect.rs`, not the spec's `request_intro.rs` — see learnings) gained two
  states: `Connecting` (a `PendingConnect`/socket2 non-blocking `connect(2)` that parks on
  `SocketReadiness` write readiness and reads `SO_ERROR`) and `TlsHandshake` (rustls
  `complete_io` driven by `drive_handshake`, parked on the direction rustls asks for). One
  fixed connect deadline covers every address and the handshake, so a partial handshake
  cannot reset it. Error kinds survive (`ConnectionTimeout`, `IoError` with the real
  `ErrorKind`, `TlsHandshakeFailed`). The public sync constructors stay on the blocking
  boundary (`Connection::with_timeout`, `TcpStreamWrapper::connect_timeout`, the SSL
  connectors). `connect_completion` in iogate was read and not reused (it does a blocking
  `TcpStream::connect` first, and iogate sits above netio). Four regression tests in
  `tests/tcp_and_tls/nonblocking_connect_tls.rs` pin the behaviour; the blackhole test
  shows a ticker keeps running on a 2-thread-saturated pool. Backlog task #23 deleted; a
  new DNS entry added (`SystemDnsResolver::resolve` still blocks). PR: see the PR.

- 2026-10-28: feature 03 done on `fix/io-01-valtron-parking`. T1–T2 (the §2 hunks:
  `SendSafeBodyBytesIterator` reports `Stream::Wait` for every body kind, and
  `is_would_block`) landed earlier. T3–T6 landed through feature 29 (the reactor bridge):
  `ReadWakeSource` is built from the connection and attached at every body construction site,
  `poll_next` registers the waker then repolls once before parking, the async downstream
  consumers use `into_async_parts`, and the regression tests pass 20 runs in a row. The one
  `wake_by_ref` left in `body_reader.rs` is the logged fallback. The `quic` run still fails the
  pre-existing `idle_h3_connection_uses_no_cpu` (feature 02/28); it is out of scope. PR: see
  the PR.

- 2026-10-28: feature 29 done on `fix/io-01-valtron-parking`. The I/O reactor moved into the
  new `foundation_reactor` crate below `foundation_netio`, so netio can reach `FdReadiness`
  with no crate cycle. `FdReadiness::register_waker(&Waker)` was added. `RawStream::read_wake_source()`
  builds the body's `ReadWakeSource` for three transports: an overlay registers a one-shot
  `ConnWaker`, a non-blocking kernel socket registers its fd with the shared reactor, and a
  blocking or completion socket returns `None` (a blocking `WouldBlock` is a timeout, not a
  wait). `SimpleResponse` gained a fourth field that carries the source; `into_async_parts()`
  and `take_async_body()` attach it, and the sync `into_parts()`/`take_body()` drop it. The
  async downstream consumers (`foundation_ai`, D1, R2, `turbopuffer`, `acme`,
  `foundation_deployment`) now use `into_async_parts` and `collect_*_async`. The regression
  tests live in `tests/simple_http/async_body_readiness_tests.rs` and measure the polling
  thread's CPU, not process CPU. Verification: `foundation_reactor` green (sourcefd doctest
  fixed), netio 999/999 non-quic and 1068/1069 quic (the one quic failure is the pre-existing
  `idle_h3_connection_uses_no_cpu` process-CPU test, feature 02/28), iogate/db/ai/vectors/
  proxy/deployment/connectrpc green. nativeapis still fails `fd`/`poll`/`reactor_shared_selector`
  on macOS (feature 26, pre-existing). PR: see the PR.

- 2026-10-28: feature 29 added (client body reader reaches a Waker-capable readiness source).
  Feature 03 T1–T2 landed and pass (netio 1206 tests, connectrpc, iogate), but T3–T6 are
  blocked: the client body is built in `foundation_netio`, whose kernel-socket readiness is
  poll-based `SocketReadiness` (no `std::task::Waker`), while the reactor that could wake a
  `Waker` (`FdReadiness`) lives in `foundation_iogate`, which depends on netio. Feature 03 §3.2
  says stop and report rather than invent a global registry. Feature 29 records the bridge
  (move the reactor below netio, add a netio poll reactor, or drive the body through valtron
  `Depends`). Pending, size M, depends on 02; blocks 03 → 04 → 05 → 06.

- 2026-10-28: feature 28 added (QUIC driver parks on its UDP socket). It closes the gap that
  feature 02 left open: `serve_h3` builds the `QuicDriver` without `with_fd_readiness`, so the
  driver still polls at 1 ms (`IDLE_POLL`), and `EventReadiness` has no `clear` for the
  edge-triggered socket. Pending, size M, depends on 02.

- 2026-10-28: feature 02 (HTTP/2 and HTTP/3 servers park on readiness) implemented on
  `fix/io-02-h2-h3-readiness`. The h2 connection parks on its socket through `FdReadiness`
  (readable-only; writable is opt-in via `set_writable` so an idle socket does not spin the
  drain thread). The reactor gained `reregister` and kqueue `reregister_fd` deletes a dropped
  filter before re-adding. The H3 accept task parks on its waker/shutdown; H3 request futures
  park on the QUIC connection. Three backlog entries deleted. The CPU test exposed a
  pre-existing executor busy-spin: a worker hosting only a readiness-parked task returned
  `CanProgress` every loop and `block_on` did not yield on it. Fixed by sleeping the readiness
  quantum in `block_on`'s `CanProgress` arm when nothing is runnable. PR number: see the PR.

- 2026-10-28: two new features added from bugs found during the feature 01 review. Feature 26
  (native reactor tests on macOS, `libc::pipe2`/`/proc/self/fd` portability) and feature 27
  (`SleepIterator` zero-duration deadline). Both are pending, size S, no dependency.

- 2026-10-28: feature 01 review fixes landed on `fix/io-01-valtron-parking`. The two blocking
  findings (per-poll callback growth in `TaskStatus::from_stream`/`prime`; full-queue producer
  callback pile-up) and the minor finding (lock on every pop) are fixed via `WakeableStream::announce`,
  `WakeRegistry::replace`, and a `has_waiters()` fast path in `NotifyQueue::pop`. Three new tests in
  `tests/valtron/wakeable_stream.rs` pin each fix. All valtron tests pass in both feature sets.

- 2026-10-28: feature 01 (valtron parking) done on `fix/io-01-valtron-parking`. `ThreadedIterFuture`
  parks on `Pending` and on a full queue; async stream lenses prime before the first pull; the
  `smol/tokio_drives_parked_future_task` hang (without `multi`) is fixed in
  `DrivenStreamIterator::wake`. PR number: see the PR.

- 2026-10-28: features 12–25 added: every other backlog section (6–21) and the four bugs
  recorded only on the branches of PRs #157 and #158 now have a feature. Spec total: 25.
- 2026-10-28: spec written by the main session. Baseline: PR #160 (backlog sections 1–5
  verified; correctness fixes; CI green). Readiness work in progress is on branch
  `fix/foundational-readiness` (commits `31467ebd4`, `ed13119db`, and a work-in-progress commit
  `fcbf523fa` with one hanging test).

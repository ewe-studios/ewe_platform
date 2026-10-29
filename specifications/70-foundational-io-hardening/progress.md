# Spec 70 progress

_Newest first. One entry per feature PR: date, feature, PR, result._

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

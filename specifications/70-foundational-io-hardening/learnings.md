# Spec 70 learnings

_What an agent learned that the next agent needs. Newest first._

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


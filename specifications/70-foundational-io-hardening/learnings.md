# Spec 70 learnings

_What an agent learned that the next agent needs. Newest first._

- 2026-11-06 (feature 142 — a stale commented block survives a compiler):
  - **A comment is invisible to the compiler, clippy and the tests.** The
    commented `clone_stream` block named two enum variants that feature 137 had
    removed, and every check stayed green. A dead-code or refactor sweep must
    grep the comments too, not only the live items.
  - **A later reader treats a stale example as current.** The block looked like
    a working method, so a reader would believe `RawStream` still has a public
    `AsPlain` and an `AsTls` variant. Delete stale examples; do not keep them
    as history, because the version control system keeps the history.
  - **A clone of `RawStream` must not copy the wake source.** The type holds a
    `OnceLock<ReadWakeSource>`. A derived or hand-written clone would either
    share one source between two streams or build a second source for the same
    file descriptor. That is the reason the method stays deleted.

- 2026-11-06 (feature 141 — a negative read wake source is not cached):
  - **Cache a success, not a snapshot of a condition that can change.** Feature
    137 cached `Option<ReadWakeSource>`, so a `None` taken while the socket was
    blocking was permanent even after `set_nonblocking(true)`. The fix keeps
    `OnceLock<ReadWakeSource>` and fills it only on a successful build.
  - **Write the test so it fails on the old code before the fix lands.** Running
    `a_negative_answer_is_not_cached` against `39f4eedb3` gave 0 passed and 1
    failed; that output is the proof that the test pins the defect.
  - **`std::sync::OnceLock::get_or_init` takes `&self`.** `RawStream` is moved
    between threads but never shared as `&`, so a `get` before `get_or_init`
    cannot race with a second builder. A type that is shared as `&` would let
    two callers reach the builder, and the second source takes a second reactor
    token for the same fd.

- 2026-11-06 (feature 137 — one wake source per stream):
  - **Do not cache a negative answer when the reason can change.**
    `read_wake_source` returned `None` because the socket was still blocking,
    and a `OnceLock<Option<_>>` kept that answer after the caller set
    non-blocking. Cache only a successful source; repeat the checks otherwise.
    The backlog now schedules this as feature 141.
  - **A public enum can become a struct without a version bump** when no
    consumer names or constructs the variants. Prove it with a workspace grep
    and the dependent compile checks, then say so in the Result.
  - **A top-level test file is auto-discovered.** Registering it again in
    `tests/mod.rs` compiles and runs the same tests in both targets. Check how
    the crate's `tests/mod.rs` is built before adding a module entry; feature
    136 left its file unregistered for this reason.

- 2026-11-06 (feature 140 — liveness for a bridged waker):
  - **A bridged waker with no liveness state can never be pruned.**
    `ReadinessWaker::from_waker` sets `park: None`, so `is_live()` is always
    true. `ReadinessNotifier::register` and `notify` therefore keep every dead
    waker until the source fires. A bridge whose wait can end first needs a
    liveness token (`from_waker_with_liveness`).
  - **The token belongs to one wait, not to the signal.** A token shared by
    every `cancelled()` future stays true while any wait lives, so a dead waker
    is never pruned. One `Arc<AtomicBool>` per `Cancelled`, set false in `Drop`,
    bounds the registrations to the live waits.
  - **`will_wake` only de-duplicates the same task waker.** A valtron task that
    re-parks gets a fresh `ParkState`, and therefore a fresh `Waker`, on each
    poll. `will_wake` is then false and the growth was real. A test that reuses
    one `Waker` does not show the growth; use a distinct waker per poll.
  - **Expose a count, not the field.** A crate outside `foundation_core` cannot
    read the notifier of a `ResumeSignal`. Three `#[doc(hidden)]`
    `waiter_count()` hooks (`ReadinessNotifier`, `ResumeSignal`, `CancelSignal`)
    let the test prove the bound without private access.
  - **Liveness makes an old de-duplication unsafe.** `register` skipped a push
    when `will_wake` matched. Two live waits behind one task then shared one
    registration whose token belonged to the first. When the first ended, the
    notifier pruned the only waker and the second never woke. De-duplicate on
    the wait identity (`Arc::ptr_eq` of the token) as well as on `will_wake`.
    A repeated poll of one future keeps its token, so its registration stays
    single; two waits keep one each.

- 2026-11-06 (feature 139 — where a slow poll is actually measured):
  - **The cited poll line numbers are inner polls, not the worker drive point.**
    `FutureTask::next_status` polls a future, but a pool worker invokes the task
    through `ExecutorState::do_work` -> `iter.next(...)` in `executors/local.rs`.
    That call is the one place to measure: it sees every task type and every
    wrapper, including the single-threaded engine.
  - **Name the task from the trait, not from the entry.** `ExecutionIterator::name()`
    defaults to `core::any::type_name::<Self>()`; the boxed and `Rc`/`Mutex`/`Arc`
    wrapper impls forward it. Without a name the warning cannot tell a developer
    which task to fix.
  - **Warn at most once per task per second.** A blocked task blocks on every
    poll turn. An unthrottled warning floods the log and hides the other tasks.
    The per-entry `last_warn` map is removed with the task's `Ctx`.
  - **A blocking task and a blocking caller look different.** The caller of a
    `#[valtron_test]` body is a normal thread; the body itself is polled by a
    worker. Measure the poll, not the test wall time.
  - **Two existing gates stay red for unrelated reasons.** A `-D warnings`
    clippy run that includes `foundation_macros` fails on the pedantic debt of
    feature 120, and a full multi run can show the `targeted_wake` timeout. Note
    both as pre-existing, but do not call them green.

- 2026-11-06 (features 131, 138, 139 — valtron surface and pool starvation):
  - **The poll of a `#[valtron_test]` body runs on a valtron pool worker.** The
    caller thread blocks on the result, and the pool polls the body's future. A
    synchronous call inside the body (`thread::join()`, a blocking socket read, a
    blocking file read) therefore holds a worker, not the caller's thread. On CI
    with 2 vCPUs the pool has 2 task workers, so two such bodies starve it. This
    is the CI-only failure of `a_timed_out_receive_cancels_the_call`: the H2 pump
    task never ran, so the socket never closed.
  - **Reproduce pool starvation by pinning the pool.** `threads = 1` on the test
    plus `--test-threads=2` failed 3 of 3 runs locally; `threads = 2` or one test
    thread passed. A CI-only async failure is worth a pinned-pool run before any
    client-side theory.
  - **Wait for a thread without blocking a worker.** Poll `JoinHandle::is_finished()`
    with `valtron::sleep_async(10 ms)` up to the bound, then `join()`. The helper
    `join_within` in `tests/client_deadline_tests.rs` is the pattern.
  - **A "leak" of valtron means a detail, not a type.** External crates may use the
    surface: implement `EventReadiness`/`WakeableStream`, return
    `TaskStatus::Depends(AnyReadiness)`, use `EventQueue`/`Pipe`, and fire a
    readiness source's own `ReadinessNotifier`. They must not hold a waker to poke
    the executor. The audit of this branch found no such leak: the H2 pump reports
    readiness and never touches a waker.
  - **`ResumeSignal` is the port for "set a flag and wake the waiters".** It is a
    flag plus a `ReadinessNotifier`; `set(true)` fires the notifier and valtron
    wakes. `ReadinessWaker::from_waker(cx.waker().clone())` bridges a `Future` to
    it. A hand-rolled `Mutex<Vec<Waker>>` plus `wake()` duplicates this and loses
    de-duplication and stale-waker cleanup (`CancelSignal` did; feature 138 moved
    it).
  - **Register before the re-check, and ignore the return in a `Future`.**
    `EventReadiness::register` returns "wakes on every change", which the executor
    uses to sleep without a quantum. A `Future` re-checks its own state instead,
    so it ignores the value but must still register first to close the race.

- 2026-11-06 (feature 129, macOS read edge lost):
  - **macOS kqueue reports pipes level-triggered despite `EV_CLEAR`.** The
    selector in `foundation_reactor` suppresses repeated read events itself: it
    stores the fd's byte count and delivers again only when the count grows. A
    drained socket that receives a smaller frame produces no new edge.
  - **`Reactor::clear` is the re-arm.** Clearing `READABLE` calls
    `Poll::rearm_readable`, which forgets the stored count. `FdReadiness::clear`
    is therefore part of the park protocol, not only a flag reset. The netio
    wake source skipped it; the task never woke again.
  - **Linux CI cannot prove edge-triggered code.** epoll re-arms without a call,
    so the missing `clear` passed every Linux run. A readiness change needs a
    macOS run, or a test that drives the selector directly.
  - **A 10-of-10 failure is evidence.** The feature was recorded as
    intermittent. Ten runs alone and ten full-suite runs failed every time,
    which narrowed the range at once. Count runs before theorising.
  - **Probe the selector, not only the pumps.** The decisive line was the
    selector's own `data=15 last=Some(77) new_edge=false` printout: it proved
    the bytes arrived and were suppressed, rather than lost in the relay.

- 2026-11-06 (feature 135, response head publication):
  - **A shared slot is not a wake.** A `Mutex<Option<T>>` lets a reader see a
    value that is already stored; it cannot tell a reader that a value arrives
    later. Every publish must fire a signal, and the reader must park on that
    signal. `Pipe<()>` of depth 1 is the smallest such signal in this tree:
    `try_send` reports a full pipe when a second publish repeats the wake.
  - **A peek field is a symptom.** `PipeClientReceiver::peeked` existed only to
    hold the frame that the head wait consumed. Once the wait parks on the
    publication, the field and its three-state `Option<Option<Frame>>` drop out.
    Look for this shape when a wait is folded into an unrelated read.
  - **The close of the wake pipe is the "no head" signal.** The receiver half
    returns `None` when every sender is dropped, which is exactly a transport
    that ends before the head. Keep one sender alive only in the reader task,
    so the close happens at the right time.
  - **Closing this class of bug unlocks the stronger test.** Feature 131 had to
    send one body message to release the call setup. Feature 135 restores the
    head-only peer, which proves both the setup and the drop.

- 2026-11-06 (feature 131, dropped facade cancels the call):
  - **`CancelSignal` is global to the call.** It is one `Arc` over a shared
    flag, and every clone shares it. A raw fire from one facade stops every
    task of the call, so a split bidi stream needs a live-handle count. A
    private guard with `Arc<AtomicUsize>` (`Clone` increments, the last `Drop`
    fires) gives the count without implementing `Drop` on the facades, which
    would stop `BidiStream::split` from moving fields out.
  - **Do not implement `Drop` on a struct that a method destructures.**
    `split(self)` moves the facade's fields; a `Drop` impl forbids that move.
    Put the `Drop` on a small guard value held by each facade instead.
  - **A facade wait that times out must fire the call signal.** Passing the
    facade's signal to `with_deadline_at` is the smallest change, and it also
    stops the reader and writer tasks, which the caller alone cannot do.
  - **A peer that sends the head and no body cannot release
    `server_stream`.** `PipeClientReceiver::response_headers` returns the
    published head only when the reader task stored it first. The call setup
    has no yield between spawning the reader and asking, so the reader cannot
    publish first; the method then waits for the first body frame. Use a peer
    that sends one message for a drop test, and record the wait as backlog §1.
  - **A drop closes the socket through the pump, not the protocol reader.**
    The signal drops the reader task, which drops the body receiver. The h2
    pump sees the closed body pipe, sends `RST_STREAM(CANCEL)` and exits, which
    closes the TCP connection; the peer observes `Ok(0)`.
  - **Prove the old-code failure by reverting only the source file.** Save the
    new `client.rs` outside the tree, `git checkout --` that file, run the new
    tests, record the failure, then restore. The test file then compiles against
    the old library because it uses only public API.

- 2026-11-06 (feature 127, non-blocking H2 dial):
  - **A non-blocking connect to a closed loopback port does not fail inside
    `connect`.** On macOS it returns `EINPROGRESS` (errno 36) and reports
    `ECONNREFUSED` (61) through `SO_ERROR` once the fd turns writable. Linux does
    the same for TCP. The dial failure therefore always reaches the caller from
    the pump, not from `open`; a test that expects `Err` from `open` fails.
  - **Rust does not map `EINPROGRESS` to `WouldBlock`.** Check
    `raw_os_error() == Some(libc::EINPROGRESS)` on unix as well as
    `kind() == WouldBlock`. `foundation_netio`'s `PendingConnect` does the same;
    the order is: socket, non-blocking, connect, then treat these two as in
    flight.
  - **`SO_ERROR == 0` does not prove the connection is up.** While the connect is
    in flight `take_error` also returns `None`. Call `peer_addr()`: it succeeds
    only after the connection completes and fails with `ENOTCONN` before. Use
    both checks.
  - **Do not read before the connect settles.** An in-flight non-blocking socket
    answers a read with `ENOTCONN`, not `WouldBlock`, and the pump's error arm
    would end the call. Settle the connect at the top of the turn, before the
    read loop, and only then enter `Handshaking`.
  - **Arm WRITABLE before the pump's first turn.** Register the fd with the
    reactor in `open`, then call `set_writable(true)` and record
    `watching_writes = true`. The connect-completion edge can arrive before the
    pump first runs. Disarm it when the connect resolves.
  - **Two budgets, one per phase, are correct here.** A single deadline at
    `open + connect_timeout + handshake_timeout` doubles the wait for a silent
    peer and breaks `an_idle_h2_call_does_not_busy_poll`, whose 200 ms handshake
    timeout must fire inside a 5 s test bound. Start the connect budget at
    `open` and the handshake budget when the connect resolves.
  - **The closed-port test and the blackhole test fail for different reasons.**
    On the old code the closed-port test fails because `open` returns `Err`
    (the path changed); it is not a timing test. The blackhole test is the
    timing proof: `open` took 302.29 ms with the blocking dial, and under 50 ms
    with the new one.
  - **`10.255.255.1:9` stays in flight on macOS.** The OS would wait about 75 s,
    so the pump's 300 ms deadline is what ends the call. The unrelated task
    finishes at about 50 ms. Record a skip when `open` returns `EACCES`,
    `ENETUNREACH` or `EHOSTUNREACH` at once, and when the head resolves before
    the unrelated task can run.

- 2026-11-06 (feature 126, close-only pipe readiness):
  - **A spurious readiness wake is not a poll turn.** The single-threaded
    executor gates a woken sleeper on `(park.take_woken() || due) &&
    signal.is_ready(dur)` in `executors/local.rs`. The old `ReceiverGone` adapter
    reports not-ready after a pop, so its notifier runs but the pump never
    re-runs. The pump turn count is 3 on both the old adapter and the new
    close-only signal. Do not plan a turn-count regression test for a wake that
    leaves its signal false; the only cost is the executor-thread interrupt, and
    no public counter observes it. A plain `vacancy()` park does fail the same
    test, because vacancy is ready on an empty pipe and the task spins.
  - **A pop must land while the pump is parked for the test to mean
    anything.** With 20 ms chunk spacing the consumer popped in the window
    between the pump's push and its park, so even the old notifier found no
    waker. The test now sends one chunk every 200 ms and waits 220 ms before
    each pop, which leaves at least 160 ms of parked time.
  - **The design's `PipeInner` notifier duplicates the queue's.** The pipe's
    queue is an `EventQueue`, and `PipeInner::close_queue` closes it. A
    close-only notifier in `EventQueueInner` is fired once by
    `EventQueue::close` (guarded by the open-to-closed change). A second field
    in `PipeInner` would be unused. Both drops call `close_queue`, so the
    open-to-closed guard is what makes the wake exactly-once.
  - **There is no `LocalQueue` in the workspace.** The design names
    `executors/local.rs`; the queue there is `NotifyQueue`, and its
    `space_tasks` notifier fires on every pop. It has no close-only notifier, so
    no accessor was added.
  - **The disk stopped two verify commands.** `foundation_proxy` links three
    test binaries and failed with `ld: ... No space left on device`; the wasm32
    check was not attempted. The `foundation_core/multi` impl did compile in the
    proxy graph before the link failed.

- 2026-11-06 (feature 128, wasm deny gate):
  - **`cargo check` takes no trailing rustc arguments.** This cargo
    (`1.100.0-nightly`) rejects `cargo check ... -- -D warnings` with
    `error: unexpected argument '-D' found`, unlike `cargo clippy` and
    `cargo test`. The deny flag must travel in `RUSTFLAGS` for that single
    command. Cargo caps lints on registry dependencies, so the flag does not
    turn a third-party warning into a failure; the workspace path dependencies
    stay uncapped and must be warning-free.
  - **A per-command environment keeps the package environment intact.** Setting
    `RUSTFLAGS` for the whole package would also change the native build's
    fingerprints. A small plan value that carries argv plus its own environment
    scopes the flag to the wasm gate.
  - **A wasm warning can be a native-only item, not dead code.** All three
    warnings came from functions that the H2 server and `native/h3_serve.rs`
    call. The fix is a `cfg` gate, never a deletion. Confirm each caller first:
    `native` is compiled only off wasm.
  - **The test for a CI plan does not need `cargo metadata`.** Extracting
    `settings_for(name, defaults, package)` from `load_settings` lets a unit
    test merge the real `ci/packages.toml` table and build the real plan,
    without the slow workspace scan or its unknown-package check.

- 2026-11-06 (feature 117, timestamps from the injected clock):
  - **A required trait method breaks every implementor, including ones outside the
    feature's crate scope.** `Clock::wall_now` is required because a default would
    read the real global, but `foundation_netio`'s test `SimClock` is a fourth
    implementor. The gate and the crates compile, but the netio test target does
    not. The plan must name every implementor before it sets the crate scope, or
    the scope must include the implementor files. The main session added the
    method after review; a test clock that reads only `now` returns a fixed
    `UNIX_EPOCH` for `wall_now`.
  - **An allowlist needs a stale-entry rule.** A ratchet that only fails on new
    reads keeps entries after the fix, and the count stops going down without a
    signal. The gate now fails on an entry that matches no read, and a temp-dir
    test must pass an explicit empty allowlist or the repository allowlist counts
    as stale there.
  - **A directory allowlist entry and a file allowlist entry are both needed.** The
    real time backends are a whole crate subtree (`foundation_compact/src/`) plus
    one file (`foundation_core/src/valtron/ctx.rs`). The gate supports a trailing
    slash for the first and an exact path for the second.
  - **Measure the tree, do not copy the plan's counts.** The plan was written on
    2026-11-05; features 116 and 118 changed `foundation_core` afterwards. The
    measured start is 452 reads, not 428. The phase files use the measured values.
  - **The scan finds doc comments and the polyfill, not only calls.** Words like
    `` `Instant::now()` `` in a doc comment match. A directory allowlist entry for
    the real backends is simpler and more stable than a line marker per doc line.
  - **A pre-epoch time needs a negative branch, and seconds round down.** On the
    native and wasm `SystemTime`, `duration_since(UNIX_EPOCH)` returns an error
    before the epoch. `unix_seconds` rounds down on the number line, so half a
    second before the epoch is `-1`, not `0`.
  - **A simulated wall clock must start at a fixed value.** `SimulatedClock` starts
    its wall clock at the UNIX epoch and moves only through `advance_wall`. Reading
    `SystemTime::now()` in the constructor would make a deterministic simulation
    test (DST) non-deterministic.
  - **`cargo clippy` found pre-existing debt in the crate under change.** The
    `needless_continue` in `foundation_compact`'s getentropy backend is fixed in the
    same change, because a zero-warning run is the rule.

- 2026-11-06 (feature 124, streaming receive deadlines):
  - **A deadline test needs a peer that does not enforce the deadline.** The
    real Connect server reads `Connect-Timeout-Ms` and races the whole handler
    against it. A stalled server-stream against the real server still ends with
    `DeadlineExceeded`, so the test passes on the old client code and proves
    nothing. Use a raw HTTP/2 peer (`H2Channel`) that ignores the header when
    the property under test is "the client enforces its own deadline". Keep the
    real server for the no-deadline path.
  - **Copy the absolute deadline into the facade, do not count idle time.**
    "A call fails on its deadline" is a gRPC contract about one end time, not
    about each wait. A stream that sends one item per minute under a 10 s
    deadline fails on the first wait that crosses it. The unary path and the
    facade now use the same absolute instant.
  - **Split the helper instead of duplicating the race.** Taking the deadline
    and an optional cancel reference keeps one implementation for setup and
    for every facade wait. The feature 123 form keeps its own past-deadline
    check so its message and cancel fire do not change.
  - **Dropping a facade does not stop its reader task.** The facade holds no
    `CancelSignal`. The reader task owns the response body, so it stays parked
    on a silent peer after the caller drops the stream. The stalled-peer test
    observes the socket stay open past the deadline and the drop. Recorded in
    backlog §1.

- 2026-11-06 (feature 125, the H2 client pump):
  - **Clear reactor readiness before an attempt, never after it.** The `FdReadiness`
    module doc and `foundation_http`'s h2 server (`arm_wakers`) clear at the start of a
    turn. A clear after `WouldBlock` can erase an edge that arrived during the attempt
    and lose the wake. A clear before can only cause a spurious wake, which the next
    read resolves.
  - **Read until `WouldBlock` before parking.** The h2 channel can answer `WouldBlock`
    with bytes still in the kernel: the read took 8 KiB and a frame needs more. Parking
    there after clearing loses the edge. Guard every park with "if this turn read bytes,
    continue".
  - **A park must include a wake for every liveness check in the turn.** The pump checks
    `head_tx.is_closed()` and `body_tx.is_closed()` to abort a cancelled call. A parked
    task never runs those checks: without a close wake, a cancelled gRPC call left the
    pump parked and never sent `RST_STREAM`
    (`client_cancel_resets_the_stream_on_the_server` failed 3/3). `QueueVacancyReadiness`
    is not a close signal, because an empty pipe is "not full". The close-only adapter
    shares the pipe's vacancy notifier, so a consumer pop still wakes the pump; the
    readiness filter stops a spin, and the cost is one extra poll per delivered chunk.
    A dedicated close notifier on `PipeSender` would remove it.
  - **Arm write interest only when a write comes up short.** Arming it before every write
    latches WRITABLE on an always-writable socket; the next park is already ready, logs
    `[DEPENDS] already-ready signal`, and costs one extra poll per turn. Arm it in the
    `WouldBlock` arm, and drop it when `pending_out` drains.
  - Measured pump turns: idle 200 ms handshake = 2 (old `Delayed(10 ms)`: 18-19); 300 ms
    delayed body = 5 (old: 29).

- 2026-11-06 (feature 121, the proxy stall):
  - **A blocking read inside a valtron task freezes the whole worker.** A task's
    `next_status` runs on a worker that also hosts other tasks. If it calls a
    blocking `recv`, every task on that worker stops, including the ones the
    blocking call waits on. The symptom is two tasks frozen at once (here the
    proxy relay and the client's H2 pump). Rule: keep a task non-blocking and
    park on a readiness.
  - **`send_async` returns a blocking response socket.** `SendRequestTask`
    restores blocking mode for the synchronous body readers. An async streaming
    caller must call `set_nonblocking(true)` and attach `read_wake_source`, or
    the body read blocks the worker.
  - **A past-due `DurationWaker` must report zero remaining time, not `None`.**
    `Sleepers::time_until_next` filters `None`, so a due timer vanished from the
    minimum and the worker slept to the next far deadline. The window is real:
    `wakeup_ready_sleepers` runs before `do_work`, and a task step can last
    longer than a 10 ms timer. Evidence: `worker ... last_wait_ms=59999` next to
    a 10 ms pump poll.
  - **Preserve `WouldBlock` through error mapping.** `map_err` collapsed errors
    into `io::Error::other`, which erased the kind and made "no bytes yet" a
    terminal error. A body stream can park only if the kind survives.
  - **Instrument the scheduler, not only the socket.** The reactor dump was
    clean (`wake_skipped=0`). The decisive evidence was the worker's last sleep
    duration and its sleeper count, sampled next to the frozen poll counter.
  - **A green run that takes 16-24 s for 17 tests is a clue.** The tests are
    serialised by the pool gate. Each test paid a pool-shutdown wait for a
    blocked worker. After the fix the suite takes about 2.7 s.

- 2026-11-06 (feature 123, the client deadline):
  - **Enforce a deadline above the transport trait, not inside each transport.**
    `Transport::open` is synchronous and receives no `Ctx`, so a transport
    cannot read the deadline. Put the race in the shared `Client`, which every
    transport goes through, and a new transport inherits the bound. This is
    stronger than a per-transport check, which a new transport can forget.
  - **`sleep_async` is the portable timer.** On native it rides the valtron
    timer thread; on wasm it is a valtron sleep task. Racing it against the call
    future keeps one code path for both targets, with no thread and no OS timer.
  - **A past deadline must be checked before the first poll.** The call future
    dials in `transport.open` on its first poll. Return the error before polling
    it when `remaining` is zero, or the "no request" contract breaks.
  - **The old code does not hang forever, so assert the code and the time.** The
    H1 client has its own timeout: the call without enforcement failed after
    about 3 s with `Unavailable` ("transport closed without response head"),
    while the new code returns `DeadlineExceeded` in about 200 ms. A test that
    asserted only "an error" would pass on the old code.
  - **Check the wasm gate's warning policy separately.** The wasm run in
    `ci/packages.toml` does not deny warnings, while the native clippy run does.
    A crate can be clippy-clean and still print warnings under
    `--no-default-features --features rpc` on wasm32. Record them.

- 2026-11-06 (feature 122, the fd-reuse deregistration):
  - **A number-keyed selector needs a number-to-owner map to deregister safely.**
    kqueue, epoll and io_uring identify a registration by fd number. A token is
    therefore not enough to remove a registration: the reactor must also know
    which token currently owns the number. `Reactor::fd_tokens` provides that,
    under the existing `reg_lock`, so no new lock order appears.
  - **A stale-drop test must control fd allocation.** The operating system gives
    the lowest free number, so closing a socket then opening a pair gets the
    number back. Keep every non-matching pair open so each retry gets a higher
    number, and serialise tests that need a known number with a process-local
    mutex: parallel tests in one binary compete for the same fd.
  - **Prove a guard test by reverting only the guard.** Replacing
    `reactor.rs` with `git show HEAD:...` and keeping the new test showed
    `a drop after fd reuse: fd 3 (FdReadiness { fd: 3, token: Token(2) }) did not
    report readability within 2s`. A full revert would not isolate the guard.
  - **Check the package's CI lint policy before trusting a verify command.**
    `foundation_reactor` is not in the `clippy-deny-warnings` set in
    `ci/packages.toml`, and it carries 120 pre-existing pedantic warnings. The
    `-- -D warnings` check fails at HEAD and on `master` for that reason alone,
    so a new feature cannot make it pass without the crate-wide cleanup. Record
    it; do not hide it.

- 2026-11-06 (the second proxy failure):
  - **A selector keyed by fd number can lose a live registration.** The reactor's
    entries map is keyed by token, but kqueue, epoll and io_uring all identify a
    registration by fd number. `Reactor::deregister` passes the number alone, so a
    drop that runs after the socket closed can delete the entry of a *new* socket
    that reused the number. The parked task then never wakes. Feature 122.
  - **A bounded retry loop cannot explain a long stall.** The accept loop retries
    `accept` every `accept_shutdown_poll` (10 ms by default) and on every reactor
    wake. A clobbered listener registration costs one pause, not 30 s. Check
    where the wait is bounded before blaming the accept path.
  - **A recorded deadline is not an enforced deadline.** `Ctx::with_deadline`
    stores `now + d` and starts no timer. A hung client call waits for the
    transport's own idle timeout. Feature 123.
  - **Trace logs localise a hang faster than a watchdog.** Setting
    `RUST_LOG=foundation_http=trace,foundation_proxy=debug` and keeping the
    failing run's log showed the exact stop point in one reproduction: the
    proxy front parsed and dispatched the request, the backend completed its
    handshake, and then the backend never read the request. A watchdog would
    have needed the same number of runs plus a rebuild.
  - **Constrain the CPUs to reproduce a load-dependent flake.** Five runs of both
    proxy binaries in the Linux CI image on this host pass; with
    `docker run --cpus=2` the same loop failed once in five, on a third test
    (`connect_client_stream_via_grpc_backend`, 29.14 s). A GitHub runner has
    about two usable cores. Always set `--cpus=2` when the flake needs load.
  - **Record the selector backend before comparing environments.** On Linux the
    reactor picks io_uring when its probe succeeds and epoll when not
    (`poll/sys/unix/selector/dispatch.rs`). A local pass on a different backend
    says nothing about CI.

- 2026-11-06 (CI proof of features 115, 116 and 119):
  - **One stalled test holds every later test of its binary.** `#[valtron_test]`
    takes the process-wide pool on the harness thread, and the pool gate
    serialises the tests. When `connect_streams_spread_across_grpc_backends` hit
    its 30 s cap, six queued tests finished within 0.7 s of the kill. A hang in
    one test looks like a slow suite, and the visible symptom is the timeout.
  - **A single test run does not prove a suite.** That test passes alone in
    0.11 s and in six local full-suite runs (16 s to 22 s), yet it timed out on
    Linux CI. Recorded as `docs/bug-backlog.md` §9 and scheduled as feature 121.
  - **Two failed jobs can have unrelated causes.** The same run failed
    `foundation_core` on a Linux-only lint and `foundation_proxy` on a timeout.
    After a fix, re-read every failed job; do not stop at the first.

- 2026-11-06 (feature 116's Linux proof):
  - **A platform-gated test module hides its lints from the host.** The module
    behind `#[cfg(all(target_os = "linux", feature = "multi"))]` in
    `tests/valtron/readiness_deadline_tests.rs` is not compiled on macOS, so a
    local zero count is not a Linux zero count. GitHub run 42087629722 found two
    `clippy::used_underscore_binding` errors (`mem::forget(_stream)`). Name a
    binding you use, even in a test. To check a Linux-only module before the CI
    round trip, run the clippy command in the CI image with
    `docker run ... scripts/ci/linux.sh`-style mounts.

- 2026-11-06 (feature 115, clock injection):
  - **A multi-pool task did not read `ThreadRegistry.root`.** The worker's
    `LocalThreadExecutor` had its own `Ctx::new()`, and that is what
    `ExecutorState::root.fork()` gives a task. Injecting the registry root alone
    changes nothing. Fix in this feature: build the worker executor with
    `from_seed_with_root(..., self.root.clone())`.
  - **Giving workers the pool root also shares its `config` and `rng`.** Before
    feature 115 each worker built its own `Ctx::new()`, so its seed-0 RNG stream
    was separate. A task now forks the pool root, so one stream serves the pool.
    That is the deterministic simulation testing (DST) behaviour a host wants,
    and no test depended on the separation. State such a side effect when you
    change where a root comes from.
  - **`std::time::Instant` is opaque, so a simulated clock cannot be rebuilt
    from a `Duration`.** Store a base `Instant::now()` and a `Mutex<Duration>`
    offset; `now()` is `base + offset`. Differences stay exact; the absolute
    value is not an epoch.
  - **A `catch_unwind` closure that owns a `Ctx` is not `UnwindSafe`.** The
    `Arc<dyn Clock>` is not `RefUnwindSafe`. Wrap the worker closure in
    `AssertUnwindSafe` when the closure only builds a local executor.
  - **A macro expression used on two threads must be bound once.** In the
    `timeout` path the same `ctx = expr` would be evaluated on the outer thread
    and on the body thread. A local `Ctx` would move twice; bind it once and
    clone for the outer pool.
  - **`foundation_macros` is not a CI package and has 62 pedantic diagnostics.**
    The feature's `-D warnings` command fails before and after the change.
    Recorded in `docs/bug-backlog.md` §30 and scheduled as feature 120.

- 2026-11-06 (feature 119, tracing capture test):
  - **`tracing`'s interest cache is process-wide, and `rebuild_interest_cache`
    does not close the race.** Another test thread in the same binary that
    reaches a callsite with no subscriber caches it as `never` again, so a
    scoped-subscriber test records nothing. Run such a test in its own
    integration target: one test, no other thread touches the callsite.
  - **A test failure on a docs-only commit is a flake, not a regression.**
    Compare the failing SHA with the last green one before you look at the
    code. Run 42072422806 failed on `680b96e45`, whose only difference from the
    green run was Markdown.

- 2026-11-06 (planning 115, 117 and 118):
  - **A crate-level `#![allow]` outranks `-D warnings`.** `Cargo.toml` sets
    `pedantic = "warn"`, but `foundation_core/src/lib.rs` still starts with
    `#![allow(clippy::pedantic)]` and three others from `f6916c465`. Both clippy
    runs and the new CI deny setting are green while the pedantic group is
    hidden inside that crate. Read the crate root before trusting a zero count;
    feature 118 removes the allows.
  - **`clippy::cognitive_complexity` is a `nursery` lint.** The workspace does
    not enable `nursery`, so allowing it changes nothing today. It is dead
    configuration, not debt.
  - **The executor never reads `ctx.clock()`.** A grep of the engine finds the
    accessor only at its definition and in tests. Injecting a clock into `Ctx`
    therefore changes no scheduling behaviour: parking stays on the real
    monotonic clock. Feature 115 states this limit so a DST run is not expected
    to replay time.

- 2026-11-06 (feature 116, `foundation_core` clippy reaches zero):
  - **`cargo clippy --fix` reverts the whole crate when one suggestion does
    not compile.** It printed `errors present after applying fixes to crate
    mod` and wrote nothing. `--broken-code` writes the suggestions so the one
    broken fix can be repaired by hand. The broken one was
    `assert_ne!(b.as_slice(), [])` from `clippy::assert_is_empty`; the empty
    array has no element type (`E0282`). A typed empty (`&[] as &[u8]`) fixes it.
  - **An `#[expect]` must be `cfg_attr`-gated when the lint fires in one feature
    tree only.** `arc_with_non_send_sync` fires in the single-threaded tree and
    not under `multi`. A bare `#[expect]` would be unfulfilled under `multi` and
    fail `-D warnings`. Use
    `#![cfg_attr(not(feature = "multi"), expect(...))]` in the test module.
  - **A block scope is the only shape for the `#[valtron]` guard.** `mem::drop`
    reports `drop_non_drop` in the single-threaded build, where `PoolGuard` has
    no `Drop` impl. Moving the guard into a block drops it at the block end in
    both trees. In the `timeout` panic arms the guard now drops during the
    unwind; the success path is unchanged.
  - **The bare build is clean only when `futures-core` is optional.** Gating
    `pub mod valtron` on `std` removed the unresolved imports, but cargo then
    reported `unused dependency futures-core`. Making the dependency optional
    (pulled in by `alloc`/`std`) leaves the named `compile_error!` as the only
    output.
  - **The `#[task]` macro pins the transition type.** `state_task.rs` emits
    `fn(Ctx, T)` and accepts only a bare `Ctx`, so an example cannot take
    `&Ctx` to silence `needless_pass_by_value`. Use a function-level `#[expect]`
    with a reason; the real fix is a macro API change (backlog §4).

- 2026-11-05 (CI proof for features 108 to 113):
  - **A push can leave `refs/pull/<n>/head` stale.** Push `02cbb08b5` moved
    `refs/heads/spec70/priority-followups` at once, but `refs/pull/180/head`
    stayed on the previous commit for more than 10 minutes. `git ls-remote`
    shows the mismatch. No workflow runs, because the `pull_request` event
    never fires. Fix: close and reopen the pull request. Reopening makes
    GitHub read the branch head again and starts a run for the true commit.
  - **A job log is unavailable until its run finishes.** `gh run view --job
    <id> --log` answers "still in progress" and `gh api .../jobs/<id>/logs`
    stops on terminal escape sequences. Use
    `gh api --allow-escape-sequences .../jobs/<id>/logs` to read a finished
    job's log while the rest of the run continues.
  - **Read the job that owns the feature.** `foundation_nativeapis` runs the
    ptrace spawn tests only with `EWE_REQUIRE_PTRACE=1` on a native kernel; a
    green `foundation_netio` or `foundation_browser` job proves nothing about
    them.

- 2026-11-05 (feature 109, `foundation_browser` clippy debt):
  - **`i64::try_from(f64)` does not exist.** Rust has no safe fallible
    float-to-integer conversion. The checked-conversion route cannot remove
    `cast_possible_truncation`. Use one small helper with `#[expect]` and a
    measured reason, or `f64::to_int_unchecked` in `unsafe`.
  - **A leading `+ ` in a doc comment is a Markdown list marker.** It makes the
    next line a lazy continuation and raises `doc_lazy_continuation`. Reword
    the line ("plus …") instead of adding indentation.
  - **Do not change a public signature to fix `needless_pass_by_value`.** Rust
    cannot add a reference to a by-value parameter without breaking every
    caller. Put `#[expect(clippy::needless_pass_by_value, reason = "...")]` on
    the function; the reason names the public API.
  - **`match_same_arms` can appear across a merged arm.** After merging the
    close/error/end arms in the reader loop, clippy then asked for nested
    or-patterns (`Some(Ok(..) | Err(_)) | None`). Run clippy again after each
    merge.
  - **`tests/smoke.rs` is behind `browser-tests`.** The default test run has 0
    smoke tests and 1 SSE test. Do not claim the browser suite ran without a
    real Chromium.

- 2026-11-05 (feature 113, `foundation_nativeapis` clippy debt):
  - **Run `cargo clippy --fix` in the Linux image first.** One pass with
    `--features vfs-preload --no-deps --fix --allow-dirty` applied 29 file
    fixes and removed over half of the 384 unique diagnostics (backticks,
    inline `format!` args, pointer casts, `map_unwrap_or`, `LazyLock`,
    `implied_bounds_in_impls`). It needs `--allow-dirty` when the tree already
    has edits. The bind mount means it edits the host checkout.
  - **Only prose lints remain after `--fix`.** `missing_errors_doc`,
    `missing_panics_doc` and `missing_safety_doc` are not machine-applicable.
    Their JSON span points at the `fn` line, so a script can insert the
    sections before each signature; group by `(file, line)` because one
    function can miss two sections.
  - **A doc comment inserted before an `extern` block function lands after
    `#[unsafe(no_mangle)]`.** It is valid Rust but reads badly. Move the block
    above the attribute; the lint only needs the section to exist.
  - **`u32` has no `From<u32> for usize`.** `usize::from(signum.cast_unsigned())`
    does not compile on macOS. Use `usize::try_from(signum).unwrap_or_default()`
    when the value is a positive signal number.
  - **`Path` and `PathBuf` do not implement `Display`.**
    `unnecessary_debug_formatting` asks for `.display()`, not `{}`; writing
    `{disk_path}` fails to compile.
  - **Keep the cast semantics of the 64-bit shim.** `cast_signed`,
    `cast_unsigned` and `try_from(...).unwrap_or(MAX)` keep the value for every
    reachable input. A negative `ftruncate` length still becomes a huge
    allocation; that is a separate bug, now in backlog §35.
  - **`once_cell` stays for `ipc`.** The shim's `Lazy` statics moved to
    `std::sync::LazyLock`; `src/shared/ipc/version.rs` still uses `Lazy`, so the
    dependency and the `ipc` feature entry stay. Only `vfs-preload` drops
    `dep:once_cell`.
  - **`clippy-runs` lists only the trees that are at zero.** The package keeps
    its other `runs` feature sets; adding them to `clippy-runs` without
    measuring them would make CI fail on debt this feature did not cover.

- 2026-11-05 (feature 111, ptrace `O_TRUNC` and `dup2` destination close):
  - **A virtual fd must own a number whose kernel fd is closed.** With that invariant the
    `dup2` destination close is exact: a real fd at the new number is closed by an injected
    `close(new_fd)`, and a number that already holds a virtual entry needs no kernel call.
  - **To replace a syscall, rewrite `orig_rax` and the argument registers at the entry
    stop, then set `rax` at the exit stop.** The kernel dispatches on `orig_rax` after the
    ptrace entry stop, so the rewrite runs at once. The old `Skip` could not do this: it
    left the destination open.
  - **An open dispatch test does not need a traced child.** `process_vm_readv(pid, ...)`
    on the test's own pid reads the test's memory, so `on_syscall_entry` runs the whole
    open path (path read, flags, VFS calls) on any Linux kernel, including the emulated
    one.
  - **`std` opens files with `O_CLOEXEC`; `libc::pipe` does not.** A test that hands a real
    fd to a traced child must use a pipe or clear `FD_CLOEXEC`: a `std::fs::File` fd is
    gone after `exec`.
  - **The shell `read` builtin waits for a newline.** One arbitrary byte does not release
    it, so a test that blocks a child on `read` must write `\n`.
  - **`-D warnings` is not a gate for this crate.** The crate carries about 615 clippy
    warnings under `vfs-native,vfs-ptrace`, tracked in backlog §35, and
    `ci/packages.toml` does not deny warnings for it. Read the crate's CI config before
    you treat a `-D warnings` failure as a defect of your change.

- 2026-11-05 (feature 108, deterministic `Expect: 100-continue` timer test):
  - **A timer test must assert on the clock it injects, not on `Instant::now()`.**
    The engine `Ctx` clock reaches the task through `next_status(ctx)`, but no code
    outside `foundation_core` can replace it: `Ctx::clock()` returns a private field and
    `ThreadRegistry.root` is private. A request `Arc<dyn Clock>` in its `Extensions` is
    the remaining typed seam in `foundation_netio`. The production path still reads
    `ctx.clock()`; only the test sets the extension.
  - **Advance the simulated clock only after the client has fixed its deadline.** The
    client computes `send_body_at = clock.now() + timeout` in the same poll as the head
    write. If the server advances first, the deadline moves. The test clock counts
    reads; the server waits for the first read before it advances. This removes the
    race in both arrival orders.
  - **Parking on a readiness that cannot wake is re-checked on the readiness quantum.**
    `SocketReadiness` does not implement `register`, so `AnyReadiness::register` returns
    `false` and the task is re-polled every 1 ms. That is what lets an injected clock
    fire before the real deadline passes.
  - **A wall-clock failure needs a scheduling stall, not CPU load.** Eight busy CPU
    loops did not reproduce the 2 s bound failure. `SIGSTOP` of the test process for
    2.5 s did, on the first try. Use `kill -STOP`/`-CONT` by PID to prove a wall-clock
    dependence.
  - **A simulated clock still maps to a real `Instant`.** `std::time::Instant` has no
    public constructor from an arbitrary value, so the test clock anchors at one real
    `Instant::now()` and moves forward from it. The executor still sleeps to the mapped
    deadline, so the test keeps the timer's real duration. Only the assertion is
    simulated, and that is the flaky part.

- 2026-11-05 (status close-out after the PR #179 merge):
  - **Read job conclusions, not the run status.** A run stays `in_progress` while one
    job is stuck. Run 42040473182 has 111 jobs `success`, 0 `failure` and 1 job at
    `in_progress`: `foundation_wireguard` stopped at `Free runner disk` before any build
    step. The run never completes, and the web page shows it as running a long time.
    Use `gh run view <id> --json jobs` and read each `conclusion`.
  - **A CI proof belongs to the job that runs the package.** A package job runs every
    feature set in `runs` from `ci/packages.toml`. The `foundation_nativeapis` job green
    proves the `vfs-native,vfs-ptrace` spawn suite only because that set is named in
    `runs` and `EWE_REQUIRE_PTRACE=1` fails instead of skipping. Check both before you
    accept the proof.
  - **A host limit can block a repeat count.** Feature 47 asked for 20 runs of the
    ptrace spawn tests. The Apple Silicon host cannot run one, because the amd64
    emulation returns `EIO` from `PTRACE_GETREGS`. The native CI job gives one run of
    the full suite, and the feature file records that limit in place of the count.

- 2026-11-05 (PR #179 CI fix pass, fourth round):
  - **`docker exec` into a virtual machine image reaches the Linux host, not the guest.**
    Commands such as `sw_vers`, `cargo` and `cmd` run on Ubuntu. A green step proves only
    that the Linux side answered. The log line `default host tuple is
    x86_64-unknown-linux-gnu` is the tell that a "macOS" step ran on Linux.
  - **Docker answers a missing bind source with an empty directory.** A wrong absolute
    path in `docker-compose.yaml` is silent, and the container starts with an empty
    `/shared`. Take mount sources from environment variables with developer defaults,
    then assert a file from the mount inside the container.
  - **`--lib` with a name filter can select zero tests and still pass.** `cargo test -p
    foundation_testbed --features docker-tests --lib -- docker` matched no library test,
    because `docker_tests` is an integration test. Use `--test <file>` and read the test
    count in the log.
  - **A cold runner needs a host-side check, not a guest boot.** A guest install takes
    30 minutes or more and cannot fit a pull request. Check what the host can prove in
    two minutes, and leave in-guest builds to the provisioned images that accept SSH.

- 2026-11-05 (PR #179 CI fix pass, third round):
  - **A generator that writes manifests is a second source of versions.** The Cloudflare
    build script regenerates group crates when its output is stale, and it compares the
    whole output, manifests included. `write_group_crate` hard-coded
    `foundation_netio = "0.0.4"`, so the committed manifests (0.0.6) were stale by
    definition: every build rewrote 42 files and the next `cargo metadata` failed. Read
    the version from the sibling crate's own `Cargo.toml`. Any version literal in a
    generator is a bug waiting for the next version bump.
  - **`Codegen::check` renders into a scratch tree, so the output tree holds no
    siblings.** Reading the sibling versions from next to the output works in `write()`
    and fails in `check()` with `No such file or directory`. A generator that reads
    anything besides its own output must take that source as an explicit input; the
    caller that stages a scratch tree passes the shipped path. A test that runs the real
    `check()` (`the_full_cloudflare_check_is_current`, about 13 s) finds this without a
    CI round trip.
  - **A CI step boundary hides a build-time file rewrite.** `ci.py` calls
    `cargo_metadata()` at the start of every step, so the `build` step passed (manifests
    were current when it started) and the `clippy` step failed (the build script had
    rewritten them). When two consecutive steps disagree, look for a build script that
    writes into the source tree, and compare `git status` inside the mount after the
    build.
  - **`toml_edit`'s `Index` panics when the key is absent.** `doc["package"]["version"]`
    panics with `index not found` on a manifest with no `[package]` table. Use
    `get`/`and_then` so a missing key becomes the typed error the signature promises.
  - **`docker compose` names a container `container_name`, not the service name.** The
    macOS job ran `docker exec macos` against a container called `ewe_macos`, so every
    step failed and the wait loop hid it for 20 minutes. A boot loop must check
    `docker inspect -f '{{.State.Running}}'` and print `docker logs --tail 100` when the
    container is not running.
  - **A `tauri` Linux build needs the GTK and WebKit development packages.** Without
    them `glib-sys`'s build script stops with `Package 'glib-2.0' not found` — the error
    names `glib`, and the cause is a missing image dependency. The list verified on
    Ubuntu 26.04 is in `.github/workflows/platform-docker-tests.yml`.
  - **A wasm wall-clock ceiling must tolerate how late a JS timer fires.** A `Delayed`
    park asks the host for one timer per tick, so a timeout over a 10 ms ticker waits on
    six timers and every one's lateness adds up. The idle machine ends that case at about
    63 ms against a 90 ms bound; the loaded CI container took 126 ms. Keep the lower
    bound and the poll bound tight — they do not depend on load — and widen only the
    ceiling, with the measured CI number in the doc comment.

- 2026-11-05 (PR #179 CI fix pass, second round):
  - **A facade crate can link a second copy of a native library.** `deno_core` 0.404 names
    the engine itself (`v8` 149.4.0) and needs no `deno_v8`. `deno_v8` 0.4.0 is a facade
    whose `v8` feature pulls `v8` 150.4.0. With both in the tree the linker reports
    `duplicate symbol: v8::internal::...`. Remove the unused dependency; do not try to
    unify the versions. `cargo tree -i v8@150.4.0` names the only parent in one command.
  - **A test helper that returns a cached artifact can return the wrong build.** The
    `LD_PRELOAD` shim is a `cdylib`, and `cargo test` does not build it. `find_shim_so`
    looked for the `.so` and returned the first match, so a file built earlier with a
    smaller feature set was used unchanged, and the child process failed with a bare
    `ExitStatus(256)`. Always run `cargo build --lib` with the features the test needs
    before the lookup: cargo does nothing when the artifact is current, so the cost is
    one second, and the artifact is correct when it is stale. Build once per test binary
    (`OnceLock`) so two tests cannot overwrite each other's cdylib.
  - **A version requirement behind the crate version breaks every patched resolution.**
    Feature 82 moved `foundation_netio` to 0.0.6; 42 `foundation_cloudflare_*` crates
    still required `0.0.4`. The workspace build tolerates it where the dependency is a
    path dependency of a workspace member, but `bin/platform/tests/templates_build`
    patches crates.io with local paths and then cannot select a version. `cargo metadata`
    fails the same way. Scan with `cargo metadata --format-version 1` and compare each
    path dependency's `req` with the target crate's version. Do not trust a text scan of
    the manifests: a `version = "..."` line at column 0 can belong to a dependency table.
  - **A captured subprocess hides its own failure.** `ci.py` ran
    `cargo metadata` with `capture_output=True` and let the `CalledProcessError`
    escape, so CI printed a traceback with exit 101 and no cause. Raise a `SystemExit`
    that carries `stderr`.

- 2026-11-05 (PR #179 CI fix pass, two root causes worth remembering):
  - **A signal is not a timeout.** `epoll_wait`, `kevent` and the io_uring submit calls
    return `EINTR` when a handler catches a signal. Returning `Ok` with no events turns
    that into "the timeout expired". Every readiness wait must keep the deadline and
    resume for the time that is left. A test with a real `SIGUSR1`, a handler without
    `SA_RESTART`, and a 150 ms budget proves it in one second.
  - **rustc's cdylib version script decides what `LD_PRELOAD` can see.** For a `cdylib`
    rustc passes a generated linker script of the form `{ global: <crate symbols>;
    local: *; };`. A symbol that comes from a linked C object, or from `global_asm!`,
    is not in that list and is localized. `--dynamic-list` and
    `--export-dynamic-symbol` do not override it. To export a name from a Rust cdylib,
    the crate must define it: write the function in Rust with `#[unsafe(no_mangle)]`.
    Rust also refuses a `no_mangle` definition whose signature differs from the known
    libc one (`invalid_runtime_symbol_definitions`); a fixed-arity replacement for a
    variadic libc function needs an `#[allow]` and a written ABI argument.
  - Cargo's new build layout breaks "exe parent is the profile directory" arithmetic.
    Find the profile directory by walking the executable's ancestors to the one that
    holds `.cargo-lock`.
  - **A code-scanning alert lives in a code region, not only on a line.** Feature 67
    rewrote the assertion around a fake password in `foundation_keychain/tests/identity.rs`.
    The sink did not change, but GitHub closed the old alert and opened a new one, so the
    `CodeQL` check failed on a construct that `master` already carries. The repository's
    convention is to dismiss a test fixture with the built-in reason "used in tests"
    (seven alerts already use it). Dismiss it, and say why in the comment.

- 2026-11-05 (feature 84, a closed `ServeOutcome`): a type that can express a nested
  future forces every caller to handle a case that no caller uses. The server logged
  and closed on that case, and one test pinned the accident. Replace the future's
  result with an enum that has no `Async` variant: the rule moves to the compiler and
  the unreachable arm disappears. Keep `ConnectionResult` for the synchronous
  `Serve`/`ServeWriter` path, and add `ConnectionResult::split` so a test wrapper
  handles both cases with two arms, not four. `ServeFuture` is a type alias, so a
  changed output type does not rename any identifier. Do not bump `foundation_http`
  again when feature 82 already set 0.0.5 in the same unreleased PR. Two pre-existing
  problems blocked the local runs: `foundation_connectrpc`'s `arrow_codec_tests` and
  `examples/codecs` do not compile after feature 67 changed `IpcResult` (backlog §1),
  so run that crate with `--lib --test <name>` and record the rest; and the proxy
  verify command in the feature doc named a `multi` feature that `foundation_proxy`
  does not have. Confirm a feature name with `cargo metadata` before you put it in a
  verify command. The macOS failures in `foundation_proxy` and `foundation_keychain`
  are already in the backlog (§9 and §22) and are not caused by this change.

- 2026-11-05 (feature 82, async SSE event writes): an SSE event is a complete unit.
  A synchronous `writeln!` on a non-blocking socket can write a prefix and return
  `WouldBlock`, so the client never sees the dispatch blank line. Render the event to
  bytes first, then write the whole buffer through `ResponseWriter`; it parks on
  readiness and resumes at the accepted offset. Keep one renderer: `EventWriter` and
  the async `SseWriter` must call `SseEvent::render`, or the two paths drift.
  `SseStream::new` must keep the head `ResponseWriter` for the events. A second writer
  on the same stream loses the idle-timer and readiness state that the head write
  started. The browser broadcaster is synchronous (`BroadcastTx::push` runs on the
  App's thread), so an async `FrameTransport` is not possible without changing that
  call chain. Use a bounded valtron `Pipe` and one driver task per client. The queue
  depth is configuration; a full queue makes `send_*` return `false` and the
  broadcaster drops the client, which then reconnects and replays the backlog. Do not
  silently drop a frame. `foundation_browser` has 56 pre-existing clippy errors in
  files feature 82 did not touch (backlog §33); its `test/stream.rs` is clean.
  Measured before/after: the synchronous path returned `WouldBlock` after 10 event
  bytes on a 64-byte fake socket; the async path delivered all 16406 bytes to a client
  reading one byte per poll.

- 2026-11-05 (feature 83, ptrace on native amd64 CI): a shell redirect does not keep the
  virtual fd number. `echo x > /virtual/y` makes dash `dup2` the virtual fd to fd 1, so a
  dispatch guard such as `VirtualFdTable::is_virtual_fd(fd)` (fd >= 10,000) no longer finds
  it. The write then passes through to a kernel fd that does not exist and fails with
  `EBADF`. Dispatch must use the `VirtualFdTable` entry as the source of truth, and `dup2`
  must copy the entry for the new number. The probe's pure decision function must sit outside
  the `target_os = "linux"` gate, or a macOS host cannot test it: gate the probe test and its
  `mod common;`, not the whole file. In the forked child, exit at once if `PTRACE_TRACEME`
  fails: an untraced `SIGSTOP` hangs the parent. Always `PTRACE_DETACH` and `waitpid`, so a
  failed probe leaves no stopped process or zombie. Do not add a skip helper for the ptrace
  suite: nothing calls it, and the spawn tests must fail with the errno, never skip. Two
  dispatch bugs remain and are in the backlog: `O_CREAT` on an existing virtual file returns
  `EEXIST` instead of truncating, and a virtual `dup2` over a real fd leaves that real fd
  open.
- 2026-11-05 (features 82 and 83, recorded from owner reports): an empty test binary passes.
  `ptrace_vfs/mod.rs` is gated on `target_os = "linux"` plus two features, so a wrong feature
  set in `ci/packages.toml` builds nothing and CI stays green. Give such a suite a named run
  and a probe target that asserts when `EWE_REQUIRE_*` is set (the `io_uring_probe` pattern).
  The ptrace suite is Linux-only: macOS has no Linux `ptrace` and this workspace has no macOS
  backend, so a macOS runner cannot cover it, amd64 or not. The native amd64 kernel that the
  OrbStack `PTRACE_GETREGS` `EIO` failure needs is the GitHub `ubuntu-latest` x86_64 runner,
  which runs the linux/amd64 image as a container on its own kernel.
  SSE has the same truncation hazard as `respond::*` had: an `EventWriter` over a non-blocking
  socket uses `writeln!` and drops the rest of an event on `WouldBlock`. Rendering to bytes and
  writing through `ResponseWriter` is the fix; keep one renderer so the wire format does not
  fork.
- 2026-11-05 (feature 67, one error standard): `ErrorTrace<C>` needs
  `C: Error + Send + Sync + 'static`. A crate gets `?` for free only after it
  adds `impl From<CrateError> for ErrorTrace<CrateError>`; foreign source
  errors (`std::io::Error`, `ArrowError`, `notify::Error`) cannot convert this
  way because both types are foreign (orphan rule), so each call site maps
  them through the crate error first (`.map_err(CrateError::from)?`). An
  explicit `Err(CrateError::Variant)` needs `.into()`; a tail expression that
  already returns `Result<_, CrateError>` needs `.map_err(Into::into)`.
  `derive_more`'s `Error` derive treats a single tuple or named `source` field
  as the source; a `String`, `PathBuf` or `Version` must be marked
  `#[error(not(source))]` or the derive fails. For an error enum that carries
  generic event or request values, the derived `Debug`, `Display` and `Error`
  impls add `E: Debug`/`E: Display`/`E: Error` bounds that the domain types do
  not satisfy. Write `Display`, `Debug` and `Error` by hand with only the
  bounds the payload's `Display` needs (`E: Clone`), as `DomainOpsErrors<E>`
  does. `ErrorTrace` is also `Sync`-bound, so the `ewe_domain` traits had to
  add `Sync` to their `Events` and `Requests` associated types. Match on
  `trace.current_context()` when a caller used to match the plain enum; the
  keychain servers call `current_context().status()`/`.to_json()` for the same
  reason. Two tests fail on `spec70/io-hardening-final` with these changes
  reverted: the proxy websocket upgrade test and the keychain streamed-body
  timing test (both macOS `WouldBlock`). Do not report them as regressions.
- 2026-11-05 (feature 63, retiring the sync response path): a test that must
  prove a writer parks needs a transport whose readiness the test controls.
  Loopback TCP did not work: with `SO_SNDBUF` and `SO_RCVBUF` set to 4 KiB,
  macOS still queued 327,680 bytes before the first `WouldBlock` (the kernel
  auto-tunes), so the write finished instantly and the old writer passed.
  A `UnixStream::pair()` queued 4,096-8,064 bytes and blocked, but it failed on
  Linux: `unix_writable()` only reports writable when the queue is at most 25%
  full, so a client reading 512 B does not make the fd writable (and epoll
  therefore never fires `EPOLLOUT`) for seconds. The test then timed out while
  draining. The final design is a fake in-memory `OverlayReadWrite` transport:
  a `VecDeque` with a fixed capacity, a `WouldBlock` write when it is full, and
  a `ConnWaker` the test client fires after every read. It behaves the same on
  macOS and Linux and exercises the overlay readiness path.
  A send timeout that starts at writer creation is only observable when the
  *first* write of the operation blocks: `write_all` already restarted the time
  on every successful partial write, so an idle writer whose first write
  succeeds never showed the bug. Pre-fill the transport before creating the
  writer and start the client reader only after the writer blocks.
  `BufWriter::flush` on a non-blocking socket can drain part of its buffer and
  then return `WouldBlock`, so a `flush` that only waits on readiness silently
  makes progress. Measure the buffered length before and after (`RawStream` now
  exposes it) and restart the idle timer when it shrinks.
  Moving a task-driven response to an async writer inside a `TaskIterator`
  needs a state that owns the `ServeFuture`. The existing `Awaiting` state can
  serve closing writes by setting `should_close: true`; a `100 Continue` needs
  its own state because success continues to `WaitingForBody` instead of closing.
  `block_on_future` is the right tool at a blocking boundary only when the pool
  exists; a branch that runs *because* `valtron::send` failed has no executor,
  so it cannot drive an async write (`block_on_future` panics without a pool).
  Log and close there. Inside a task, spawn a `FutureTask` (protocol detection)
  so the guard and the connection clone outlive the detector.
  Poll an `Awaiting` future **before** checking the shutdown flag. The old order
  dropped the future first, which truncated any handler whose remaining work was
  the response write; polling first lets it finish, while a future that is still
  `Pending` is still dropped at the drain checkpoint (the feature-08 test needs
  that). This is why `completes_inflight_request_within_grace` failed until the
  order changed.
  Clearing `clippy -D warnings` on `foundation_http` needed the multi-gated test
  targets too: CI's clippy run only uses default features, so ~34 warnings in
  the `multi`-only server tests were never seen. Run clippy with `--features
  multi` before claiming the crate is clean. `assert!(x.is_empty())` now fails
  `assert_is_empty`; `assert_ne!(x.len(), 0)` avoids it.
- 2026-11-05 (feature 59, public FFI callback): moving an inline test out of
  `infrastructure_llama_cpp` made `logs_to_trace` public. Clippy then failed with
  `not_unsafe_ptr_arg_deref` (deny by default in `clippy::correctness`): a public
  function takes a raw pointer and dereferences it. The lint only looks at
  public items, so private code never saw it. The fix is to mark the callback
  `pub unsafe extern "C" fn` and document the pointer contract, because the
  `ggml_log_callback` binding type is already
  `Option<unsafe extern "C" fn(..)>`; the two signatures match. The tests keep a
  local safe wrapper. A clippy failure in a package job can come from a path
  dependency, not the package itself: read the `-->` path before you search the
  package.
- 2026-11-05 (feature 49, warning cleanup): `cargo clippy --all-targets` lints
  every test and example. A `clippy-deny-warnings = true` entry in
  `ci/packages.toml` therefore needs zero warnings in the test targets too, not
  only in the library. `foundation_core` has 327 test/example warnings
  (`drop_non_drop`, `arc_with_non_send_sync`, `items_after_statements`,
  `doc_markdown`), so its deny entry is held back. Check the all-targets set
  before you add deny, not just `--lib`.
  `cargo clippy --fix` rewrites generated code. The generator is the real source:
  if it still emits the old shape, a regeneration restores the warnings.
  A manifest warning can mean a feature enables a dependency without the code
  that uses it: `compression` enabled `flate2` but not `gzip`/`deflate`.
  An `unused dependency` warning that appears only on one target is fixed by
  target-gating the dependency, and a dependency that must stay unused needs
  `use crate as _;` (or a target gate plus an underscore import) to say why.
- 2026-11-05 (feature 50, test-only constructors): an inline `#[cfg(test)]` module
  can use private items and test-only constructors that an integration test
  cannot see. Move the test to the public API first; when no public path exists,
  make the smallest item public and document it. The `ParseErrorKind` enum and
  `ParseError::kind` field became public for one test, and a test-only
  `Generator::for_testing` became a local helper over the public constructor.
  A target-gated module can be absent from `lib.rs` and still be referred to by
  a completeness gate: `foundation_wireguard`'s `ios` and `android` modules were
  dead for that reason. Search for the module name before you assume it compiles.
- 2026-11-05 (feature 48, ignored condvar tests): the two tests were marked
  `#[ignore = "Requires threading support"]` under `#[cfg(feature = "std")]`. With `std`
  the conditions are always true, so the reason was impossible. Both tests pass with no
  change to the condvar. Lesson: an ignore whose reason contradicts its `cfg` is a stale
  marker; run it before you look for a bug.
- 2026-11-05 (feature 47, ptrace deadlock): two bugs made the seven spawn tests hang.
  (1) The child was forked by the test thread, then the `ptrace_loop` ran in a new
  thread. A ptrace request must come from the thread that attached to the tracee. From
  another thread it fails with `ESRCH`: `PTRACE_GETREGS` returns nothing and the
  `PTRACE_SYSCALL` resume fails, so the tracee stays stopped and `waitpid` blocks. The
  tracer thread now forks the child and runs every ptrace request, and it sends the child
  pid to `spawn` through a channel.
  (2) `nix::unistd::execvp` does not add `argv[0]`. The code passed `args` only, so
  `/bin/sh -c "..."` got `argv[0] = "-c"` and never ran the command. Build `argv` as
  `[command, args...]`.
  Use `PTRACE_TRACEME` plus `raise(SIGSTOP)` in the child, not `PTRACE_SEIZE`: with SEIZE
  the child can exec before the attach, the tracer sees the `EXEC` event in the wrong
  place, and a forked child starts in `PTRACE_EVENT_STOP`, which `PTRACE_SYSCALL` does not
  resume. Build every `CString` and the null-terminated pointer array before `fork`; the
  child may call only async-signal-safe functions.
  Tests run in parallel in one binary. `waitpid(-1)` from one tracer reaps another test's
  tracee and then `PTRACE_GETREGS` fails. Add `__WNOTHREAD` next to `__WALL`.
  Local limit: OrbStack amd64 emulation returns `EIO` from `PTRACE_GETREGS` for every
  traced amd64 process (a minimal C program shows it too), so the spawn tests fail here
  before they reach the dispatch code. They must run on native amd64 in GitHub CI.
- 2026-11-05 (feature 80, "Thread pool not initialized" in CI): `PoolGuard::shutdown`
  cleared the global `REGISTRY` and `BG_REGISTRY` slots unconditionally. A guard from
  `PoolGuard::new` owns a private registry, takes no lifecycle gate, and its tests run
  in parallel with every other plain `#[test]` in the same binary (for example
  `test_poolguard_drop_triggers_shutdown` next to `can_queue_and_complete_task`). Its
  drop therefore wiped the live pool of another test. In `block_on` the window is
  between `initialize_pool` (installs the registry) and `get_pool` (reads it), so the
  panic was "Thread pool not initialized, ensure to call block_on or initialize_pool
  first", and it looked like a random flake. The fix compares the `Arc` and clears a
  slot only when it still holds the registry this guard owns; the `PoolGuard` docs
  already promised this "generation still matches" rule. Test:
  `private_pool_guard_drop_keeps_the_global_pool` in `tests/valtron/multi_workers.rs`.
  Lesson: a process-wide singleton needs an owner check on every teardown path, and a
  guard that did not install a slot must never clear it.
- 2026-11-05 (feature 80, ignored doctests): `foundation_core` marked 57 doc examples
  `ignore`, so `cargo test --doc` reported them as ignored instead of running them.
  They were pseudo-code (`MyTask::new()`, undefined types), and `ignore` hid that the
  documented API was never compiled. All 57 are now real doctests: each builds a small
  local task or a plain `Vec<Stream<..>>` iterator, runs the documented call and asserts
  the result. The default set runs 70 doctests and the `multi` set runs 73; both report
  zero ignored. Write a doctest as an external crate: use only `foundation_core::...`
  paths. A hidden `# let _pool = initialize_pool(7, Some(3));` line brings up a pool for
  the `execute` family and has the same signature in both executors.

- 2026-11-05 (feature 80, lost wake in the `Depends` park path): the executor checked a
  readiness with `is_ready(None)` before it called `register`, because a ready signal never
  wakes anybody. A source that became ready between the two calls found no waker: the notifier
  count was zero, so `notify` returned early. The task then slept to its deadline. In the HTTP
  client the deadline is the 30 s DNS timeout, so a lookup that had already finished still
  stalled the request for 30 s. The Linux CI runs showed the stall in three netio tests (the
  Expect-Continue upload timing test and two unsolicited-interim tests) and in two proxy e2e
  tests (`connect_unary_via_h1_proxy`, `grpc_web_unary_via_grpc_backend`).
  The fix registers first and checks again. It runs that second check only when `register`
  returned `true`: a signal that did not take the waker keeps its timed re-check. When the
  signal is ready at once, the handler releases the park state before it unparks the task, so a
  source that still holds the waker cannot park the task again. Do not remove the second check:
  the single check before `register` is the bug.
  Test: `tests/valtron/readiness_register_race.rs`. Its readiness becomes ready inside
  `register` and does not wake the waker, which is the exact window. The old code ran the task
  after 505 ms (the deadline); the new code runs it after 0 ms with two readiness checks. It
  runs on the single-thread executor and on a pool.
- 2026-11-05 (CI image and submodules): master commit 2512754fa deleted the
  `tools/whisper.cpp` gitlink but left its `.gitmodules` entry and the `WHISPER_DIR`
  setting in `mise.toml`. GitHub checks out a pull request as its merge commit, so the
  deletion reached the image job, and `images.nu prepare` stopped with "submodule
  tools/whisper.cpp is not in HEAD". A branch that does not change the path cannot stop a
  deletion in master from landing in the merge. To keep a submodule, merge master into the
  branch and re-add the gitlink (mode 160000) in a commit on top, which makes the path a
  change that the merge keeps. Check with `git merge-tree --write-tree HEAD origin/master`
  and `git ls-tree <tree> tools/whisper.cpp` before you push.
- 2026-11-05 (Docker runs of `foundation_core` on an Apple Silicon Mac): the tests measure
  the CPU time of idle threads with a bound of 2 ms per second. An amd64 container on an arm64
  Mac runs under emulation, and `idle_threads_parked_on_a_resume_signal_use_almost_no_cpu`
  fails there with 2.3 to 3.1 ms. The old code fails too (2.44 ms), so the failure is the
  emulation, not a regression. Judge CPU-time tests by the native GitHub job or by a native
  run. The same host runs the test 10 times out of 10 green.
- 2026-11-02 (feature 39, rustfmt version pin): the nightly channel name and
  the rustfmt commit date differ by one day. The CI image's rustfmt is
  `1.10.0-nightly (feaadeeaca 2026-09-19)`. The channel that gives that
  rustfmt is `nightly-2026-09-20`. The date in the version string is the
  commit date, not the channel name. Pin the channel, not the commit date.
  `cargo fmt --all` does not touch `crates/*` (they are excluded), so each
  standalone crate there needs its own `cargo fmt`. rustfmt needs two passes
  on a few long `format!` calls inside `match` arms: the first `cargo fmt
  --all` left one file flagged and a second run settled it. Run `cargo fmt
  --all` and then `--check` until clean. The first `--check` counted 7292
  `Diff in` lines. That is diff hunks across 1372 files, not 7292 files.
  Generated code must stay byte-identical to the generator output: the
  Cloudflare per-group build script regenerates on any difference, so those
  43 shims are excluded with a `rustfmt.toml` `ignore` list, not reformatted.
  The feature 08 learning "the repo is not fmt-clean" is now resolved.

- 2026-11-02 (feature 19 E4, foundation_nativeapis): the local `turso`
  engine (crate `turso` 0.8.3) can back a local delta store. It is not the
  same as `LibsqlDelta` (remote Turso through the `libsql` 0.9 crate). The
  two crates differ. `turso` `Statement::query` and `Statement::execute`
  take `&mut self`; libsql consumes the statement. `turso` `execute_batch`
  and `execute` fail with "unexpected row during execution" on any statement
  that returns rows; libsql discards those rows. `PRAGMA
  wal_checkpoint(TRUNCATE)` and the four open PRAGMAs (`journal_mode = WAL`,
  `synchronous = NORMAL`, `foreign_keys = ON`, `busy_timeout = 5000`) all
  return rows. Run them with `conn.query` and drain the rows. The PRAGMA
  side effect happens on the first step; an undrained `Rows` does nothing.
  Depend on `turso` with `default-features = false`; its default features
  pull in `mimalloc` (a global allocator). `turso` is native-only, so gate
  the dependency and module behind `cfg(not(target_arch = "wasm32"))` (the
  same as `foundation_db`). The `turso` async chain overflows the default
  `recursion_limit` while proving `Send` through the `async_trait` boxed
  futures; set `#![recursion_limit = "256"]` on the crate root and the test
  file (the same root cause and fix as feature 35). For the pedantic clippy
  casts, use `cast_signed()` and `cast_unsigned()` (stable since Rust 1.87)
  for same-width sign changes, `i64::from` and `u64::from` for lossless
  widening, and `try_from(...).unwrap_or(0)` or `.expect(...)` for the rest.
  The `uid` and `gid` local bindings trip `similar_names`; allow it on
  `from_row`.

- 2026-11-02 (feature 19 E10, foundation_nativeapis):
  `foundation_core::blocking_boundary!` evaluates `foundation_core`'s own
  `js-wasmbindgen`/`js-foundation-wasm` features (after feature unification),
  not the dependent crate's `cfg`, so a dependent crate must not write its
  own `cfg(wasm ...)`; it just wraps items in the macro. The macro keeps
  items on native and plain wasm and drops them on JS-hosted wasm. Put `///`
  doc comments INSIDE the macro, not just before it — a doc comment on a
  macro invocation trips `unused_doc_comments`. The macro wraps a whole
  module body and also a single method inside an `impl` block. When you gate
  a module, gate every reference too: `pub use` re-exports, crate-root
  re-exports, and callers (here `DynFs::from_async`). Move imports that only
  the gated code uses inside the macro, or the JS-hosted wasm build warns
  about unused imports. To verify a JS-hosted wasm build of a crate that
  does not forward the js features, use a scratch crate outside the repo
  that depends on both the crate and `foundation_core` with
  `js-wasmbindgen`, and set `RUSTUP_TOOLCHAIN` to the repo toolchain: outside
  the repo the `rust-toolchain.toml` override does not apply, and stable
  cargo then fails on the `CARGO_PROFILE_DEV_CODEGEN_BACKEND=cranelift` env
  var.

- 2026-11-02 (feature 19 E9, foundation_nativeapis): a per-store copy chunk
  size is a defaulted trait method `copy_chunk_size()` (32 KiB, Go
  `net/http` `io.Copy`), not a const; a config-backed store overrides it.
  `read_at`/`read_at_async` may return fewer bytes than asked at end of
  file, so the copy loop must advance by the returned count and stop on zero
  (or an empty `Vec`), never assume a full read. A counting VFS that observes
  read sizes must wrap the `File` type (reads happen on the file handle);
  changing `File` forces the `Directory` associated type to change too, so
  stub the unused `Directory` with `unimplemented!()` methods, because the
  default `copy` never opens a directory. `usize::try_from(...).unwrap_or(usize::MAX)`
  avoids the clippy `cast_possible_truncation` warning when a `u64` size is
  narrowed to `usize`.

- 2026-11-02 (feature 19 E8, foundation_nativeapis): the NFS v3 READDIR
  cookie is the previous entry's `fileid`, so `start_after` must be matched
  against the entry fileids, not against an index into the raw list. Resolve
  every entry's fileid in list order before paginating, so the ids equal the
  cookies the client sends back. `MemoryFs` lists children in name order, so
  paging through it is deterministic. nfsserve's `nfstime3.seconds` is 32
  bits; convert 64-bit seconds with
  `u32::try_from(d.as_secs()).unwrap_or(u32::MAX)` to saturate without the
  clippy `cast_possible_truncation` warning. In one test binary, manual
  `initialize_pool` and `#[valtron_test]` cannot coexist (the valtron pool is
  process-wide), so tests added to a file that already uses manual
  `init_pool` must reuse the file's manual `run_*_async` pattern.

- 2026-11-02 (feature 19 E7, foundation_nativeapis): when one type implements
  both `VfsFileSystem` and `AsyncVfsFileSystem`, `capabilities` is the only
  method name the two traits share (every other async method has an `_async`
  suffix). A call such as `self.inner.capabilities()` in the sync
  `VfsFileSystem` impl then fails with E0034; qualify it as
  `VfsFileSystem::capabilities(&self.inner)`. The async face is sync-native:
  each async method delegates to the sync trait method. Re-wrap the
  `Box<dyn VfsDirectory>` results of `create_dir` / `open_directory` in an
  `AsyncInodeDirectory` wrapper (the same shape as `AsyncMemoryDirectory`).
  A private helper that is never called (here `InodeDir::new`, replaced by
  struct literals) trips `dead_code`, so delete it. The `#[valtron_test]`
  macro drives an `async fn` directly with `.await`; no manual
  `initialize_pool` is needed.

- 2026-11-02 (feature 19 E6, foundation_nativeapis): `async_trait` boxes the
  `DocumentStore` futures as `Send`, so an async VFS store must never hold a
  `std::sync` lock guard across an `.await` (the guards are not `Send`). Two
  patterns: for writes, mutate the batch and advance the high water mark under
  the lock, drop the guard, then await the VFS write; for reads, scope the
  read guard to a block that returns before the VFS await. The fjall range
  iterator is not `Send`, so a `Vec`-returning helper (`collect_rows`) must
  collect the decoded index entries first and await the document reads
  afterwards. Calling `read_at_async`/`write_at_async`/`sync_data_async`/
  `size_async` on `<V as AsyncVfsFileSystem>::File`, and `list_async` on its
  `Directory`, needs the `AsyncVfsFile` and `AsyncVfsDirectory` traits in
  scope (not just `AsyncVfsFileSystem`), or method resolution fails with
  E0599. `MemoryFs` implements both the sync and async VFS traits, so the
  tests move to it, and the read-open counting wrapper now implements
  `AsyncVfsFileSystem` and counts `open_async(Read)`.

- 2026-11-02 (feature 19 E5, foundation_nativeapis): fjall's `range` returns
  a `!Send` iterator (`Box<dyn DoubleEndedIterator + 'static>` with no `Send`
  bound, from lsm-tree), so it cannot live in an `AsyncStorageItemStream`
  (which is `Send` on non-wasm). Bridge it with a producer `std::thread` that
  owns the iterator and pushes decoded index entries through a bounded valtron
  `Pipe`; the thread blocks on a full pipe with
  `futures_lite::future::block_on(tx.send(msg))` (backpressure, no valtron pool
  needed) and the stream drains with `rx.receive().await` in
  `futures_lite::stream::unfold`. Dropping the stream's receiver closes the
  pipe, which unblocks the producer's send, so the thread exits. The laziness
  test counts `VfsFileSystem::open(Read)` calls through a delegating wrapper
  over `InodeFs`; the collection must be flushed first, because unflushed
  batches are read from memory, not through the VFS.

- 2026-11-02 (feature 25 E3, foundation_deployment_huggingface): a lazy
  pagination iterator over async page fetches must drive each fetch with
  `block_on_future` inside `Iterator::next`. That blocks the calling thread
  and parks the fetch on the valtron pool, so it is safe only with the crate's
  `multi` feature (the default, at least two task workers). The next page URL
  is taken verbatim from the `Link` header, so the mock server builds the
  absolute URL from the request's `Host` header.

- 2026-11-02 (feature 25 E2, foundation_deployment_huggingface): the netio
  client always chunk-frames a `SendSafeBody::Stream` request body — its
  `SimpleIncomingRequest::build()` removes any `Content-Length` and adds
  `Transfer-Encoding: chunked`. So a streamed upload computes and exposes its
  length up front, but the wire uses chunked framing, not `Content-Length`.
  The local `TestHttpServer` reads both sized and chunked request bodies to
  completion before the handler runs, so body-exact assertions still work.

- 2026-11-02 (feature 19 E3, foundation_nativeapis): the shim only compiles on
  Linux (`target_os = "linux"` gate in `native/vfs/mod.rs`), so macOS cannot
  type-check it. `cargo check --target x86_64-unknown-linux-gnu` fails on the
  `ring` crate's build script (no `x86_64-linux-gnu-gcc`), not only on the
  shim's own C file, so a local Linux cross-check is not possible. Validate the
  Linux-only code by manual review plus a macOS `--test vfs_shim_vfs --no-run`
  (the `#![cfg(target_os = "linux")]` file still parses). Also: the D1/R2 shim
  branches must `initialize_pool` and keep the `PoolGuard`, exactly like the
  sqlite branch — `DynFs::from_async` is `SyncFs` over `exec_async`, which
  fast-paths ready-once futures (memory delta) but drives pending HTTP futures
  through the valtron pool. The standard CI `runs` for foundation_nativeapis
  do not include `vfs-preload`, so `vfs_shim_vfs` (and the shim) are not built
  there; the D1 test needs `--features vfs-preload,vfs-d1` and a local
  Cloudflare emulator.

- 2026-11-02 (feature 25 E1, foundation_deployment_huggingface): the async
  seam is `send_async` -> `into_async_parts()` (not `into_parts()`, which drops
  the body's read-wake source). `AsyncSendSafeBody` is `Unpin`, so
  `futures_lite::StreamExt::next().await` drains it without a manual `Pin`.
  Sync callers at the boundary use
  `block_on_future({ let x = x.clone(); async move { f(&x).await } })` because
  the future must be `Send + 'static`. A `#[valtron_test] async fn` returning
  `Result<(), Box<dyn std::error::Error>>` fails the `Send` bound
  (`Box<dyn Error>` is `!Send`); return the concrete error or
  `Box<dyn Error + Send + Sync>`.

- 2026-11-02 (feature 19 E2, foundation_nativeapis): the `d1_delta::chunking`
  module was gated `#![cfg(feature = "vfs-d1")]` inside `d1_delta`, but
  `blob_delta` compiles for both `vfs-d1` and `vfs-r2`, so the chunking logic
  could not live there. Put the chunk slice/assemble helpers in `blob_delta.rs`
  (gated `any(vfs-d1, vfs-r2)`) and delete the unused `d1_delta::chunking`,
  `d1_delta::types` and `r2_delta::types` modules. Also: `MemoryStorage` has no
  byte counter, so `blob_delta_small_writes_touch_one_chunk` needs a small
  `BlobStore` wrapper in the test that forwards every method and counts
  `put_blob` bytes; `foundation_db` re-exports `BlobStore`/`StorageResult` at
  the root but `BlobListStream` only under `foundation_db::traits`.

- 2026-11-02 (feature 19 E1, foundation_nativeapis): `BlobStore::list_blobs_after`
  returns a bare `BlobListStream` (`Pin<Box<dyn Stream<Item = StorageResult<String>> + Send>>`)
  with no `collect_all`, so a consumer needs a `StreamExt` in scope.
  `foundation_db` already depends on `futures-lite`; `foundation_nativeapis` had
  it only behind `vfs-fjall`. Add `dep:futures-lite` to `vfs-d1`/`vfs-r2`
  rather than reaching into `foundation_core::valtron::StreamExt`. Also: D1
  `list_keys` matches with SQL `LIKE`, so `%` and `_` in blob keys widen the
  match; `list_blobs_after` must re-filter with `starts_with(prefix)` to keep
  the literal contract.

- 2026-11-02 (backlog §15 clippy cleanup, foundation_cedar): the
  `assert_is_empty` fix has three more traps beyond the earlier `[] as [T; 0]`
  note. (1) A plain `vec![]` in `assert_ne!(x, vec![])` fails E0282 ("type
  annotations needed") because the empty vec's element type cannot unify through
  the `PartialEq<Vec<T>> for Vec<T>` impl inside the macro; clippy's own form is
  a typed empty array, `assert_ne!(x, [] as [T; 0])`. (2) Clippy emits a
  fully-qualified element path that can name a private module —
  `[] as [cedar_policy::api::id::PolicyId; 0]` does not compile because `api` is
  private; use the public re-export `cedar_policy::PolicyId` instead. (3) For a
  `String`, `assert_ne!(x, "")` compiles fine (no array needed).

- 2026-11-02 (backlog §7 `vms` clippy follow-up, foundation_deployment_platform):
  two more gotchas. (1) rustc and clippy colorize their output, so a `grep
  "^warning:"` matches nothing on a colorized line; a crate can look clean
  while 28 warnings remain. Strip ANSI codes first: `sed
  's/\x1b\[[0-9;]*m//g'`. (2) `#[allow(clippy::cast_precision_loss)]` on a
  `println!`/`eprintln!` macro call is ignored: the cast sits inside the
  macro's generated code, so the allow is reported "unused" and the cast still
  warns. Move the cast to a `let` binding and put the allow on that binding.
  Also: the earlier note to write `.map_while(|l| l.ok())` for the `Result`
  alias is incomplete — that closure form trips
  `redundant_closure_for_method_calls`. Use the fully-qualified
  `.map_while(std::result::Result::ok)` instead.

- 2026-11-02 (backlog §7 `vms` clippy, foundation_deployment_platform): three
  concrete gotchas from the manual pass. (1) `lines_filter_map_ok`'s
  `map_while(Result::ok)` suggestion breaks when a local `type Result<T> = ...`
  alias shadows `std::result::Result`; write `.map_while(|l| l.ok())` instead.
  (2) `assert_is_empty`'s `assert_ne!(x, [])` suggestion does not compile for
  `&[T]` (the empty-array element type cannot be inferred); use
  `assert_ne!(x, [] as [T; 0])`. (3) `format_push_string`/`format_collect` need
  `use std::fmt::Write as _;` for `write!`. Also: moving a test-only optional
  dependency to `[dev-dependencies]` clears `unused_crate_dependencies` without
  any new default-feature warning.

- 2026-11-02 (backlog §7 clippy cleanup, foundation_deployment_platform): CI's
  clippy and build steps run only `runs[0]` of `ci/packages.toml`. For
  `foundation_deployment_platform` that is the default feature set, so the
  `vms` feature tree is compiled only by `cargo test --features vms`, which
  does not lint. A crate can therefore pass CI clippy while a feature-gated
  tree still has hundreds of pedantic warnings. Check both feature sets by
  hand with `--features vms` before calling the crate clean. Also: the
  workspace-wide `fullfp16` rustc deprecation (`.cargo/config.toml`,
  `aarch64-apple-darwin`) is reported once per crate as `generated 1 warning
  (1 duplicate)`, so it looks like a real lint; it is tracked in backlog §10,
  not a code defect. `cargo clippy --no-deps` is what isolates a crate's own
  warnings from its path-dependency warnings.

- 2026-11-02 (backlog §10 flaky-test fix, foundation_ai): the
  `TestHttpServer`-based SSE mock (`sse_response`) sent no `Connection: close`,
  so the mock kept the HTTP/1.1 connection open after the response. The SSE
  client reads a `text/event-stream` body as a line-feed stream until EOF; it
  ignores `Content-Length`. With no EOF the client parked after the buffered
  events, the 100 ms idle timeout fired, and the reconnecting SSE task replayed
  the tool-call deltas. The duplicated deltas made the accumulated arguments
  JSON invalid. Fix: set `Connection: close` on the mock response so the client
  sees EOF and never reconnects. The `json_response` helper already sets the
  same header.

- 2026-11-02 (backlog §10 candle-gemm fix, foundation_ai): the candle CPU
  matrix multiply path uses the pure-Rust `gemm` crate (gemm-common 0.19.0)
  when candle-core has no MKL and no Apple Accelerate. `init_gemm_fn` and
  `init_gemm_cplx_fn` pick the `fma` kernel on `feature_detected!("fma")`
  only. That kernel needs the V3 SIMD type. V3 needs AVX2 (and the features
  AVX2 implies). The Docker Desktop linux/amd64 CPU reports `fma` but not
  `avx2`. `V3::try_new()` then returns `None`, and the `__inject_mod!`
  `.unwrap()` panics. The panic shows at `gemm-f32-0.19.0/src/gemm.rs:3:5`.
  Fix: vendor gemm-common 0.19.0 under `patches/gemm-common`, add
  `[patch.crates-io]`, and require `fma` and `avx2` before the fma kernel.
  macOS still compiles: the check is inside an x86/x86_64 cfg block, and every
  AVX2 CPU also has FMA, so the gate does not change on real hardware.

- 2026-11-02 (backlog §10 wasm32 Send fix, foundation_ai): valtron re-exports
  `sleep_async` from `non_sendables` or `sendables` based on foundation_core's
  `multi` feature. On wasm32 `multi` is off (single-threaded), so `sleep_async`
  is `non_sendables::sleep_async`; its `timer.into_ready_future().await` future
  holds the single-threaded executor and is `!Send`. Awaiting it inside an
  `#[async_trait]` method on a `Send + Sync` trait makes the boxed future
  `!Send`, so the wasm32 `--no-default-features` check fails with three `Send`
  errors. The fix wraps the sleep future in `foundation_compact::SendWrapper`,
  which has a wasm-only `unsafe impl Send` (one thread, so the assertion is
  vacuously true) plus a `Future` impl. On native the same wrapper auto-derives
  `Send` from the inner future, so the code is unchanged there.

- 2026-11-02 (feature 16 E8 follow-up, foundation_ai): the pub/sub `Appended`
  event id must reuse the record's own id, not mint a fresh one. `write()` stores
  a memory record under its own id (`append_promotable_with_id`), so a fresh
  mint in `append()` makes the event id differ from the stored `doc_id`. The
  fix routes `WorkingMemory` / `Observation` / `Reflection` through the record's
  id in the event match, matching `Conversation`. `Retracted` still mints a
  fresh id because `write()` stores it through the non-idempotent `append`, not
  its `id` field; that is a separate gap.

- 2026-11-02 (feature 16 E8, foundation_ai): `append_with_id` /
  `append_promotable_with_id` idempotency is keyed on `(collection key, doc_id)`.
  The `doc_id` must be STABLE across a retry: mint it once in the record
  constructor, never at the write site and never from wall-clock time. The
  `Conversation` arm of `MessageInner::write` was already on `append_with_id`;
  `session.rs` / `agent_loop.rs` only buffer via `MessageApi::append`, and the
  real write is in `MessageInner::write`. The `Retracted` / `FailedAction` /
  `Summary` variants carry no id, so they cannot be idempotent without a schema
  change.

- 2026-11-02 (backlog §13 fuse_vfs fix, foundation_nativeapis): `fuser` 0.15
  (resolved 0.15.1) declares `default = ["libfuse"]` and `libfuse =
  ["pkg-config"]`. A bare workspace dependency `fuser = "0.15"` pulls in
  libfuse, whose build script probes for a system fuse3 via pkg-config and
  panics when it is absent. The pure-Rust mount implementation is the
  non-default path: set `default-features = false` on the WORKSPACE dependency.
  A member-level `default-features = false` on a `fuser = { workspace = true,
  optional = true }` dependency did not take effect (seen with `cargo tree -e
  features`); the override must sit on the workspace dependency itself.
  `fuser`'s public API (MountOption, Session, BackgroundSession, Filesystem,
  the Reply* types) is not feature-gated and compiles the same in pure-Rust
  mode, so only the one workspace line changed.

- 2026-11-01 (feature 13 E1, foundation_deployment_platform): a mock daemon can
  serve a `follow=true` log stream with a plain `Content-Length` response. The
  client reads exactly the declared bytes and then ends the stream, so the mock
  does not need to hold the connection open. `LogFrameDecoder` yields a frame as
  soon as its 8-byte header and payload are complete, so two frames written in
  one `write_all` still arrive as two `LogOutput::StdOut` items.
  `futures_lite::future::race` needs both futures to share one output type; a
  local enum (`FrameOrTimeout`) unifies `frames.next()` and the timeout. A `Fn`
  handler passed to `mock_socket::serve` cannot move an `Arc` into an inner
  `thread::spawn` closure — clone the `Arc` first. `ErrorTrace` exposes its
  context with `current_context()`, not `as_error()`.

- 2026-11-01 (feature 13 E2, foundation_deployment_platform):
  `DockerClient::connect_with_defaults()` over a Unix socket is infallible and
  lazy: it returns Ok even when `/var/run/docker.sock` is missing, because the
  dial happens on the first request. A daemon-availability check must therefore
  ping the daemon (`negotiate_version()`), not just connect. A sync
  `#[valtron_test]` fn body runs on the test thread (not a pool worker), so a
  sync `docker_available()` that wraps `block_on_future` is safe there and in a
  plain `#[test]`, as long as the pool is initialized before the ping.

- 2026-11-01 (feature 13 E3, foundation_deployment_platform):
  `block_on_future` needs a `'static` future, so an async function that borrows
  `&VmProfile` cannot be driven by it. Pass the profile's `&'static str`
  `image_name` by value instead; the future then owns a `Copy` reference and is
  `'static`. The netio `ClientConfig::max_body_size` enforces the body cap at
  transport level: a `Content-Length` larger than the cap is rejected before
  reading, and a chunked body is truncated by `LimitedEOFStreamReader`, so
  `collect_bytes_async` reads a bounded body without a second explicit cap. A
  plain `std::net::TcpListener` on `127.0.0.1:0` serves as a local HTTP mock
  for `send_async` tests under `#[valtron_test]`.

- 2026-11-01 (feature 17 PR 2, verification): the six-crate CI block
  (`foundation_db`, `foundation_auth`, `foundation_deployment_cloudflare`,
  `foundation_nativeapis`, `foundation_vectors`, `foundation_cedar`) and the
  workspace check are green in the dev profile after the disk was freed. The
  only warning is a pre-existing unused `urlencoding` dependency in
  `foundation_deployment_cloudflare` (recorded in backlog §6). Committed as
  `28f951edf`; the Linux CI gate is the remaining check.

- 2026-11-01 (feature 17 PR 2, foundation_db): a migration that rebuilds a table
  cannot be tested by creating the old shape and then running the whole
  migration set. The old `rate_limits` shape has no `window_end`, so migration
  014's `CREATE INDEX ... ON rate_limits(window_end)` fails on it before the
  rebuild migration runs. The test marks every migration except 027 as already
  applied in `_migrations`, then runs the runner, so only 027 runs
  (DROP + CREATE + CREATE INDEX against the new shape).

- 2026-11-01 (feature 17 PR 2, foundation_db): auth tables store timestamps in
  Unix milliseconds (the IdP already writes milliseconds; `created_at` defaults
  are already milliseconds). Migration 026 converts legacy whole-second values
  with `UPDATE ... WHERE col > 0 AND col < 10000000000`, which is idempotent and
  leaves real millisecond values alone. `rate_limits` stays in seconds, so
  `run_full_cleanup` reads both clocks (`unix_now_millis` and `unix_now_secs`).

- 2026-11-01 (feature 17 PR 2, foundation_db): a fixed-window rate-limit record
  is atomic in one SQLite statement with `INSERT ... ON CONFLICT(key) DO UPDATE
  ... RETURNING count`. The `RETURNING` row is read once from the query stream,
  not buffered (rule 4). Workers KV has no atomic read-modify-write, so its
  `RateLimiterStore` stays as an explicitly best-effort option with a doc
  warning; the exact path is D1 (or a Durable Object).

- 2026-11-01 (feature 17 PR 2, foundation_db): `futures_util::future::join_all`
  (0.3) accepts non-`Unpin` async-trait futures (`impl<F: Future> Future for
  JoinAll<F>` has no `F: Unpin` bound). It drives the 50 concurrent
  `record_rate_limit` calls in the E8 test over the local Turso store, which
  serialises writes the way D1 does.

- 2026-11-01 (feature 17 PR 2, foundation_db): wasm stores kept a second
  inherent `*_async` API alongside the traits. E10 makes those inherent methods
  private, removes the unused valtron helper path in `d1_wasm.rs`, and routes
  `storage_provider`, `foundation_auth`, `wasm/session.rs` and the
  `cf-login-app` example through the trait methods. The gate
  `rg -n 'pub (async )?fn \w+_async' backends/foundation_db/src/wasm` is now
  empty.

- 2026-11-01 (feature 17 PR 2, foundation_db): the `examples/cf-*` directories
  are self-contained wasm32 workspaces excluded from the main workspace, so
  `cargo metadata` never lists them. The CI `examples` job is a synthetic
  package: `ci.py load_workspace` adds it after the metadata loop, and
  `commands()` special-cases its name to build each `examples/cf-*/Cargo.toml`
  for `wasm32-unknown-unknown` plus the native `answerme-agent`.

- 2026-11-01 (bug-backlog §13, foundation_reactor): the macOS kqueue waker fix.
  `filt_usertrigger` in XNU copies the `NOTE_TRIGGER` kevent's `udata` onto the
  knote (`kn->kn_udata = kev->udata`), so the returned event's `udata` is the
  trigger's, not the one stored by `EV_ADD`. `register_waker` stored the token as
  the `EV_ADD` `udata`, but `wake` triggered with a null `udata`, so the event
  came back with `Token(0)`. Fix: in `wake`, pass the stored token
  (`waker_token.0 as *mut c_void`) as the trigger `udata`. Verified by the 12
  tests in `foundation_nativeapis --test poll` (dev profile), including the two
  previously failing `waker_wakes_*` tests.

- 2026-11-01 (version bump, workspace): commit `a896ff0bb` bumped `foundation_db`
  to 0.0.5 but missed three consumers (`foundation_deployment_docker`,
  `foundation_deployment_hetzner`, `foundation_deployment_digitalocean`). They
  pin `version = "0.0.4"` on the line after `path` in a multi-line
  `[dependencies.foundation_db]` table, so a same-line grep misses them.
  `cargo metadata` then fails for the whole workspace and every CI gate dies
  pre-flight. Feature 16 added public API (`generate_async`, async
  `embed`/`embed_batch`, the `record`/`hydrate` renames, `max_error_body`) but
  kept `foundation_ai` at 0.0.2. Bump it to 0.0.3 and both consumers
  (`bin/platform`, `apps/answerme-agent`) in the same change. Use a `tomllib`
  scan for version checks; it reads both inline and multi-line `version`
  fields.

- 2026-11-01 (feature 12, foundation_deployment_docker): moving the seven client
  methods onto the generated request/response types changes their public return
  type from JSON to typed structs, so it is a breaking change that needs the
  version bump and the one consumer's `version` update in the same change (rule
  12). The mock-daemon Unix socket under the deep `target/uat` directory exceeds
  macOS `SUN_LEN` (104 bytes), so `docker_restart_container_defaults` fails
  there; it is test infrastructure, recorded in backlog §6.

- 2026-11-01 (feature 20, foundation_nativeapis): `foundation_shell` is a
  nushell stub (only `add`), not a process helper. The CLI searcher therefore
  uses a small in-crate runner: a `BackgroundJobRegistry` offload worker runs
  the blocking `Command`, and the async side parks on a valtron `Pipe`. The
  child's stdout is read line by line with `BufReader::lines` and mapped into
  results, never `read_to_end` (rule 4). Pipe only stdout; leave stderr on
  `Stdio::null` unless you read it, or a full stderr pipe deadlocks the child.

- 2026-11-01 (feature 20, foundation_nativeapis): the macOS mach-port code
  compiled but failed at runtime because it turned a receive right into a send
  right with `MACH_MSG_TYPE_COPY_SEND`. For a port the process owns, the
  correct type is `MACH_MSG_TYPE_MAKE_SEND`. `task_get_special_port` also needs
  the bootstrap-port enum cast to `i32` and a `ptr::addr_of_mut!` output
  pointer. Compile clean is not enough for mach rights; the call returns an
  error the assert then trips.

- 2026-11-01 (feature 20, foundation_nativeapis): the IPC client now receives
  on an async future and matches responses by request id, so it no longer
  holds a lock across a send and its receive. The join and poll timeouts are
  fields on `VfsIpcConfig` (defaults 30 s join, 1 s poll, the poll cited from
  Go `http.Server`); the search limits are fields on `SearchConfig` (50 per
  file, 200 per page, 5 s budget, 30 s scan wait, cited from fff-search and
  Caddy). Rule 8: put them on the config, not in the code path.

- 2026-11-01 (feature 17, foundation_db): the Cloudflare R2 object-listing
  endpoint is absent from the vendored OpenAPI spec that generates
  `foundation_deployment_cloudflare`'s types, so the R2 listing types cannot be
  generated with the rest. They are defined by hand in the new leaf crate
  `foundation_cloudflare_types` (`R2ObjectInfo`, `R2ListObjectsResultInfo`,
  `R2ListObjectsResponse`) to the shape the R2 REST API returns. The D1 `/query`
  types there (`D1QueryResultResponse`, `D1QueryMeta`,
  `D1QueryDatabaseResponse`) match the generated output exactly, so a later
  generator run can replace them without a wire change. The leaf crate has no
  dependency on `foundation_db` or `foundation_deployment`, so both storage
  crates can depend on it (rule 5).

- 2026-11-01 (feature 17, foundation_db): R2 returns object keys in
  lexicographic order, so the cursor-driven listing stream (`list_after`) needs
  no sort and can page by the last key of the previous page. Streaming page by
  page keeps a large R2 bucket's listing from being read whole into memory
  (rule 4).

- 2026-11-01 (feature 24, foundation_testbed): `deno_core` 0.404 hard-codes
  `DefaultReactor = TokioReactor` with no seam to swap it, so its timers, TCP
  and WebSocket need a tokio runtime. `JsRuntime::poll_event_loop` cannot be
  driven from a valtron task on valtron's clock. So `embedded_js.rs` keeps a
  current-thread tokio runtime local to the module, behind the
  `wasm-embedded-js` feature flag, documented as a third-party boundary.

- 2026-11-01 (feature 16, foundation_ai): `futures_lite::future::Zip<F1, F2>`
  stores `Option<F1::Output>` and `Option<F2::Output>`, so `zip(a, b)` is `Send`
  only when both OUTPUT types are `Send`. A timing test that
  `zip(model.generate_async(..), ticker).await` inside a `#[valtron_test]` async
  body failed to compile because `GenerationError` held
  `foundation_core::extensions::result_ext::BoxedError` (a non-`Send`
  `Box<dyn Error>`). The fix: use `SendableBoxedError`
  (`Box<dyn Error + Send + Sync>`) in `GenerationError`, `ModelErrors` and
  `ModelProviderErrors`. The same root cause was hidden in the pre-existing E1
  test; it is a correctness bug, not a test-only one.

- 2026-11-01 (feature 16, foundation_ai): `tests/agentic/mod.rs` was both the
  `agentic_tests` crate root and a `mod agentic` submodule of `tests/mod.rs`
  (the `foundation_ai_tests` crate). A crate-level attribute such as
  `#![recursion_limit = "256"]` therefore fixed one crate and added an
  `unused_attributes` warning in the other.

- 2026-11-02 (feature 16, foundation_ai): the `recursion_depth_exceeding_limit`
  warning in `session_mock_tests.rs` is fixed by splitting the two crate roots.
  `tests/agentic_tests.rs` is now the `agentic_tests` crate root. It holds
  `#![recursion_limit = "256"]` and loads the shared module tree with
  `#[path = "agentic/mod.rs"] mod agentic;`, so the existing module files keep
  their paths. `tests/mod.rs` (the `foundation_ai_tests` crate root, not
  shared) gains the same inner attribute. `tests/agentic/mod.rs` stays the
  shared `mod agentic` list with no inner attribute, so neither crate warns
  `unused_attributes`. The Cargo `[[test]] agentic_tests` path now points at
  `tests/agentic_tests.rs`. Both test crates check without the warning and the
  329 tests pass.

- 2026-11-01 (feature 16, foundation_ai): the dev profile uses the Cranelift
  codegen backend (`CARGO_PROFILE_DEV_CODEGEN_BACKEND=cranelift` via mise), and
  Cranelift cannot compile `gemm-f16`'s AArch64 inline assembly, so the E9
  verify command `cargo test -p foundation_ai --no-default-features --features
  testing --no-run` fails in the `gemm-f16` dependency before reaching
  `foundation_ai`. Run it with `--profile uat` (llvm) to verify the submodule
  removal. `cargo test` also takes one positional TESTNAME before `--`; several
  filters go after `--` as libtest arguments.

- 2026-11-01 (feature 21, foundation_cronjobs): the `cron` crate was rejected
  and `saffron` 0.1 was chosen. `saffron` parses a five-field Quartz expression
  (minute, hour, day of month, month, day of week). That matches the existing
  `cron_expr` values such as `* * * * *`. The `cron` crate needs six or seven
  fields with seconds first, so it would break every saved expression. In
  `saffron`, the day of week is Quartz too: 1 is Sunday and 7 is Saturday. So
  `1-5` is Sunday to Thursday, not Monday to Friday. Monday to Friday is `2-6`.

- 2026-11-01 (feature 21, foundation_cronjobs): cron matching needs wall-clock
  calendar fields (weekday, month, leap day). Valtron's `Clock` is monotonic
  only and has no calendar. The crate adds a `CronClock` trait, a `UtcClock`
  default and a `SharedCronClock`. The clock is injected by constructor
  (threaded through `JobRunTask::new` / `into_run_task` / `into_run_tasks` and
  stored in `JobRunState`), never read from `Ctx` — a missing clock is a caller
  error, not a silent `UtcClock` fallback (rule 11). The `#[task]` macro
  generates a braced struct that hides its state, so the crate writes a manual
  `AsyncStateTask` wrapper (`JobRunTask`). That wrapper keeps its state in an
  `Arc<Mutex<JobRunState>>` and exposes `snapshot()`.
  `saffron::Cron::next_after` returns the next fire strictly after the input,
  at whole minutes.

- 2026-11-01 (feature 21, foundation_cronjobs): `TursoStorage::new` inside an
  async test triggers a `recursion_depth_exceeding_limit` warning from the
  `async_trait` Send-check. The fix used in `foundation_db/src/lib.rs`
  (`#![recursion_limit = "256"]`) is also needed at the top of the test crate
  (`tests/scheduler_tests.rs`), because each test target has its own recursion
  limit.

- 2026-11-01 (feature 21, foundation_cedar): `EntityId::new(id)` is infallible
  in `cedar-policy` 4.11. The old `EntityId::from_str(id).map_err(..)` mapped
  an error type that cannot occur (`Infallible`), which caused the rustc
  `unreachable call` warnings; `new` removes them. A `PolicyBundle` struct plus
  a `PolicyStore::load_all` default method lets `SqlPolicyStore` read its row
  once (four columns in one `SELECT`) instead of once per field. Clippy still
  reports 24 pre-existing `pedantic` warnings in untouched cedar library and
  test files; they are recorded in `docs/bug-backlog.md` §15.

- 2026-11-01 (bug-backlog §13): macOS kqueue reports `EVFILT_READ`
  level-triggered for pipes despite `EV_CLEAR`. A C probe confirmed it: byte
  counts 5, 3 and 9 all fire, and re-registering with fresh `EV_CLEAR` does not
  help. The `data` field is the bytes available, which is reliable. So the
  selector tracks the read edge in software: it delivers a readable event once per
  edge and suppresses it while `data` has not grown. A drained fd must re-arm
  (`rearm_readable`, wired to `reactor.clear`), or a drain-then-refill of the same
  or fewer bytes is lost — the kernel cannot see the read.

- 2026-11-01 (bug-backlog §13): the macOS kqueue waker (`EVFILT_USER`) returns
  `udata = 0` after `NOTE_TRIGGER` with a null `udata`, so `Event::token()` reads
  `Token(0)`, not the registered token. Linux epoll uses a different waker, so
  this is macOS-only. `waker_wakes_from_another_thread` and
  `waker_wakes_from_multiple_threads` fail locally for this reason.

- 2026-11-01 (feature 26): `reactor_shared_selector_tests.rs` is gated
  `#![cfg(all(unix, feature = "fd"))]`, and `fd` is not a default feature of
  `foundation_nativeapis`, so `cargo test -p foundation_nativeapis` does not run
  these tests — pass `--features fd`. `/proc/self/fd` exists only on Linux; any
  test or helper that reads it must carry `#[cfg(target_os = "linux")]`, or it
  panics at runtime on macOS (`.expect("procfs must be mounted")`), which a plain
  `cargo check --all-targets` will not catch because the code compiles fine. The
  portable pipe (`pipe` + `fcntl` on macOS, `pipe2` on Linux) was already present;
  only the procfs gating remained.

- 2026-11-01 (feature 36): `DnsResolver::resolve` became async via `#[async_trait]`
  (the crate re-exports the macro). The `Arc<T>` blanket impl compiles with
  `T: DnsResolver + ?Sized`, so `Arc<dyn DnsResolver>` still works. The trait also
  gains a sync `resolve_blocking` (default = poll the boxed future with a
  `std::thread::Thread::unpark` waker and `park` on `Pending`); valtron's
  `block_on_future` cannot be used because it needs a `'static` future and
  `resolver.resolve(..)` borrows the resolver. Critical: `SystemDnsResolver` and
  `CachingDnsResolver` must override `resolve_blocking` to run `to_socket_addrs()`
  inline — routing the sync path through the async offload calls
  `run_background_job`, which panics ("Background job pool not initialized") on a
  plain thread with no pool. Offloading `to_socket_addrs()` uses
  `run_background_job` + `Pipe`;
  the submit result is `AnyResult<(), Box<dyn Error>>` whose error is not `Send`,
  so stringify it (`.map_err(|e| e.to_string())`) before the pipe `await` or the
  async fn future stops being `Send` (E0310/E0277). The request task polls the
  lookup through a new `Resolving` state that drains a `ConcurrentQueue<WakeToken>`
  with `queue_waker(..)`, parks on `io_or_deadline(QueueReadiness, deadline)`, and
  fails with `HttpClientError::ConnectionTimeout` at the configured
  `dns_timeout` (default 30 s, nginx `resolver_timeout`). Gate the offload with
  `#[cfg(not(target_family = "wasm"))]`; wasm keeps the blocking lookup inline
  because its `DefaultResolver` is `NoopDnsResolver` and the browser resolves.

- 2026-11-01 (feature 37): a transport with `conn_readiness() == None` needs two
  fallback shapes. `IoReady` (wait to read or write) must sleep one poll-fallback
  interval, then return `Ok`; the caller retries its syscall. Re-polling
  `ready_now` forever never completes, because it always answers "not ready yet"
  for such a transport. `Hangup` (wait for the peer to close) must re-poll
  `gone_now` every interval and return `Ok` only on close/error. The raw-fd
  `poll(2)` probe uses `POLLHUP`/`POLLERR`/`POLLNVAL`, plus `POLLRDHUP` on Linux.
  Do not poll the completion socket's fd for *readiness*: `is_kernel_read()` is
  true, so the kernel already moved the data into the inbox; the fd only still
  tells close/error. The interval is a config field (rule 8), default 10 ms from
  `POLL_DELAY` in `transport/h2.rs`; the wake is a valtron timer, never a spin.

- 2026-11-01 (uuid via foundation_compact): do not give a consumer crate the
  `uuid` crate's `js`/`v4`/`getrandom` features. Those bring a second entropy
  path that bypasses rule 11 (one injected entropy seam). `uuid` 1.x offers
  `Builder::from_random_bytes([u8; 16])` + `into_uuid()`, a `const fn` that only
  sets the version-4 and RFC 4122 variant bits and needs no feature. So
  `foundation_compact` depends on `uuid = { version = "1", default-features = false }`
  and exposes `foundation_compact::uuid::new_v4()` which fills 16 bytes with
  `foundation_compact::entropy::fill` (native OS RNG / wasm
  `crypto.getRandomValues`) and then calls the builder. Consumers (for example
  `foundation_keychain`) use `foundation_compact::uuid::{self, Uuid}` and never
  depend on `uuid` directly. Inside `foundation_compact/src/uuid.rs`, write the
  crate path as `::uuid::Builder` (leading `::`) so the extern prelude resolves
  the crate, not the module of the same name.

- 2026-11-01 (wasm fix, superseded): `uuid` 1.20 added a `compile_error!` on
  `wasm32-unknown-unknown` when no randomness source is enabled (`js`,
  `rng-getrandom`, or `rng-rand`). Of those three, only `js` works on wasm:
  uuid target-gates its `getrandom` and `rand` dependencies to non-wasm, so the
  other two features resolve to nothing there. `js` uses WebCrypto
  `getRandomValues` and is a no-op on native (its `wasm-bindgen`/`js-sys` deps
  are target-gated). The earlier fix — declaring `uuid = { version = "1",
  features = ["v4", "js"] }` in a consumer crate — is now replaced by the
  `foundation_compact` uuid module above. Do not restore the consumer-level
  `js` feature.

- 2026-11-01 (publish fix): a version bump must move every consumer, not just the
  changed crate. `cargo package` resolves a `path = ".."` + `version = "0.0.6"`
  dependency to crates.io `^0.0.6` (`>=0.0.6, <0.0.7`), not to the local tree. So
  when `foundation_core` 0.0.6 was published before `request_executor_resume` and
  `EventReadiness::clear` existed, `foundation_reactor` 0.0.1 (which uses both)
  failed its publish tarball verification with E0425/E0407. The published 0.0.6 is
  a floor, and `^0.0.6` excludes the fixed 0.0.7. Fix: bump `foundation_core` and
  `foundation_macros` to 0.0.7 and bump every inline and table-format dependency on
  them to `0.0.7` in the same change (43 manifests plus the lock). Do not bump
  `foundation_nostd` (its own 0.0.6 is unrelated) or the cloud-provider crates'
  own 0.1.x versions. A `release-plz` `release-pr` failure that looks like a
  manifest parse problem is often the root manifest's nightly-only
  `cargo-features = ["codegen-backend"]` (see the next entry).

- 2026-11-01 (release-plz fix): the `release-pr` step failed because the root
  `Cargo.toml` carried `cargo-features = ["codegen-backend"]`, a nightly-only
  directive that stable Cargo 1.98.0 refuses to parse. release-plz runs `cargo
  package --list` on stable, so it could not read `edition.workspace = true` and
  failed. The `codegen-backend` choice lives in `mise.toml` `[env]`
  (`CARGO_PROFILE_DEV_CODEGEN_BACKEND=cranelift`, `_UAT_`=llvm,
  `CARGO_UNSTABLE_CODEGEN_BACKEND=true`), not in the manifest; the manifest line is
  redundant and wrong. Already fixed on master by `df86d7e6d`. A release that fails
  only in `release-pr` (not `publish`) usually points at a root-manifest
  `cargo-features`, not the crate being released.

- 2026-11-01 (recursion fix): the `recursion_depth_exceeding_limit` warning on
  `#[valtron_test]` tests is not a macro problem. It is the `turso` crate's deep
  async chain (`Connection::execute`, `maybe_handle_dangling_tx`,
  `Statement::execute`) overflowing the default 128 limit when `Send` is resolved.
  The macro only forces the `Send` check (its `timeout` path spawns a
  `std::thread`). The fix is `#![recursion_limit = "256"]` in the turso-backed
  crates (`foundation_db` lib and the three test files in `foundation_db` and
  `foundation_auth`), not a macro change. `recursion_limit` is per-crate, so each
  integration-test binary that overflows needs its own attribute. Check the true
  scope before trusting a spec: run `cargo check --workspace --all-targets` and
  grep the warning, instead of the crate the spec names. A local rewrite cannot
  avoid the limit: the depth is in turso's future *types* (its nested
  `async fn` graph), not in our own nesting, so splitting the generic method or
  hand-writing the boxed future still walks the same chain. Dropping `Send`
  (`?Send`) would stop the recursion, but it is unavailable — `block_on_future`
  (the `#[valtron_test]` / `#[valtron]` sync boundary) schedules onto the
  multi-threaded pool and requires `F: Future + Send`. `recursion_limit` is the
  intended knob for a third-party async graph deeper than the conservative 128.

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

- 2026-11-06 (feature 136): the kqueue read-edge re-arm belongs in the reactor, not in
  every caller. `Reactor::register_waker` and `Reactor::register_task_waker` now call
  `Poll::rearm_readable(entry.fd)` before they store the waker. The selector suppresses a
  readable event while the fd's byte count does not grow, so a drained socket that receives a
  smaller frame must first forget the old count. A loopback TCP test proves the rule: 77
  bytes, drain, arm the waker, then 15 bytes. The first test fails on the old code on macOS
  after the 2 s bound. `clear` keeps its re-arm; the docs now say it serves the latched flags,
  not the edge. Clippy stayed at the recorded pedantic debt (121 before, 120 after).

- 2026-11-06 (backlog migration): a written backlog rots in both directions. Of 73 entries, 11
  described bugs that were already fixed, and 3 described code that no longer matched the text
  (`BashTool::timeout` is read, `FileChangeEventPump` has a user, `QueueReadiness` clones `Arc`s
  rather than duplicating state). Each entry needs a code check before it is trusted. The
  tracker is now the issue tracker, where an entry carries the same labels as the defect and
  closes in the fixing change. Clippy debt is one issue with a per-crate checklist, because one
  issue per crate multiplied the noise without adding a separate action.

- 2026-11-06 (feature 120): a crate that CI never lints hides its debt twice. The first
  `--all-targets` run stops at the lib target, so the test-target warnings appear only after
  the lib compiles. Measure `--all-targets` on the fixed tree before you claim zero. A
  newly added `#[must_use]` on a method that many callers ignore produces warnings in
  dependent crates, not in the crate that declares it, so the per-crate deny command stays
  green while a fresh build prints 105 lines (issue #211).

- 2026-11-06 (feature 118): the `type_complexity` lint and `impl Trait` do not mix behind an
  alias. A type alias cannot name `impl Fn`, so "fix it with an alias" forces a boxed trait
  object, which adds `'static` bounds and changes public signatures. On an extension trait
  that returns `impl Fn`, either keep `impl Fn` and carry one `#[expect]`, or accept the
  breaking change and bump the crate. Feature 118 took the second path: foundation_core
  0.0.13 and 38 consumer requirements. Measure a lint fix on the public surface as an API
  change before you apply it, not after.


- 2026-11-06 (feature 132): the executor already owns the pool root `Ctx`. A
  deadline that the executor computes can therefore read `root.clock()` without a
  new bundle. The only reads that cannot move are the blocking waits: the timer
  thread, the condvar park and the accept loop. They name `MonotonicClock`
  explicitly, so the code says "real clock here" and the gate still counts no
  direct read. Do not hide such a read in a helper; name the backend type.
- 2026-11-06 (feature 132): a ConnectRPC call context has its own `Ctx`, not the
  valtron `Ctx`. The shared `ContextBag` is the seam: the HTTP server stores its
  engine clock in the app bag before it dispatches, and `build_ctx` reads it. A
  bag built outside an engine has no clock, so a direct dispatcher test gets an
  explicit `MonotonicClock`. That is a host shortcut, not a global fallback.
- 2026-11-06 (feature 132): `#[must_use]` on `PipeSender::close` and
  `PipeReceiver::close` produced 105 warnings in dependent crates (issue #211).
  Some `.close()` calls return `()` (a sink trait method), so a blanket
  `let _ = x.close();` triggers `clippy::let_unit_value`. Find the exact warning
  lines; do not rewrite every `.close();`. The clippy gate fails on the
  `unused_must_use` warnings even with `--no-deps`, so a feature in these crates
  cannot pass until they are fixed.
- 2026-11-06 (feature 132): a Rust doc example is a compiled test. A changed
  constructor in a `//!` or `///` example fails the crate's doctest target, not
  the lib target. Update the example and keep its `//!` prefix on every line.
- 2026-11-06 (feature 132): `SimulatedClock::now()` starts at the real instant it
  was built plus its offset, while `wall_now` starts at the UNIX epoch. A test
  that compares a simulated deadline with the real clock sees a difference of
  milliseconds, not hours. That is why the old-code HTTP test failed with
  `Some(Delayed(3ms))` and not with a large value.

- 2026-11-06 (feature 133): a process-global cache that holds a backend also
  holds the first caller's clock. `foundation_ai` caches GGUF models in
  `LOADED_MODELS`; a plain reuse would serve a model built with session A's
  clock to session B. `share_weights(clock)` now re-injects the caller's clock
  on every load. Check every `static`/`OnceLock` cache that stores a clock.
- 2026-11-06 (feature 133): `Arc::clone` does not coerce
  `&Arc<MonotonicClock>` to `&Arc<dyn Clock>`. The clone happens before the
  argument coercion, so annotate the binding at the construction point:
  `let clock: Arc<dyn Clock> = Arc::new(MonotonicClock::new());`. Three app and
  example call sites hit this. Passing the `Arc<MonotonicClock>` by value does
  coerce, so constructors that take the clock by value are fine.
- 2026-11-06 (feature 133): the `foundation_auth --features server-test` suite
  does not finish at default parallelism on a 10-CPU host. A sampled run shows
  every test thread blocked in
  `initialize_pool_with_slow_poll_warning` -> `FairGate::acquire`, while one
  filtered test passes. Treat this as a separate executor load finding (high);
  run a new integration test with
  `--test foundation_auth_tests <name> -- --test-threads=1` to prove it.
- 2026-11-06 (feature 133): `saturating_sub(now, window)` in
  `foundation_db::core::rate_limit` looked wrong near the UNIX epoch, but it is
  correct. The cutoff clamps to 0, and `window_start <= 0` is false for every
  record stamped after the epoch, so the window is longer, not "elapsed". Do
  not change it without a failing test.
- 2026-11-06 (feature 133): a bulk edit that inserts a helper at the first
  `#[test]`/`#[valtron_test]` occurrence can land inside a doc comment or a
  `use` block. Two files failed to parse, and nine more lost the doc comment
  that belonged to the first test. Run `cargo fmt --all --check` after a scripted
  insertion; it is the cheapest parser check.
- 2026-11-06 (feature 133, coordinator): a suite that "hangs with every thread
  in `FairGate::acquire`" can be a panic, not a starvation bug. A panic on a
  spawned dispatch thread leaves the caller blocked and holding the pool guard,
  so all later tests wait at the gate. Sample the stuck process and read the
  last completed test; the panic message is behind it. Check the failing test
  alone before you suspect the executor.
- 2026-11-06 (feature 133, coordinator): when a type is needed both in a task
  `Ctx` and on a required struct (`HandlerStorage`), make the struct the source
  and let the `Ctx` value win when present. A second required registration
  makes every host that builds the app by hand panic.

- 2026-11-06 (feature 134): a crate below `foundation_core` cannot take
  core's `Clock` without a dependency cycle (`foundation_core` depends on
  `foundation_nostd` and, optionally, `foundation_wasm`). `foundation_nostd`
  defines its own small `TimeSource` trait for the blocking wait.
  `foundation_wasm` keeps one `Instant::now()` and marks it with
  `// time-source:` because its WASI extern-C ABI has no Rust caller that can
  pass a clock; every timer function gained an `_at` variant instead.
- 2026-11-06 (feature 134): a selector that blocks in the kernel must compute
  its EINTR-resume deadline from the operating system's monotonic clock. A
  simulated clock would make `epoll_wait`/`kevent` return at once or block
  forever. `foundation_reactor` names `MonotonicClock` explicitly for this
  boundary, like the executor waits in issue #102. Do not inject a simulated
  clock into a real blocking wait.
- 2026-11-06 (feature 134): a time value used only as an ID seed does not need
  a clock. `foundation_deployment_docker` replaced the timestamp seed in
  `new_session_id` with a process-wide counter mixed with the process id. The
  ID is still unique within the process and the read is gone.
- 2026-11-06 (feature 134): generated code carries the caller's API version.
  When `WgNode::from_config` and `PlatformSession::new_test` gained a clock,
  the `#[wireguard_main]` and `#[platform_test]` generators had to change in
  the same feature. Grep every `quote!` block that calls a changed
  constructor; a macro crate can compile while its output does not.
- 2026-11-06 (feature 134): a public API change ripples into test helper
  modules, not only `src/`. `foundation_testbed/tests/common/mod.rs` and
  `foundation_ai/tests/tools/*` called the changed constructors. Run
  `cargo check --workspace` and every changed crate with `--all-targets`.
- 2026-11-06 (feature 134): read a crate's own version from `[package]` with
  a TOML parser. Several manifests put `[package]` after the dependency
  tables, so `grep -m1 '^version'` returns a dependency's version.
  `foundation_deployment_cloudflare` is 0.2.0, not 0.1.3; it moved to 0.2.1.
- 2026-11-06 (feature 134): the gate fails on an allowlist entry that matches
  no read. A phase that fixes a crate must delete its entries in the same
  change; the count then only falls. The final state is 13 allowlisted reads
  (the two permanent entries) and 1 marked boundary read.
- 2026-11-06 (feature 134): editing a shared dependency while other agents
  build breaks their runs. The `foundation_reactor` selector signature briefly
  broke `foundation_nativeapis` and `foundation_wireguard` builds. Schedule a
  shared dependency (reactor, sshkit) after the agents that compile it, or
  keep the change internal so its public signatures do not move.

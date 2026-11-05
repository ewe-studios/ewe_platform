---
feature: "The proxy load-balancing e2e test stops hitting its 30 s cap"
status: "pending"
lane: "priority follow-up (bug found in run 42087629722)"
model: "sonnet"
branch: "spec70/f121-proxy-spread-flake"
crates: ["foundation_proxy"]
catalogue_rows: []  # from docs/bug-backlog.md §9; not a §8 catalogue row
---

# Feature 121: the proxy load-balancing e2e test stops timing out

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **e2e test**: end-to-end test. It starts a real proxy and real backends in one process.
- **pool**: the valtron worker threads that run the test body and the asynchronous calls.
- **pool gate**: a process-wide lock that lets one pool exist at a time.

## Problem (measured 2026-11-06)

Two consecutive Linux CI runs failed the `foundation_proxy` `test` step on two different
tests. Both failures are a request that stalls, then fails when a timeout tears the test
down. The pattern needs the full test binary, not one test.

### Run 42087629722 (commit `b7bff1de1`)

The suite `tests/rpc_proxy_e2e_tests.rs` reported:

```text
test result: FAILED. 16 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 53.23s
thread 'connect_streams_spread_across_grpc_backends' panicked at backends/foundation_proxy/tests/rpc_proxy_e2e_tests.rs:898:1:
timeout: the test took 30001 ms. Max 30000 ms.
```

### Run 42099300473 (commit `9b1138a63`)

The suite `tests/h2_integration_tests.rs` reported:

```text
test result: FAILED. 6 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 60.54s
thread '<unnamed>' (7172) panicked at backends/foundation_proxy/tests/h2_integration_tests.rs:576:5:
assertion `left == right` failed: grpc-status trailer must be relayed as 0
  left: None
 right: Some("0")
thread 'h2_grpc_trailers_relayed' (7145) panicked at backends/foundation_core/src/valtron/executors/sendables.rs:905:43:
block_on_future: the future produced no result (pool not initialized or torn down early)
```

### Measured facts

1. In the green run 42072422806 the `rpc_proxy_e2e_tests` suite finished in 17.50 s. The
   `h2_integration_tests` suite finished in 0.59 s in run 42087629722.
2. Alone on macOS, the spread test finishes in 0.11 s. Six full-suite runs on macOS all
   pass. Five full-suite runs of both binaries in the Linux CI image also pass.
3. In run 1 the proxy and both tagged backends started at 19:45:15.534, 2 ms after the
   previous test's servers stopped at 19:45:15.532. No `SendRequestTask` or `access` line
   follows before the 30 s cutoff.
4. In run 2 the proxy started at 20:40:18.928, 1 ms after the previous test's servers
   stopped at 20:40:18.926. The failure came 60.1 s later. The front's HTTP/2 idle
   timeout is 60 s (`foundation_http/src/native/server/h2_connection.rs:53`).
5. Six queued tests finish within 0.7 s after the harness kills the run 1 test. The pool
   gate serialises the tests of one binary, so the killed test also held the pool.
6. Each client call in `rpc_proxy_e2e_tests` uses `ctx()`, which sets a 15 s deadline.
   That deadline did not fire. It cannot: `with_deadline`
   (`foundation_connectrpc/src/shared/context/mod.rs:228`) only records
   `now + d` in the request context, and no layer enforces a parked read.
   `h2_grpc_request` sets no deadline at all.

The load balancer and the trailer relay are not the cause: in both runs the request
never reaches the handler. The common shape is a proxy started within 2 ms of another
proxy's shutdown, in a binary whose tests run in parallel and share one process-wide
shared file descriptor reactor (`foundation_reactor/src/reactor.rs`) and one pool at a
time. The leading hypothesis is a lost readiness wake for a new socket when the old
server deregisters its registrations from the shared reactor at the same moment.

Reading the reactor code found a mechanism that produces exactly this stall, recorded in
[feature 122](../122-reactor-deregister-fd-reuse/feature.md): `Reactor::deregister`
changes the selector by fd number alone, and a socket that closed earlier can hand its
number to a new socket. The late deregistration then deletes the new socket's selector
entry, and a task parked on that socket never wakes. Feature 122 fixes the cause.

The trace run below puts the stalled wait after the backend handshake: the proxy holds an
established backend connection, and the backend never reads the forwarded request. That
matches the feature 122 shape most closely: the backend connection's fd number is freshly
allocated, the previous test's late deregistration removes its selector entry, its first
read succeeds from the kernel buffer, and its next park never wakes.

The accept loop is not the stalled wait: on `WouldBlock` it parks with
`accept_shutdown_poll`, which defaults to 10 ms (`ServerConfig`, `server/mod.rs:213`), and
it calls `accept` again on every wake. A clobbered listener registration would cost one
10 ms pause, not 30 s. The stall must be a parked task on a connection, or a client call
that waits for a response. In run 2 the connection's own 60 s idle timer fired, which
shows that the task was alive and its timers ran while the read never reported readiness.

## Design

### 1. Reproduce on Linux

Loop the full test binaries in the Linux CI image, not a single test. Run them in
parallel (`cargo test` default threads) because the failure needs the concurrent
servers:

```bash
for i in $(seq 1 30); do
  cargo test --profile uat -p foundation_proxy --test rpc_proxy_e2e_tests \
    || { echo "rpc run $i failed"; break; }
  cargo test --profile uat -p foundation_proxy --test h2_integration_tests \
    || { echo "h2 run $i failed"; break; }
done
```

Run with `RPC_E2E_TEST_TIMEOUT_MS=120000` only to let a stuck run finish and print its
own diagnostics. Do not keep that value as the fix.

Five runs of both binaries in the Linux CI image pass on an Apple Silicon host. Constrain
the container to two CPUs (`docker run --cpus=2`), which matches a loaded GitHub runner:
that reproduces the flake. Run 5 of 10 failed
`rpc_proxy_e2e_tests::connect_client_stream_via_grpc_backend` at
`rpc_proxy_e2e_tests.rs:810` (`client_stream`) after 29.14 s. That is a third test in the
same class: a client call made soon after another test's proxy stopped, which never
finishes.
Then stress the suspected window: start and stop two proxies in a row with a 1 ms gap in
one test, in a binary that runs several such tests in parallel.

Record which selector each environment uses before comparing runs. The Linux reactor
picks io_uring when the probe succeeds and epoll when it does not
(`foundation_reactor/src/poll/sys/unix/selector/dispatch.rs`), and this host runs the
linux/amd64 image under emulation. Run
`cargo test -p foundation_iogate --test io_uring_probe` with `EWE_REQUIRE_IO_URING=1` in
the image, and print `Reactor::backend()` in the loop's first run. If the two environments
use different selectors, compare only like with like.

### 2. Locate the stall

The trace run already narrows it: the front accepts, parses and dispatches, and the
backend completes the handshake but never receives the request. Start from the relay's
own reads: the front-to-backend request-body pump, and the backend connection's read of
the forwarded frames. Add a temporary watchdog that prints once a second: bytes read and
written in each direction of the relay, and which of the two waits is parked. Enable
`foundation_proxy=trace` and `foundation_netio=debug` for the run.

Also log the registry state for the two fds when a wait exceeds one second: the entries
map token, the selector's live token for that number, and whether the waker is installed.
That answers the feature 122 question directly.

### 3. Fix the cause

The measurement picks one of these:

- **A late deregistration removes the new socket's registration.** Feature 122 states the
  mechanism: the selector is changed by fd number alone, and a number can be reused. Feature
  122's test reproduces it without the proxy. If it passes and this flake remains, continue
  with the next bullet.
- **A listener readiness loses its wake when another server deregisters.** The shared
  reactor is a process singleton (`foundation_reactor/src/reactor.rs`), and in both
  failures a new proxy started within 2 ms of another proxy's shutdown. A token is never
  reused (`foundation_reactor/src/token.rs` is a monotonic counter), so the lost wake is
  not a token reuse. Check the register-before-check window and the drain loop's
  handling of an entry removed while its event is in flight.
- **A parked task misses its wake.** Feature 75 fixed one such wake in `block_on`.
  Check the readiness park path and the executor's idle re-check.
- **The HTTP/2 front stops accepting under load.** Check the accept loop and the
  reactor's readiness registration.
- **No layer enforces the call deadline.** `Ctx::with_deadline` only records a
  timestamp. The test therefore needs its own bound (Design 4). Record a separate
  backlog entry if a server-side deadline is also missing.

### 4. Keep the test honest

A hung call must fail on its own deadline with a message that names the call and the
state. The 30 s cap stays as a backstop. Do not raise it to hide a hang.

`Ctx::with_deadline` is only a timestamp, so the test helper must enforce the bound
itself: wrap each client call in a `select` over the call and a
`foundation_core::valtron::sleep_async` of the same length, and panic with the procedure
name and elapsed time when the sleep wins. Add the same bound to `h2_grpc_request`. Set
it above the slowest passing run (about 0.2 s per call) and far below the 30 s cap, for
example 5 s.

## Tests

- The regression check is the 30-run loop above, green.
- Feature 122 carries the deterministic test of the fd-reuse deregistration. Use it here
  as the unit-level proof that the reactor keeps a live registration armed.
- If the cause is a lost reactor wake, add a deterministic test of the token lifetime:
  register a readiness, deregister it, register another, and assert that an event for
  the first never reaches the second.
- If the fix changes a shared wake or deadline path, add one deterministic test that
  fails on the old code. Use the `test-clock` feature (feature 115) to advance a
  simulated clock past a deadline and assert that the waiting task runs.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_proxy --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_proxy --test rpc_proxy_e2e_tests
cargo test --profile uat -p foundation_proxy --test h2_integration_tests
python3 scripts/ci/ci.py run foundation_proxy --profile uat
```

## Done when

- [ ] 30 consecutive full-suite runs on Linux in the CI image report 17 passed and 7
      passed.
- [ ] The commit message names the call that stalled and the wait it sat on.
- [ ] A hung call fails on its own bound (about 5 s) with a message that names the call,
      not only on the 30 s cap.
- [ ] The backlog §9 entry is deleted.
- [ ] CI green.

## Result

2026-11-06. Plan only. Not started.

Measured while the plan was written:

- Six full-suite runs on macOS (`cargo test --profile uat -p foundation_proxy --test
  rpc_proxy_e2e_tests`) all report 17 passed, in 16.42 s to 22.46 s. The failing test
  alone passes in 0.11 s.
- Five full-suite runs of both binaries in the Linux CI image all pass: 7 passed in about
  1.0 s and 17 passed in about 24.2 s each time.
- Ten more runs with `--cpus=2` reproduce it once: run 5 failed
  `connect_client_stream_via_grpc_backend` after 29.14 s. The failure rate is about 1 in
  5, so the 30-run loop must run several times.
- A second loop of only `rpc_proxy_e2e_tests`, twenty runs, failed on run 5 in
  `connect_bidi_via_grpc_backend`: the 30 s cap fired. The saved log of that run
  shows the epoll backend (this container has no
  io_uring: `io_uring_setup failed: Function not implemented`), the proxy started at
  T+0.000, two `[DEPENDS] Task ... returned already-ready signal` warnings within 0.2 s,
  and then no further line until the harness killed the test at T+30 s. The accept line
  is `tracing::trace!`, so an INFO capture cannot say whether the connection was
  accepted. That is why Design 2 adds a counter.
- Both loops together: three different tests fail, all of them proxy e2e tests that make
  a client call soon after another test's servers stopped.
- A third loop with `RUST_LOG=foundation_http=trace,foundation_proxy=debug` failed on
  run 6 with two tests. The trace localises the stall:
  - `connect_client_stream_via_grpc_backend`: the front accepted the client at T+0.00,
    parsed the request by T+0.08, dispatched it, and the backend accepted the proxy's
    dial at T+0.14 and completed the HTTP/2 handshake at T+0.15. At T+5.10 the client's
    own transport logged `[INTRO] no response head before the deadline timeout=5s` and
    the call failed with `[unavailable] transport connect failure: Timeout occurred`.
    The proxy's front handler stayed parked until the harness dropped it at teardown
    (`Awaiting: server draining, dropping the handler future`), and the harness reported
    that pool workers were still blocked when it killed the test.
  - `connect_bidi_via_grpc_backend`: the front received `HEADERS` and four `DATA`
    frames and routed the request, and the backend completed its handshake. The backend
    then logged `h2 server: idle — waiting for frames` four times and never saw the
    request.
  - So the request reaches the proxy front and the proxy dials the backend, but the
    backend never receives it. The stalled wait is in the front-to-backend relay, not in
    the client-to-proxy leg and not in the accept loop.
  - No `[DEPENDS] already-ready` line appears in this run; the two in the earlier run
    were incidental.
- Attempt 1 of audit run 42099300473 failed `foundation_proxy` again, while
  `foundation_core`, `foundation_macros` and `foundation_netio` passed.

The failure needs the Linux CI environment, so the plan starts with the 30-run loop
there. A loop with `--cpus=2` is running to match a loaded GitHub runner.

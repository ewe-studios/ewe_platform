---
feature: "The `Expect: 100-continue` timer test is deterministic under load"
status: "done"
lane: "after 84"
model: "sonnet"
branch: "spec70/f108-expect-continue-timer-test"
crates: ["foundation_netio"]
catalogue_rows: []  # from docs/bug-backlog.md §2; not a §8 catalogue row
---

# Feature 108: `silent_server_gets_body_when_the_timer_fires` does not depend on load

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (measured 2026-11-05, feature 82)

A full `foundation_netio` test run failed
`simple_http::nonblocking_client_tests::silent_server_gets_body_when_the_timer_fires`
(`backends/foundation_netio/tests/simple_http/nonblocking_client_tests.rs:459`) once, while
two other `cargo test` suites built and ran on the same machine. The same test passed 10 of
10 solo reruns.

The test waits for the server's `Expect: 100-continue` timer, which is about 300 ms, then
asserts that the wait is under 2 s. The wait uses the wall clock. Compiler and test load
stretches it past the bound. CI also runs test binaries in parallel on one runner, so a
loaded runner can fail this test the same way. A flaky test hides real failures and costs a
rerun.

## Design

Remove the dependence on wall-clock scheduling.

1. Find the wait. It is the timer in the `Expect: 100-continue` path of
   `foundation_netio` (`src/shared/http/` server side) and the client test that observes it.
2. Read the clock through the injected environment bundle (AGENTS rule 11: clocks and
   timers sit behind traits and come from the `Env` entry in `Ctx`). If the path still calls
   the system clock or `thread::sleep`, move it to the injected clock.
3. In the test, install a simulated clock and advance it explicitly. Then the assertion is
   a statement about the server's timer logic, not about the machine's speed.
4. If the production path cannot take an injected clock without a design change, do not
   widen the bound on a guess. Measure the wait under the CI runner and record the numbers in
   the feature. Widen the bound only with that measurement, and say so in a comment.

## Tests

- The existing test passes 20 of 20 runs while two other suites run on the same machine.
- A new test asserts the timer decision at the simulated deadline: the server fires the
  continuation at the configured `expect_continue_timeout` and not before it. Use the
  simulated clock, so it needs no sleep.
- A test proves the configured timeout is read from configuration, not a constant
  (AGENTS rule 8).

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_netio --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_netio --features multi --test foundation_netio_tests \
  silent_server_gets_body_when_the_timer_fires
python3 scripts/ci/ci.py run foundation_netio --profile uat   # in the Linux CI image
```

## Done when

- [x] The test reads no system clock and does not sleep.
- [x] The test passes 20 of 20 under concurrent load.
- [x] The backlog §2 entry is deleted.
- [x] CI green.

## Result (2026-11-05)

### Cause, measured

The old test measured the wait with the wall clock: `s.body_at - s.head_at < 2 s`
(`tests/simple_http/nonblocking_client_tests.rs:459`). The wait is about 300 ms, so a
scheduling stall of 1.7 s fails it. Reproduced by suspending the test process for
2.5 s in the middle of the wait, then resuming it. The old test failed with:

```text
body arrived 2.670018334s after the head; expected about the 300ms timer
```

The same test passed 12 of 12 runs under 8 busy CPU loops and 6 of 6 runs at
`nice -n 20`. CPU load alone did not reproduce it on this host. A process that is not
scheduled for 2 s does.

### Design

- The `Expect: 100-continue` timer now runs on a clock from the `Clock` trait
  (`foundation_core::valtron::Clock`), not on `std::time::Instant::now()`.
  `GetHttpRequestRedirectTask` resolves the engine clock from `Ctx` on its first poll
  (AGENTS rule 11). A request may carry its own `Arc<dyn Clock>` as a typed entry in its
  `Extensions`; that clock then replaces the engine clock for the request.
- The request extension is the injection point because a test cannot seed the pool's
  root `Ctx`: `Ctx::clock()` returns a private field and `ThreadRegistry.root` is
  private. The limitation is backlog entry §4 (foundation_core / valtron).
- The test server moves a test clock. It advances only after the client's first clock
  read, so the client's deadline cannot move with the advance. The server records the
  simulated instant of the head and of the body. The assertion is a statement about
  simulated time: the body goes out at exactly `expect_continue_timeout`.
- The server also proves "not before": it moves the clock to one millisecond before the
  deadline, waits for the client to poll again without sending, then moves the clock
  past the deadline. The second test uses 450 ms, not the 3 s default, so it proves the
  timeout comes from configuration (AGENTS rule 8).
- The blocking transports (`await_continue_blocking`) keep kernel read timeouts. A clock
  cannot drive a kernel wait. The tests use a non-blocking TCP socket.

### Verification (macOS, `--profile uat`)

| Command | Result |
|---|---|
| `cargo fmt -p foundation_netio --check` | clean |
| `cargo clippy --profile uat -p foundation_netio --all-targets --no-deps -- -D warnings` | clean |
| `cargo test --profile uat -p foundation_netio --test foundation_netio_tests silent_server_gets_body_when_the_timer_fires` | 1 passed |
| 20 runs of that test | 20 of 20 passed |
| `cargo test ... expect_continue_timer_fires_at_the_configured_deadline_and_not_before` | 1 passed |
| 20 runs of that test | 20 of 20 passed |
| 20 runs of it with two other netio suites and 8 CPU loops in parallel | 20 of 20 passed |
| `cargo test --profile uat -p foundation_netio --test foundation_netio_tests` | 1217 passed, 0 failed |

- Both tests pass with the process suspended for 2.5 s during the wait. The old test
  failed the same treatment.
- With the production timer temporarily changed back to `Instant::now()`, the rewritten
  test fails (`Timeout occurred`, 5 s response-head deadline). The restored code passes.
  So the test discriminates on the clock, not only on the bound.
- The new test shortens the wait's real duration to the configured timeout; it does not
  remove it. The executor sleeps to the mapped deadline. The assertion no longer reads
  real time.

### CI proof (2026-11-05)

GitHub run [42063296262](https://github.com/ewe-studios/ewe_platform/actions/runs/42063296262)
on `02cbb08b5`: the `foundation_netio` job (id `127371615978`) passed. Its log shows both
tests passing in the two test runs the job makes:

```text
test simple_http::nonblocking_client_tests::expect_continue_timer_fires_at_the_configured_deadline_and_not_before ... ok
test simple_http::nonblocking_client_tests::silent_server_gets_body_when_the_timer_fires ... ok
```

The whole run finished `success`: 113 jobs green, 0 failed, 3 skipped.

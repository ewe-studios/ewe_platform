---
feature: "The `Expect: 100-continue` timer test is deterministic under load"
status: "pending"
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

- [ ] The test reads no system clock and does not sleep.
- [ ] The test passes 20 of 20 under concurrent load.
- [ ] The backlog §2 entry is deleted.
- [ ] CI green.

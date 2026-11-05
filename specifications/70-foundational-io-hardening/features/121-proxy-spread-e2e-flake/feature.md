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

GitHub run 42087629722 (Linux amd64, commit `b7bff1de1`) failed the `foundation_proxy`
`test` step. The suite `tests/rpc_proxy_e2e_tests.rs` reported:

```text
test result: FAILED. 16 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 53.23s
thread 'connect_streams_spread_across_grpc_backends' panicked at backends/foundation_proxy/tests/rpc_proxy_e2e_tests.rs:898:1:
timeout: the test took 30001 ms. Max 30000 ms.
```

Measured facts:

1. In the green run 42072422806 the same suite finished in 17.50 s.
2. Alone on macOS, the test finishes in 0.11 s. Six full-suite runs on macOS all pass.
3. The failing log shows the proxy and both tagged backends started at 19:45:15.534.
   No `SendRequestTask` or `access` line follows for that test before the 30 s cutoff.
4. Six queued tests finish within 0.7 s after the harness kills the test. The pool gate
   serialises the tests of one binary, so the killed test also held the pool.
5. Each client call uses `ctx()` from the test file, which sets a 15 s deadline. That
   deadline did not fire.

The load balancer is not the cause: no request reaches it. Either the 12 client calls
are never polled, or the HTTP/2 front does not accept them while the pool is busy.
The per-call deadline not firing points at the first cause.

## Design

### 1. Reproduce on Linux

Loop the full suite in the Linux CI image, not the single test:

```bash
for i in $(seq 1 30); do
  cargo test --profile uat -p foundation_proxy --test rpc_proxy_e2e_tests \
    || { echo "run $i failed"; break; }
done
```

Run with `RPC_E2E_TEST_TIMEOUT_MS=120000` only to let a stuck run finish and print its
own diagnostics. Do not keep that value as the fix.

### 2. Locate the stall

Add a temporary watchdog to the test (or to a copy under `tests/`) that prints once a
second: how many of the 12 calls completed, the proxy's accepted-connection count, and
whether the pool workers are parked. Find the call that never returns and the wait it
sits on.

### 3. Fix the cause

The measurement picks one of these:

- **A parked task misses its wake.** Feature 75 fixed one such wake in `block_on`.
  Check the readiness park path and the executor's idle re-check.
- **The HTTP/2 front stops accepting under load.** Check the accept loop and the
  reactor's readiness registration.
- **The transport does not check the call deadline on one path.** Then a hung call
  cannot fail on its own 15 s deadline.

### 4. Keep the test honest

A hung call must fail on its own deadline with a message that names the call and the
state. The 30 s cap stays as a backstop. Do not raise it to hide a hang.

## Tests

- The regression check is the 30-run loop above, green.
- If the fix changes a shared wake or deadline path, add one deterministic test that
  fails on the old code. Use the `test-clock` feature (feature 115) to advance a
  simulated clock past a deadline and assert that the waiting task runs.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_proxy --all-targets --no-deps -- -D warnings
cargo test --profile uat -p foundation_proxy --test rpc_proxy_e2e_tests
python3 scripts/ci/ci.py run foundation_proxy --profile uat
```

## Done when

- [ ] 30 consecutive full-suite runs on Linux in the CI image report 17 passed.
- [ ] The commit message names the call that stalled and the wait it sat on.
- [ ] A hung call fails on its own 15 s deadline, not only on the 30 s cap.
- [ ] The backlog §9 entry is deleted.
- [ ] CI green.

## Result

2026-11-06. Plan only. Not started.

Measured while the plan was written: six full-suite runs on macOS
(`cargo test --profile uat -p foundation_proxy --test rpc_proxy_e2e_tests`) all
report 17 passed, in 16.42 s to 22.46 s. The failing test alone passes in 0.11 s.
The failure needs the Linux CI environment, so the plan starts with the 30-run
loop there.

---
feature: "foundation_proxy H3: stream the request body and park on the waker; io_uring tests fail loudly when io_uring is missing in CI"
status: "done"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_proxy", "foundation_iogate"]
catalogue_rows: [68, 69]
---

# Feature 72: H3 request body; io_uring test skips

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Found in feature 70 (backlog §9).

## E1. The H3 proxy buffers the whole request body and polls on a 1 ms timer

`src/h3_proxy.rs` `read_body` collects the whole request body (rule 4), and `read_headers` /
`read_body` poll with a 1 ms timer instead of parking on the connection waker. Stream the
request body to the upstream as it arrives (the same pump pattern feature 65 E5 uses for H1:
a bounded pushable pipe, `ProxyConfig::max_request_body` → 413), and park on the waker
(`park_op`, as feature 70's response path does). Tests: `h3_request_body_is_streamed`
(16 MiB upload; peak buffered under 1 MiB via the `BufferedBytesGauge`),
`h3_idle_request_uses_no_cpu` (thread CPU under 20 ms while a client holds a request open
for 1 s without sending).

## E2. io_uring tests skip silently when the kernel has no io_uring

They print a skip and pass, so CI can lose this coverage without anyone noticing. Keep the
skip for local machines, but fail when an environment variable says io_uring is expected
(for example `EWE_REQUIRE_IO_URING=1`), and set that variable in the Linux CI job for the
crates that have io_uring tests (check the CI image's kernel supports it first; if it does not,
record that in the backlog instead of setting the variable).

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo test -p foundation_proxy --profile dev --features quic
python3 scripts/ci/ci.py run foundation_proxy --profile dev
```
Flake check: 20 runs of each new test.

## Done when

- [x] H3 request body streamed and capped; no 1 ms polling (merged by feature 75).
- [x] io_uring tests fail when `EWE_REQUIRE_IO_URING=1` and io_uring is missing (CI variable not set,
      see backlog).

## Result (2026-11-03)

- E2 done for `foundation_iogate` and `foundation_proxy`. The CI variable is not set: CI jobs run in
  a Docker container and the kernel/seccomp support for io_uring is not confirmed (backlog §9).
  `foundation_nativeapis` io_uring tests are not changed.
- E1 is partial and not merged. The code and both tests are on branch `wip/f72-h3-request-body`
  (commit 7bbbf1b6a). `h3_request_body_is_streamed` failed on the old code (16777289 bytes held,
  line 311) and `h3_idle_request_uses_no_cpu` failed on the old code (816 stream polls and +165 ms
  CPU in one second, limit 100 polls). With the fix the peak is 1420 bytes and the idle polls are
  0, but the 16 MiB upload stalls in about one run in four and the 413 test in about one in five.
  Cause: a parked `QuicDriver` does not wake for its QUIC timers (the executor gives a
  `Readiness` sleeper no deadline); the old 1 ms timers hid it. Left: add the `EventReadiness`
  deadline hint (backlog §9), then merge the branch.

### Update (feature 75)

E1 landed with feature 75. The stall was a lost wake in the executor (the idle back-off
`SpinWait` sleep in `block_on` was not bounded by the sleepers), not the QUIC timers. The three
tests passed 20 of 20 each after the fix. The "idle request" test asserts on stream polls (0),
not on process CPU: the idle QUIC endpoint has its own cost.

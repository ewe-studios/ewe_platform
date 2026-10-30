---
feature: "QUIC driver wakes when a stream handle queues outgoing data"
status: "done"
priority: "high"
depends_on: "28"
branch: "fix/io-01-valtron-parking"
crates: ["foundation_netio", "foundation_http"]
backlog: "docs/bug-backlog.md §3 foundation_http — http3_server_rejects_a_request_it_has_no_handler_for is flaky"
---

# Feature 33: QUIC driver wakes when a stream handle queues outgoing data

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** input/output (I/O); Hypertext Transfer Protocol version 3
(HTTP/3); QUIC, the transport under HTTP/3 (a name, not an abbreviation); file descriptor (fd);
pull request (PR); AeroSpace and Defence Industries Association of Europe (ASD), the owner of
the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4.

## 1. Goal

The test `http3_server_rejects_a_request_it_has_no_handler_for` is flaky. It fails about 15
times in 20 runs. The test sends a request to a path with no HTTP/3 handler. The server resets
the request stream with `H3_REQUEST_REJECTED`. The reset sometimes never reaches the client, so
the test times out after 5 s.

The root cause is in `foundation_netio`, not `foundation_http`. A stream handle queues outgoing
data (a reset, write, finish, stop, close, a datagram, or a flow-control update) but does not
wake the QUIC driver. The driver parks on socket read readiness and the next timer (feature 28).
When the driver is parked with no pending timer and no incoming packet, the queued data is never
transmitted.

This feature wakes the driver for every queued outgoing data.

## 2. Design

The driver already owns an edge-triggered socket readiness and a deadline for the next timer. It
needs one more wake source: a latch that a stream handle sets after it queues outgoing data.

`BoolSignal` is the right latch. It is `Arc<AtomicBool>` and `EventReadiness`. Its `set(true)`
also asks the executor host to run again, so a driver parked on `TaskStatus::Depends` is
re-polled at once on native and re-scheduled on wasm.

The driver holds one `BoolSignal` (`driver_woken`) and shares a clone into every `ConnState`
as `driver_wake`. The driver clears the latch before each pass (the same clear-before-read order
as the socket edge) and composes it into its `Depends` readiness: socket edge, next timer, or
the latch. Every stream-handle method that queues outgoing data calls `ConnState::wake_driver()`
after the queue operation.

Clearing the latch at the start of a pass can never lose data: a queued transmit is re-discovered
by `pump_transmit` on the next pass, and a transmit the congestion controller cannot send yet is
covered by the loss-detection timer already in the `Depends` deadline.

```mermaid
flowchart LR
    A[stream handle queues outgoing data] --> B[ConnState.wake_driver]
    B --> C[driver_woken.set true]
    C --> D[request_executor_resume]
    D --> E[driver re-polled]
    E --> F[pump_transmit drains the data]
```

## 3. Items

| # | Item | Where | Do |
|---|---|---|---|
| R1 | `driver_wake` field and `wake_driver` | `foundation_netio/src/quic/state.rs` | Add `pub(crate) driver_wake: BoolSignal` to `ConnState`. Add `pub(crate) fn wake_driver(&self)` that calls `self.driver_wake.set(true)`. |
| R2 | `driver_woken` field | `foundation_netio/src/quic/quinn_driver.rs` | Add `driver_woken: BoolSignal` to `QuicDriver`, initialised `BoolSignal::new(false)` in `new`. |
| R3 | Share the latch | `foundation_netio/src/quic/quinn_driver.rs` | `insert_conn` stores `driver_wake: self.driver_woken.clone()` in each `ConnState`. |
| R4 | Clear the latch | `foundation_netio/src/quic/quinn_driver.rs` | `next_status` clears `driver_woken` before the read pass, next to the fd `clear`. |
| R5 | Compose the latch | `foundation_netio/src/quic/quinn_driver.rs` | `next_status` composes `driver_woken` into the `Depends` (fd edge, deadline, or latch) via `AnyReadiness::either(...).with(...)`. |
| R6 | Wake on queued data | `foundation_netio/src/quic/quinn_impl.rs` | Call `wake_driver` after queueing: `reset`, `send` (write, when bytes written), `finish`, `stop_sending`, `close`, `send_datagram`, and `read`'s flow-control `finalize` (when `should_transmit`). |
| R7 | Regression test | `foundation_netio/tests/quic/mod.rs` | New `queued_reset_wakes_a_driver_parked_on_readiness`. Park a server driver on an injected readiness, queue a reset, assert the parked readiness becomes ready. |
| R8 | Integration regression | `foundation_http/tests/http3_server_tests.rs` | The existing reject test is the integration regression. Verify 20 runs in a row. |

## 4. Tasks

- [x] T1. Branch from `fix/io-01-valtron-parking` (work done directly on that branch).
- [x] T2. R1–R6: add the latch, share it, clear it, compose it, and wake on every queued
      outgoing data path. One commit.
- [x] T3. R7: add the deterministic regression test.
- [x] T4. Verify (section 5).
- [x] T5. Delete the §3 flaky-test backlog entry. Update `progress.md` and `learnings.md`.

## 5. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0

# The deterministic regression test and the whole QUIC suite.
python3 scripts/ci/ci.py run foundation_netio --profile uat

# The flaky integration test, 20 runs in a row.
for i in $(seq 1 20); do
  timeout 120 cargo test -p foundation_http --profile uat --features "multi quic" \
    --test http3_server_tests http3_server_rejects_a_request_it_has_no_handler_for -- --exact \
    || { echo "FAILED on run $i"; exit 1; }
done

cargo check --workspace --all-targets --profile uat
```

## 6. Done when

- [x] `queued_reset_wakes_a_driver_parked_on_readiness` passes.
- [x] `http3_server_rejects_a_request_it_has_no_handler_for` passes 20 runs in a row.
- [x] `foundation_netio` CI is green (both runs: default and `quic`).
- [x] `cargo check --workspace --all-targets` passes.
- [x] The §3 flaky-test backlog entry is deleted.

## 7. Review checklist (main session)

- [x] The latch is cleared before the read pass, not after, so a wake during the pass is not lost.
- [x] The latch is composed into the `Depends` only when an fd readiness signal is present (the
      `Delayed` fallback already re-polls).
- [x] Every stream-handle method that queues outgoing data wakes the driver, not only `reset`.
- [x] `wake_driver` is safe under the connection lock (`resume_idle` takes a different lock and
      never runs the driver inline).
- [x] The deterministic test fails on the old code (the latch was not in the `Depends`) and
      passes on the new code.

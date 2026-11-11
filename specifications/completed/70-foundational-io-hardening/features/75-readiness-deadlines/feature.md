---
feature: "valtron: a task parked on a readiness that contains a deadline wakes at that deadline; idle QUIC listener uses no CPU; then land feature 72 E1"
status: "done"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_core", "foundation_netio", "foundation_proxy"]
catalogue_rows: [82, 83, 84]
---

# Feature 75: readiness deadlines

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Found in feature 72; confirmed by the main session (2026-11-03).

## Problem

`foundation_core/src/valtron/executors/local.rs`: `Sleepable::deadline()` (~line 239) and
`remaining_duration()` (~248) return `None` for `Sleepable::Readiness`. So when a task parks
on `TaskStatus::Depends(signal)` and the signal is "socket ready OR deadline passed"
(`AnyReadiness::either(fd, DeadlinePassed(t))` in `foundation_netio/src/quic/quinn_driver.rs`
~546; `DeadlineReadiness` in `foundation_netio/src/native/readiness.rs` ~133, used for example by
the 100-continue wait from feature 43), the executor does not know a time when the task can
become ready. If nothing else wakes the executor, it parks with no deadline and the task sleeps
past its deadline: the QUIC driver misses its ACK-delay, loss-detection and flow-control
timers, and an upload stalls until the connection idles out (feature 72's flaky tests).

## E1. Readiness reports its deadline

- `EventReadiness`: add `fn deadline(&self) -> Option<std::time::Instant> { None }`.
- `AnyReadiness`: the earliest `deadline()` of its children (`None` if none have one).
  `AllReadiness` (if it exists): think it through and say what is right (the latest? none?).
- `DeadlinePassed` (quinn driver), `DeadlineReadiness` (netio) and any other time-based
  readiness (search `impl EventReadiness for` across backends): return their instant.
- `Sleepable::Readiness`: `deadline()` and `remaining_duration()` return `signal.deadline()`,
  so the executor's existing sleeper-deadline logic wakes it (single-thread and `multi`).
- Check the `multi` pool's idle park path uses the same deadlines (`idle_wake::park_idle`).

Tests (`foundation_core/tests/valtron/`, both feature sets; each must hang/fail on the current
code with a hard time limit):
- `task_parked_on_deadline_readiness_wakes_on_time` — the only task, parked on a readiness
  whose deadline is 200 ms away and which is never signalled; it runs within 200–400 ms.
- `any_readiness_wakes_at_the_earliest_child_deadline`.
- netio: `expect_continue_body_sent_on_time_with_no_other_activity` (the 100-continue timer
  fires on an otherwise idle executor).

## E2. An idle QUIC listener uses about 85 ms of CPU per second

Feature 72 measured this with no client connected. Find what loops (per-thread CPU sampling,
as feature 62 describes). Likely the driver re-parks with a deadline in the past, or a zero
timeout. Fix it; test `idle_quic_listener_uses_no_cpu` (thread CPU under 20 ms over 1 s).

## E3. Land feature 72 E1

Merge `origin/wip/f72-h3-request-body` (commit `7bbbf1b6a`) into the lane A branch, run its
three tests 20 times each (they were flaky only because of this bug), and finish feature 72
(status "done", delete the §9 entries). If a stall remains, report with evidence; do not add
re-check timers to hide it.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_core --profile dev
python3 scripts/ci/ci.py run foundation_netio --profile dev
cargo test -p foundation_proxy --profile dev --features quic
```

## Done when

- [x] Deadline-carrying readiness wakes on time in both executors; idle QUIC listener idle;
      feature 72 E1 merged with 20/20 flake runs. Backlog entries deleted. CI green.

## Result (2026-11-03)

- The measured cause differs from the problem text: deadlines were already honored, because a
  worker with only readiness sleepers re-checks them every 1 ms (`readiness_poll_quantum`). That
  re-check is the idle cost (92 ms of CPU per second for an idle H3 listener) and it hid a lost
  wake in `block_on`'s idle back-off (`SpinWait`), which stalled the feature 72 upload.
- Added `EventReadiness::deadline()` and `EventReadiness::needs_polling()` (rule: return `false`
  only if every transition to ready calls `request_executor_resume`, or happens at the deadline),
  `ResumeSignal`, the deadline plumbing in netio, http, wireguard, nativeapis and the quinn
  driver, and `needs_polling() == false` for the reactor readiness, `ResumeSignal`, `WakeSignal`
  and the deadline-only signals. `BoolSignal`, the queue types and the nativeapis fd types keep
  polling (backlog §9).
- `block_on`: sleeps until the nearest deadline, bounded by the quantum only when a sleeper needs
  polling; the idle back-off is bounded by that wait; the kill signal is checked after a sleep;
  `idle_wake::checking()` runs before every step. The H3 accept task and QUIC driver wake through
  `ResumeSignal`, and `ShutdownSignal::turn_on` resumes the executor.
- Tests: `idle_quic_listener_uses_no_cpu` (92 ms before, under 20 ms after; 20 of 20),
  `readiness_deadline_tests` in both feature sets (they need the new method, so they did not fail
  on the old code: the old executor woke on time by polling), `event_driven_cpu` (multi: about
  21 ms against 55 ms for a polling signal), `expect_continue_body_sent_on_time_with_no_other_activity`.

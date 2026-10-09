# Scheduler latency & the Depends spin-guard — 2026-07-14 fixes

Four executor changes landed together in `executors/local.rs`, found while
debugging BuildKit tar-export throughput (spec-54): a healthy 3.5 MB transfer
through valtron-driven H2 pumps was being killed by a false-positive panic and,
once that was fixed, crawled at ~30 KB/s. All four are scheduler-level; no task
API changed.

## 1. `Depends` violation counter never reset (correctness)

**Symptom:** `panic!("Task … returned State::Depends with an already-ready
signal 3 consecutive times. This is a task bug…")` killing a demonstrably
healthy pipe-consumer task mid-transfer.

**Mechanics:** when a task returns `State::Depends(signal)` and the signal is
*already ready*, the executor gracefully re-queues the task (no park) and
counts a "violation" in `depends_true_violations`; at
`DEPENDS_TRUE_PANIC_THRESHOLD` consecutive violations it panics. The tripwire
targets a real bug class: a task that parks on a condition it should have
consumed during its poll (e.g. parking on "queue non-empty" without draining
it) — such a task violates on **every** run, forever, and without the tripwire
becomes a silent busy-livelock.

But "already ready at inspection time" also happens benignly: a consumer polls
the pipe, finds it empty, returns `Depends(readiness)` — and the producer
(another thread) enqueues in the microseconds before the executor inspects the
signal. A TOCTOU race, not a bug. The heuristic that separates the two is the
**streak**: benign races break their streaks; buggy tasks never do.

The bug: **nothing ever reset the counter**, so "3 consecutive" silently meant
"3 total over the task's lifetime". Any long-lived hot pipe consumer
accumulates 3 races eventually — BuildKit's session receive loop hit it after
~29 s of sustained transfer.

**Fix:** the counter now resets at both "healthy" observation points:
- any non-`Depends` state returned by the task (checked at the entry of the
  state dispatch), and
- a genuine park (`Depends` whose signal is *not* ready).

## 2. Panic threshold 3 → 4096

Even with true streak semantics, back-to-back benign races are possible under
sustained saturation (producer keeps landing inside the race window). A real
livelock repeats **unboundedly**, so the threshold only needs to be finite to
catch it — it does not need to be small. Raised from 3 to 4096.

## 3. Violation warn rate-limited

One `tracing::warn!` per violation floods the log at wire speed during bursts.
Now logs the first 3 violations of a streak, then every 256th.

## 4. Wrong sleep bound: `max_duration()` → `min_duration()` (latency)

`schedule_and_do_work`, NoWork-with-sleeping-tasks path: the worker computed
its sleep from `sleepers.max_duration()` — the **furthest** deadline. Sleeping
to the max guarantees every nearer deadline fires late by the gap to the
longest sleeper (a 10 ms-polling I/O pump next to a 1 s sleeper wakes ~1 s
late, every cycle). Now sleeps to `min_duration()`, the nearest deadline.

## 5. Blind sleep vs readiness-parked tasks (latency)

`LocalThreadExecutor::block_on`, NoWork arm: the sleep was
`time_until_next_wakeup().unwrap_or(no_work_yield)` where `no_work_yield`
defaults to `DEFAULT_YIELD_WAIT_TIME` = **4 s**. `time_until_next_wakeup()` is
`None` when the only sleepers are **readiness-parked** (pipe waits) — those
have no deadline, and their wake comes from a *producer on another thread
setting a flag*, which cannot interrupt a plain `sleep()`. Net effect: every
cross-worker pipe handoff could stall for seconds while the consumer's worker
blind-slept.

**Fix:** a worker whose pending work includes readiness-parked tasks sleeps
`READINESS_POLL_QUANTUM` (1 ms) instead of `no_work_yield`. Timed sleepers
still sleep exactly to their nearest deadline; a genuinely idle worker (no
sleepers at all) still uses the long `no_work_yield`.

**Known limitation / future work:** this is bounded polling, not true wakeup.
The structural fix is a wakeable parker — readiness registration hands the
producer a handle that can interrupt the consumer worker's sleep
(condvar/parker), after which the quantum can go away entirely. Until then,
1 ms bounds cross-worker handoff latency at negligible idle cost (a worker
only polls at this rate while it actually hosts parked pipe waits).

## Validation

- `foundation_core` full suite: 613 passed / 0 failed.
- BuildKit end-to-end (spec-54): all 12 integration tests green, including a
  3.86 MB OCI tar export streamed through two nested H2 connections driven
  entirely by valtron pumps — previously: spin-guard panic at ~29 s, then
  ~30 KB/s crawl, then multi-second stalls; after: clean pass.

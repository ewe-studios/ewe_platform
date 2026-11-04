---
feature: "valtron: a single-thread DrivenStreamIterator can park an async consumer"
status: "done"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_core"]
catalogue_rows: [10]
---

# Feature 46: `DrivenStreamIterator` async consumer wake

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (backlog §4, found during spec 70 feature 01)

`executors/non_sendables.rs` `DrivenStreamIterator::wake` registers only on the output
queue. The iterator drives the engine inside `next`. A parked task becomes runnable when a
foreign thread flips its readiness (`WakeSignal`, a future's waker, a socket); that pushes
nothing to the queue and fires no queue wake. So an async consumer
(`into_collect_future` under smol/tokio, without feature `multi`) whose task stays parked
longer than about two blocking waits gets `Pending` with nothing to wake it: a hang.

## Design (as built; replaces the first design, which was written before feature 75)

1. `WakeableStream::wake_send(SendWakeCallback)`: a new defaulted method. `SendWakeCallback` is
   a `Box<dyn FnOnce() + Send>`. `WakeCallback` is not `Send` without `multi`, so it is not
   changed. The default adapts the callback to `wake`; every wrapper forwards both methods. The
   async bridges in `stream_future.rs` call `wake_send` with the consumer's `Waker`. `multi` takes
   the default and is unchanged.
2. `DrivenStreamIterator::wake_send` (single-thread only) keeps the consumer's latest wake in one
   slot, and arms three triggers on that slot. Each trigger is armed at most once at a time, so
   repeated polls cannot grow any list:
   - the output queue (as before);
   - `idle_wake::register_resume_listener`: a one-shot list that `resume_idle()` (called by
     `request_executor_resume`) takes and fires. It is cleared when it fires;
   - a timed wake on the shared `valtron-timer` thread (`timer::call_at`, no new thread). The
     delay is `LocalThreadExecutor::parked_wake_after()`: the nearest sleeper deadline, bounded by
     `readiness_poll_quantum` when a sleeper has `needs_polling() == true`; `None` (nothing armed)
     when no sleeper needs a timed wake. Replacing the handle cancels the previous timed wake.
3. The inner stream stays blocking (the rule from feature 01).

## Tests (`backends/foundation_core/tests/valtron/`; run without and with `--features multi`)

- `smol_consumer_wakes_after_long_park` (hangs on master; give it a 10 s timeout): a task
  parks on a `WakeSignal` flipped by another thread after 2 s; `into_collect_future` under
  smol completes.
- Same for tokio (dev-dependency already exists) and for a `DeadlineReadiness` (timer) park.
- Existing `*_drives_parked_future_task` tests still pass.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_core --profile dev
```
Flake check: 20 runs of each new test, in both feature sets.

## Done when

- [x] New tests hang on master (time out) and pass now, in default features; multi unchanged.
- [x] §4 entry deleted. CI green (focused local checks; the full CI run is the main session's).

---
feature: "valtron: a single-thread DrivenStreamIterator can park an async consumer"
status: "pending"
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

## Design

1. Let readiness sources register a wake. Find `EventReadiness` (and `WakeSignal`,
   `FdReadiness`, `DeadlineReadiness`, `AnyReadiness`) and add
   `fn register_wake(&self, cb: SendWakeCallback)` (a `Box<dyn FnOnce() + Send>`; the
   existing `WakeCallback` is not `Send` without `multi`, so add a Send variant rather than
   change it). Default implementation: none (returns `false` = not supported).
2. The single-thread engine keeps its sleepers (tasks parked on `Depends`). Add an engine
   method that registers `cb` on every current sleeper's readiness (one-shot; fire once on the
   first ready).
3. `DrivenStreamIterator::wake` registers on the queue **and** on the engine's sleepers. Keep
   the rule from feature 01: do not switch the inner stream to non-blocking mode.
4. Make sure `multi` is unchanged (its engines already wake through worker threads).

Stop and report to the main session before implementing if step 1 needs a change to more
than about 6 readiness types or to a public trait in another track's crate.

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

- [ ] New tests hang on master (time out) and pass now, in default features; multi unchanged.
- [ ] §4 entry deleted. CI green.

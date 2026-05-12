---
feature: pending-none-livelock
description: Fix unbounded CPU spin when tasks return Pending(None) through Init/Ignore states
status: pending
priority: critical
created: 2026-05-12
tasks:
  completed: 0
  uncompleted: 9
  total: 9
  completion_percentage: 0
dependencies: []
---

# Feature 00: Pending(None) Livelock Fix

## Problem

When a task returns `State::Pending(None)`, the executor pushes the entry back to the
**front** of the processing queue and returns `ProgressIndicator::CanProgress(None)`.

In `block_on()` (`local.rs:2090-2123`), the `CanProgress` case does nothing — it
immediately loops back to `run_once()` with no yield, backoff, or sleep:

```rust
for _ in 0..200 {
    match self.run_once() {
        ProgressIndicator::CanProgress(_) => {}  // ← no yield, immediate retry
        ...
    }
}
```

This creates a 100% CPU busy-loop polling the same task 200 times before any yield check.
If the task continues returning `Pending(None)`, the outer loop re-enters the 200-iteration
inner loop immediately.

**Why this is reachable through normal usage:** `TaskStatus::Init` and `TaskStatus::Ignore`
are both mapped to `State::Pending(None)` by `StreamConsumingIter` (`task_iters.rs:161-164`,
`191-194`). These are common signals for "initializing" or "nothing to report yet."

**Impact:** A single task returning `Init` or `Ignore` turns the executor into a CPU burner.

## Design Discussion

**Q: Doesn't the executor own a task to completion by design? If we let a worker take on
more tasks because the current one keeps returning Pending, won't that create worker
imbalance requiring task stealing?**

A: Yes — the single-task-to-completion ownership model is intentional and correct. The fix
is **not** about taking on more tasks or changing ownership. The worker stays focused on the
same task. The problem is what the CPU does *while* focused on that task.

When a task returns `Pending(None)`, the worker polls it again immediately at millions of
iterations per second — burning 100% CPU doing nothing useful. The fix is adding a
**micro-yield** between polls: the worker still owns the same task, still doesn't pull from
the global queue, still doesn't take on new work. It just stops burning CPU while waiting
for the task to become ready.

Think of it like a spinlock with backoff — you're still waiting on the same thing, you're
just not hammering the CPU while you wait.

The `run_until()` path actually handles this correctly — it falls through to the
sleeper-aware yield at the bottom after `CanProgress`. But `block_on()` doesn't, because
its inner 200-iteration loop treats `CanProgress` as "keep going immediately."

Workers take on new work only when they have space and no local work. That responsibility
is preserved. This fix is purely about CPU efficiency while the worker waits on its owned
task.

## Root Cause

`do_work()` at `local.rs:1055-1064`:
```rust
State::Pending(duration) => {
    ...
    let final_state = if let Some(inner) = duration {
        // sleeper path — correctly handled
    } else {
        // Pending(None) → push to FRONT, return CanProgress(None)
        self.processing.borrow_mut().push_front(top_entry);
        ProgressIndicator::CanProgress(None)
    };
```

And `block_on()` at `local.rs:2096-2097`:
```rust
ProgressIndicator::CanProgress(_) => {}  // no-op, immediate next iteration
```

There is no backoff, yield, or rotation for `Pending(None)` tasks.

## Approach

### Use the existing `ProcessController` / `self.yielder`

The `LocalThreadExecutor<T: ProcessController>` is already parameterized with a yielder:

- **Multi-threaded (`ThreadYielder`)**: Uses CondVar-based `wait_timeout_while()` — proper
  OS-level interruptible waiting. Located in `threads.rs:460-508`.
- **Single-threaded/WASM (`NoThreadController`)**: Uses `SpinWaiter` from foundation_nostd
  — iteration-based backoff with interrupt support. Located in `single/mod.rs:31-61`.

Both implement `ProcessController::yield_for(Duration)`. The executor already has
`self.yielder` available in `block_on()`, `block_until_finished()`, and `run_until()`.

### Where exactly to put `yield_for` — and why it matters

**Not all `CanProgress` returns are equal.** Tracing the flow from
`block_on()` → `run_once()` → `schedule_and_do_work()` → `do_work()`,
`CanProgress` comes back in several cases:

- **`CanProgress(Some(State::ReadyValue(_)))`** — a task produced a value. Real progress.
- **`CanProgress(Some(State::Progressed))`** — a task advanced. Real progress.
- **`CanProgress(Some(State::SpawnFinished(_)))`** — a task spawned. Real progress.
- **`CanProgress(Some(State::Reschedule))`** — a task asked to be rescheduled. Real progress.
- **`CanProgress(None)`** — no observable progress. This comes from:
  - `do_work()` when task returned `Pending(None)` (the problem case)
  - `do_work()` when a packed/sleeping task was skipped
  - `schedule_and_do_work()` when sleepers exist but have no max_duration
  - `schedule_and_do_work()` when a SpinWait was converted because inflight/global tasks exist

**The yield must only fire on `CanProgress(None)`, not `CanProgress(Some(_))`.**

Yielding on `CanProgress(Some(State::ReadyValue(_)))` would throttle a productive task
mid-stream. Yielding on `CanProgress(Some(State::SpawnFinished(_)))` would delay spawn
processing. These are active progress — the executor should keep going fast.

**The correct location is in `block_on()` and `block_until_finished()`**, at the
`CanProgress` match arm, with the counter distinguishing the two variants:

```rust
// In block_on() inner loop:
match self.run_once() {
    ProgressIndicator::CanProgress(None) => {
        idle_count += 1;
        if idle_count >= self.max_pending_spins {
            self.yielder.yield_for(self.pending_yield_duration);
            idle_count = 0;
        }
    }
    ProgressIndicator::CanProgress(Some(_)) => {
        idle_count = 0;  // real progress, reset counter
    }
    // ... other arms unchanged
}
```

**Why here and not deeper (in `do_work` or `schedule_and_do_work`)?**

- `do_work()` is on `ExecutorState`, which doesn't have access to `self.yielder` — that
  lives on `LocalThreadExecutor<T: ProcessController>`.
- `schedule_and_do_work()` same problem — it's on `ExecutorState`.
- The yielder is only accessible at the `LocalThreadExecutor` level, which is where
  `block_on()`, `block_until_finished()`, and `run_until()` live.

For `run_until()`, it already falls through to a `self.yielder.yield_for()` at the bottom
of every iteration (line 2022), which covers the `CanProgress(None)` case. But the yield
duration there is sleeper-aware (up to 100ms) which is much longer than needed for a
micro-yield. The counter approach should be added there too, with the existing sleeper-aware
yield as the fallback after the counter fires.

### Prerequisite: Reduce call sites via refactoring

Before adding the yield logic, consolidate the three loop methods to reduce duplication
and ensure the fix only needs to be applied in two places.

**Q: Can we merge `block_on` and `block_until_finished` to reduce call sites?**

A: Yes. The two methods are structurally identical — a loop of 200 `run_once()` calls with
yield logic between batches. The only difference is the exit condition:

| Aspect | `block_until_finished` | `block_on` |
|--------|----------------------|------------|
| Exit | `NoWork` → break | `kill_signal.probe()` → return |
| Kill signal | Not checked | Required (panics without it) |
| On `NoWork` | Break out entirely | Yield and continue |

This maps naturally to inferring behavior from `self.kill_signal`:
- If `kill_signal` is present → persistent worker mode (current `block_on` behavior)
- If `kill_signal` is absent → until-finished mode (current `block_until_finished` behavior)

The merged `block_on()` takes no parameters. Callers don't change — `block_on()` already
requires a kill signal for worker threads, and `block_until_finished()` callers switch to
`block_on()` (their executors have no kill signal, so they get until-finished behavior).

**Q: `run_until()` inlines the same code as `run_once()` — can we replace it?**

A: Yes. `run_until()` at lines 1986-1988 does:
```rust
let local_executor = self.state.local_engine();
let response = self.state.schedule_and_do_work(Box::new(local_executor));
```
That is exactly `run_once()`. Replace with `let response = self.run_once();` and add
`#[inline(always)]` to `run_once()` so the compiler eliminates the call overhead.

After this refactoring, `yield_for` only needs to be placed in two methods:
1. **`block_on()`** — the merged worker/until-finished loop
2. **`run_until()`** — the checker-based loop

### What to do

1. **Merge `block_on` and `block_until_finished`** into a single `block_on()` that infers
   behavior from `self.kill_signal`. Deprecate `block_until_finished` or remove it (only
   called from `single/mod.rs:164`).

2. **Replace inlined code in `run_until()` with `self.run_once()`** and mark `run_once()`
   with `#[inline(always)]`.

3. **Track consecutive `CanProgress(None)` returns** in `block_on()` and `run_until()` with
   a counter. Reset on `CanProgress(Some(_))`.

4. **After `max_pending_spins` consecutive `CanProgress(None)` polls, call
   `self.yielder.yield_for(pending_yield_duration)`** — a very small duration (configurable,
   default around 50μs) that:
   - On multi-threaded (`ThreadYielder`): does a short CondVar wait, interruptible if new
     work arrives or shutdown happens
   - On single-threaded (`NoThreadController`): does a `SpinWaiter` iteration-based pause

5. **Make `max_pending_spins` configurable** as a constant in `constants.rs` and as a field
   on `LocalThreadExecutor`. Default value: 8.

6. **Make the micro-yield duration configurable** — separate from the existing
   `no_work_yield` which is for "no work at all" scenarios. This is for "task says not
   ready yet" scenarios and should be much shorter. Default: 50μs.

### Why not just yield on every CanProgress(None)?

Yielding on every single `CanProgress(None)` would hurt throughput for tasks that briefly
return `Init` during setup then immediately become productive. The spin counter allows a
short burst of rapid polling (good for fast transitions) before backing off (good for
sustained pending states).

## Tasks

### Refactoring (prerequisite)

- [ ] TASK-00-01: Merge `block_until_finished()` into `block_on()`: infer mode from `self.kill_signal` — if present, run as persistent worker (yield on NoWork, check kill signal); if absent, break on NoWork. Remove `block_until_finished()` and update its call site in `single/mod.rs:164` to use `block_on()`
- [ ] TASK-00-02: In `run_until()` (`local.rs:1986-1988`): replace inlined `local_engine()` + `schedule_and_do_work()` with `self.run_once()`
- [ ] TASK-00-03: Change `run_once()` from `#[inline]` to `#[inline(always)]` (`local.rs:1953`)

### Pending yield fix

- [ ] TASK-00-04: Add `DEFAULT_MAX_PENDING_SPINS: usize = 8` and `DEFAULT_PENDING_YIELD_DURATION: Duration = 50μs` constants to `constants.rs`
- [ ] TASK-00-05: Add `max_pending_spins: usize` and `pending_yield_duration: Duration` fields to `LocalThreadExecutor`; initialize from constants in constructors (`new`, `from_seed`, `from_rng`)
- [ ] TASK-00-06: In `block_on()` inner loop: add `idle_count` counter; on `CanProgress(None)` increment and yield via `self.yielder.yield_for(self.pending_yield_duration)` when counter reaches `max_pending_spins`, reset counter; on `CanProgress(Some(_))` reset counter to 0
- [ ] TASK-00-07: In `run_until()`: add same `idle_count` counter; on `CanProgress(None)` after threshold, use `self.yielder.yield_for(self.pending_yield_duration)` instead of the existing sleeper-aware yield (which uses up to 100ms — too long); on `CanProgress(Some(_))` reset counter and skip yield

### Testing

- [ ] TASK-00-08: Add test for merged `block_on()`: verify it exits on NoWork when kill_signal is absent (until-finished mode) and only exits on kill_signal when present (worker mode)
- [ ] TASK-00-09: Add test that verifies CPU does not spin when a task returns `Pending(None)` repeatedly — use a counter task that returns `TaskStatus::Ignore` N times, verify yields occur after `max_pending_spins` bursts

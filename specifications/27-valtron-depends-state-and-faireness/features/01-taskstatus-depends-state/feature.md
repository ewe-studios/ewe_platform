---
feature: "TaskStatus::Depends State"
description: "Add signal-based task waiting via TaskStatus::Depends(Arc<AtomicBool>) wired to existing Sleepable::Atomic"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "large"
created: 2026-05-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 11
  total: 11
  completion_percentage: 0%
---

# Feature 01: TaskStatus::Depends State

## WHY: Problem Statement

A task may not always perform well if it uses `TaskStatus::Delayed(duration)` when it may undershoot or overshoot the condition it needs to wait for. Timing may not be the right communication mechanism.

Worse, if we depend on timing, we may miss important signals that require low-latency executions. But we do not want to bring in more heavy burden logic like channels or mutex-guarded condvars.

The existing `Sleepable::Atomic(Arc<AtomicBool>, Entry)` variant in `local.rs:56-58` already supports signal-based sleeping with proper `Waiter` impl. What is missing is the wire from `TaskStatus` to this registration.

## WHAT: Solution

### TaskStatus Enum Changes (task.rs)

Add a new variant:

```rust
pub enum TaskStatus<D, P, S: ExecutionAction> {
    // ... existing variants ...
    /// Wait for an external signal (Arc<AtomicBool>) rather than a timed duration.
    /// The signal must be false when returned. If true, treated as Pending.
    Depends(sync::Arc<sync::atomic::AtomicBool>),
}
```

**`From<TaskStatus>` for `Stream<D, P>`:**

```rust
TaskStatus::Depends(_) => Stream::Ignore,
```

Streams do not care about this internal mechanism -- it is between the task and the worker.

**`PartialEq` impl:**

```rust
(TaskStatus::Depends(_), TaskStatus::Depends(_)) => true,
```

Variant-level match only -- identity of the `Arc<AtomicBool>` does not matter.

**`Display` and `Debug` impls:**

Add `Depends` to the internal `TStatus` debug enum:

```rust
enum TStatus<D, P> {
    Delayed(time::Duration),
    Pending(P),
    Ready(D),
    Init,
    Spawn,
    Ignore,
    Depends,
}
```

### State::Depends Handler in `do_work()` (local.rs)

```rust
State::Depends(signal) => {
    // 1. Check if task already has a sleeper record -> PANIC
    //    A sleeping task should never reach this point.
    //    It must be woken and its sleeper record removed before
    //    it can send Depends again.

    // 2. If signal is already true -> treat as Pending(None)
    //    Push to BACK of processing queue (not front).
    //    Track consecutive violations -- after 3 -> PANIC
    //    (task is misbehaving, loud fail)

    // 3. Signal is false -> register as Sleepable::Atomic(signal, entry)
    //    Remove from processing queue, let sleepers manage wake timing
}
```

### Cycle Counter (Zombie Detection)

Add to `ExecutorState`:

```rust
/// Track how many schedule_and_do_work() cycles each Depends sleeper has been waiting.
/// Only Sleepable::Atomic registrations get a cycle counter.
/// Sleepable::Timable entries are NEVER counted or nudged.
pub depends_cycle_counts: Rc<RefCell<HashMap<Entry, u32>>>,
```

**Re-poll guard:** After **10,000 cycles** without the signal flipping, forcibly remove the sleeper and re-schedule the task. If the task returns `Depends` again, re-register it with a fresh counter.

**Rationale:** A task using `Depends` guarantees it will be signalled, but the executor retains the right to forcibly wake it after prolonged inactivity (10,000 cycles) to prevent indefinite starvation.

### Ordering in `schedule_and_do_work()`

1. `wakeup_ready_sleepers()` -- removes Atomic sleepers whose signal flipped to true
2. Increment cycle counters for remaining `Sleepable::Atomic` entries
3. Force-wake those exceeding threshold (10,000 cycles)

### Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEPENDS_ZOMBIE_CYCLE_THRESHOLD` | 10,000 | Max cycles before forced re-poll |
| `DEPENDS_TRUE_PANIC_THRESHOLD` | 3 | Consecutive `Depends(true)` violations before panic |

## HOW: Implementation Steps

### Step 1: Add `TaskStatus::Depends` variant

In `backends/foundation_core/src/valtron/task.rs`:
- Add `Depends(sync::Arc<sync::atomic::AtomicBool>)` variant
- Add `sync::atomic::AtomicBool` import if not present

### Step 2: Update `From<TaskStatus>` for `Stream`

In the `impl From<TaskStatus<D, P, S>> for Stream<D, P>`:
- Add `TaskStatus::Depends(_) => Stream::Ignore`

### Step 3: Update `PartialEq` impl

In the `impl PartialEq for TaskStatus`:
- Add `(TaskStatus::Depends(_), TaskStatus::Depends(_)) => true`

### Step 4: Update `Display` and `Debug` impls

In both impl blocks:
- Add `Depends` to the `TStatus` internal enum
- Add `TaskStatus::Depends(_) => TStatus::Depends` to the match

### Step 5: Check `State` enum relationship

In `local.rs`, check if there is a separate `State` enum that mirrors `TaskStatus`. If so, add `State::Depends` variant.

### Step 6: Add `depends_cycle_counts` to `ExecutorState`

In `local.rs`:
- Add field: `pub depends_cycle_counts: Rc<RefCell<HashMap<Entry, u32>>>`
- Initialize in `ExecutorState::new()`: `Rc::new(RefCell::new(HashMap::new()))`
- Update `Clone` impl to create new `Rc<RefCell<HashMap>>` (not clone the inner data)
- Update `Debug` impl

### Step 7: Implement `State::Depends` handler in `do_work()`

Pattern match in the `do_work()` method:
- Check for existing sleeper record -> PANIC if found
- Check if signal is true -> track violations, push to back, PANIC after 3
- If signal is false -> register as `Sleepable::Atomic(signal, entry)`, remove from processing queue

### Step 8: Implement cycle counter increment

In `schedule_and_do_work()`, after `wakeup_ready_sleepers()`:
- Iterate remaining `Sleepable::Atomic` entries in sleepers
- Increment their cycle counter in `depends_cycle_counts`
- If counter >= 10,000 -> forcibly remove sleeper, re-add entry to processing queue

### Step 9: Add panic tracking for `Depends(true)`

Add to `ExecutorState`:
```rust
pub depends_true_violations: Rc<RefCell<HashMap<Entry, u32>>>,
```

In the `Depends(true)` handler:
- Increment violation count for this entry
- If >= 3 -> PANIC with descriptive message
- If < 3 -> push to back of processing queue as Pending

### Step 10: Unit Tests

Test cases:
1. Task returns `Depends(false)` -> gets registered as sleeper
2. External code flips signal -> task wakes up on next cycle
3. Task returns `Depends(true)` once -> treated as Pending
4. Task returns `Depends(true)` 3 consecutive times -> PANIC
5. Task's signal never flips for 10,000 cycles -> forced re-poll
6. Task re-registers `Depends` after being forced awake -> fresh counter
7. Cycle counter only increments for Atomic sleepers, not Timable

## Target Files

- `backends/foundation_core/src/valtron/task.rs` -- TaskStatus enum, PartialEq, Display, Debug, Stream
- `backends/foundation_core/src/valtron/executors/local.rs` -- ExecutorState fields, do_work handler, schedule_and_do_work cycle logic

## Tests

```bash
cargo test --package foundation_core -- valtron::task::tests::depends
cargo test --package foundation_core -- valtron::executors::local::tests::depends
cargo test --package foundation_core -- valtron::executors::local::tests::zombie_detection
cargo test --package foundation_core -- valtron::executors::local::tests::depends_true_panic
```

## Verification

```bash
cargo build --package foundation_core
cargo clippy --package foundation_core -- -D warnings
cargo fmt -- --check
cargo test --package foundation_core
```

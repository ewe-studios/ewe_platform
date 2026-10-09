---
feature: "TaskStatus::Depends State"
description: "Add signal-based task waiting via TaskStatus::Depends(Arc<dyn EventReadiness>) wired to existing Sleepable::Atomic"
status: "pending"
priority: "high"
depends_on: []
estimated_effort: "large"
created: 2026-05-15
updated: 2026-06-02
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

Using `Arc<AtomicBool>` directly ties the readiness mechanism to a single boolean. Tasks should be able to return any shareable readiness type — a file descriptor watcher, a condition variable, a mio-style poll token, a complex state object — anything that can answer the question "am I ready?" without the executor needing to know the details.

**Design principle:** If a task's readiness signal is never satisfied, it sleeps forever. It is not the executor's place to stop a task from being stupid.

## WHAT: Solution

### EventReadiness Trait

A new trait that any readiness signal must implement:

```rust
/// A trait for types that can answer "is this task ready to run?"
/// Implementors must be Send + Sync safe, as they will be shared
/// between the task (which may mutate the signal) and the executor
/// (which polls is_ready).
pub trait EventReadiness: Send + Sync {
    /// Check if the task is ready to run.
    ///
    /// `dur`: Optional timeout. If `None`, check immediately without blocking.
    /// If `Some(dur)`, may block up to `dur` waiting for readiness.
    ///
    /// Returns `true` if ready, `false` otherwise.
    fn is_ready(&self, dur: Option<Duration>) -> bool;
}
```

This trait allows tasks to return any type wrapped in `Arc<dyn EventReadiness>` that the system can poll to determine readiness. Types that implement this could be:
- A simple `AtomicBool` wrapper (trivial case)
- A file descriptor poll token
- A channel receiver
- A custom state machine
- A mio-style readiness set

### Blanket impl for AtomicBool

For convenience, `AtomicBool` gets a blanket impl via a newtype:

```rust
pub struct BoolSignal(Arc<AtomicBool>);

impl EventReadiness for BoolSignal {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}
```

### TaskStatus Enum Changes (task.rs)

Replace the `AtomicBool` variant with a trait-based one:

```rust
pub enum TaskStatus<D, P, S: ExecutionAction> {
    // ... existing variants ...
    /// Wait for an external readiness signal.
    /// The signal must return false on first receipt. If true, treated as Pending.
    /// Any type implementing EventReadiness can be used.
    Depends(sync::Arc<dyn EventReadiness>),
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

Variant-level match only -- identity of the `Arc<dyn EventReadiness>` does not matter.

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

### Sleepable::Atomic Changes (local.rs)

The existing `Sleepable::Atomic(Arc<AtomicBool>, Entry)` becomes:

```rust
pub enum Sleepable {
    Timable(Instant, Entry),
    /// A task waiting on an EventReadiness signal.
    /// The executor calls is_ready(None) to check without blocking.
    Atomic(sync::Arc<dyn EventReadiness>, Entry),
}
```

The `Waiter` impl for `Sleepable::Atomic` now calls `signal.is_ready(None)` instead of `atomic.load(Ordering::SeqCst)`.

### State::Depends Handler in `do_work()` (local.rs)

```rust
State::Depends(signal) => {
    // 1. Check if task already has a sleeper record -> PANIC
    //    A sleeping task should never reach this point.
    //    It must be woken and its sleeper record removed before
    //    it can send Depends again.

    // 2. If signal.is_ready(None) is already true -> treat as Pending(None)
    //    Push to BACK of processing queue (not front).
    //    Track consecutive violations -- after 3 -> PANIC
    //    (task is misbehaving, loud fail)

    // 3. Signal is false -> register as Sleepable::Atomic(signal, entry)
    //    Remove from processing queue, let sleepers manage wake timing
}
```

### Zombie Detection: Removed

**Design change:** If a task's `EventReadiness` never returns `true`, the task sleeps indefinitely. The executor does not enforce a zombie cycle threshold or forcibly re-poll. It is not the executor's responsibility to protect tasks from their own bugs — a task that waits forever on a signal that never fires is a task bug, not an executor concern.

This removes:
- `DEPENDS_ZOMBIE_CYCLE_THRESHOLD` constant
- `depends_cycle_counts` field from `ExecutorState`
- Cycle counter increment logic in `schedule_and_do_work()`
- Forced re-poll / forced wake logic for Atomic sleepers

### Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEPENDS_TRUE_PANIC_THRESHOLD` | 3 | Consecutive `Depends(true)` violations before panic |

## HOW: Implementation Steps

### Step 1: Add `EventReadiness` trait

In `backends/foundation_core/src/valtron/task.rs` (or a new `readiness.rs` module):
- Define the `EventReadiness` trait with `is_ready(&self, dur: Option<Duration>) -> bool`
- Require `Send + Sync` bounds
- Add `BoolSignal` newtype wrapping `Arc<AtomicBool>` with a trivial impl

### Step 2: Add `TaskStatus::Depends` variant

In `backends/foundation_core/src/valtron/task.rs`:
- Add `Depends(sync::Arc<dyn EventReadiness>)` variant
- Update imports

### Step 3: Update `From<TaskStatus>` for `Stream`

In the `impl From<TaskStatus<D, P, S>> for Stream<D, P>`:
- Add `TaskStatus::Depends(_) => Stream::Ignore`

### Step 4: Update `PartialEq` impl

In the `impl PartialEq for TaskStatus`:
- Add `(TaskStatus::Depends(_), TaskStatus::Depends(_)) => true`

### Step 5: Update `Display` and `Debug` impls

In both impl blocks:
- Add `Depends` to the `TStatus` internal enum
- Add `TaskStatus::Depends(_) => TStatus::Depends` to the match

### Step 6: Check `State` enum relationship

In `local.rs`, check if there is a separate `State` enum that mirrors `TaskStatus`. If so, add `State::Depends` variant.

### Step 7: Update `Sleepable::Atomic` to use `EventReadiness`

In `local.rs`:
- Change `Sleepable::Atomic(Arc<AtomicBool>, Entry)` to `Sleepable::Atomic(Arc<dyn EventReadiness>, Entry)`
- Update the `Waiter` impl to call `signal.is_ready(None)` instead of atomic load

### Step 8: Implement `State::Depends` handler in `do_work()`

Pattern match in the `do_work()` method:
- Check for existing sleeper record -> PANIC if found
- Check if `signal.is_ready(None)` is true -> track violations, push to back, PANIC after 3
- If false -> register as `Sleepable::Atomic(signal, entry)`, remove from processing queue

### Step 9: Remove zombie detection infrastructure

- Remove `DEPENDS_ZOMBIE_CYCLE_THRESHOLD` constant
- Remove `depends_cycle_counts: Rc<RefCell<HashMap<Entry, u32>>>` from `ExecutorState`
- Remove cycle counter increment logic from `schedule_and_do_work()`
- Remove forced re-poll / wake logic for Atomic sleepers
- Simplify `schedule_and_do_work()` ordering: only `wakeup_ready_sleepers()` needed

### Step 10: Add panic tracking for `Depends(true)`

Add to `ExecutorState`:
```rust
pub depends_true_violations: Rc<RefCell<HashMap<Entry, u32>>>,
```

In the `Depends(true)` handler:
- Increment violation count for this entry
- If >= 3 -> PANIC with descriptive message
- If < 3 -> push to back of processing queue as Pending

### Step 11: Unit Tests

Test cases:
1. Task returns `Depends(false)` -> gets registered as sleeper
2. External code flips signal -> task wakes up on next cycle
3. Task returns `Depends(true)` once -> treated as Pending
4. Task returns `Depends(true)` 3 consecutive times -> PANIC
5. Cycle counter no longer exists (verify removal)
6. Custom `EventReadiness` impl (e.g., a counter that flips after N calls) works correctly
7. `BoolSignal` blanket impl behaves identically to old `AtomicBool` approach

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

---
feature: "Integration and Testing"
description: "Wire TaskStatus::Depends, Worker Fairness Tracker, and TimeTracker together, plus integration tests"
status: "pending"
priority: "high"
depends_on: ["01-taskstatus-depends-state", "04-worker-fairness-tracker"]
estimated_effort: "large"
created: 2026-05-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---

# Feature 05: Integration and Testing

## WHY: Problem Statement

Features 01 (TaskStatus::Depends) and 04 (Worker Fairness Tracker) are implemented in isolation. This feature wires them together into the actual Valtron execution engine and adds integration tests to verify the mechanisms work correctly under real load.

## WHAT: Solution

### Integration Points

#### 1. ThreadRegistry owns Trackers

Add to `ThreadRegistry` struct:
```rust
/// Fairness tracker shared across all workers.
/// Created once in ThreadRegistry::new(), passed to each worker.
pub(crate) trackers: Arc<Trackers>,
```

In `ThreadRegistry::new()`:
```rust
let trackers = Arc::new(Trackers::new());
```

#### 2. spawn_worker passes trackers to LocalThreadExecutor

In `ThreadRegistry::spawn_worker()`:
```rust
// After creating ThreadRef and getting ThreadId:
let worker_tracker = self.trackers.register(thread_id.clone());

// Pass to LocalThreadExecutor::from_seed() or via ExecutorState builder
let thread_executor = LocalThreadExecutor::from_seed_with_trackers(
    seed_clone,
    worker_tag.clone(),
    task_clone,
    // ... existing params ...
    thread_id.clone(),           // NEW: worker_id
    worker_tracker,              // NEW: PackedAtomic<StatsSnapshot>
    self.trackers.clone(),       // NEW: Arc<Trackers>
);
```

#### 3. LocalThreadExecutor gets new fields

`ExecutorState` gets new fields:
```rust
/// This worker's thread ID, used for fairness tracker queries.
pub worker_id: ThreadId,

/// This worker's CAS-based stats handle. Used to update own stats.
pub worker_stats: PackedAtomic<StatsSnapshot>,

/// Global fairness tracker shared across all workers.
pub trackers: Arc<Trackers>,

/// Tracks how long each task has been processing.
pub task_timer: TimeTracker<Entry>,
```

#### 4. Stats update at start of schedule_and_do_work()

At the beginning of `schedule_and_do_work()`:
```rust
// Update own stats before doing any work
let total_tasks = ...;        // cumulative from some counter
let active_tasks = self.processing.borrow().len() as u16;
let total_waiters = self.sleepers.count() as u16;
let elapsed_ms = self.task_timer.elapsed_oldest()
    .map(|d| d.as_millis().min(65535) as u16)
    .unwrap_or(0);

self.worker_stats.update(|stats| StatsSnapshot {
    total_tasks,
    active_tasks,
    total_waiters,
    elapsed_ms,
});
```

#### 5. Fairness gate in request_global_task()

In `request_global_task()`:
```rust
// Check if we have active tasks - if so, don't take from global queue
if self.processing.borrow().len() > 0 {
    return CanProgress(None);
}

// Fairness gate: ask tracker if WE should take this task
if !self.trackers.can_take(&self.worker_id) {
    return CanProgress(None);  // Another worker deserves this task
}

// Proceed to pop from global queue
match self.global_tasks.pop() { ... }
```

#### 6. Task timer around task execution

In `do_work()`, when a task starts processing:
```rust
// Before calling the iterator:
self.task_timer.start(entry.clone());

// After getting result:
if let Some(elapsed) = self.task_timer.stop(&entry) {
    // Could log: tracing::debug!("Task {:?} took {:?}", entry, elapsed);
}
```

On cleanup paths (None/Panic/SpawnFailed), the existing `remove_sleepers_for_entry()` handles cleanup. Also clean up task timer:
```rust
self.task_timer.remove(&entry);
```

#### 7. Unregister on ThreadActivity::Stopped

In the worker thread's exit path (inside `spawn_worker`'s catch_unwind):
```rust
// After sending ThreadActivity::Stopped:
self.trackers.unregister(&self.worker_id);
```

Or in the activity listener loop that handles `ThreadActivity::Stopped`:
```rust
ThreadActivity::Stopped(thread_id) => {
    self.trackers.unregister(&thread_id);
    // ... existing cleanup ...
}
```

### ExecutorState Clone

`ExecutorState` is `Clone`. The new fields must be cloned correctly:
- `worker_id: ThreadId` -- clone directly (ThreadId is Clone)
- `worker_stats: PackedAtomic<StatsSnapshot>` -- clone the PackedAtomic (shares the same AtomicU64, which is correct -- both refer to same worker)
- `trackers: Arc<Trackers>` -- clone the Arc (shared across all workers)
- `task_timer: TimeTracker<Entry>` -- clone the Arc (shared within same executor state)
- `depends_cycle_counts: Rc<RefCell<HashMap>>` -- create new Rc<RefCell> with empty HashMap (cloned executor state should not share cycle counts)

## HOW: Implementation Steps

### Step 1: Add Arc<Trackers> to ThreadRegistry

In `threads.rs`:
- Add `trackers: Arc<Trackers>` field to `ThreadRegistry`
- Initialize in `new()` constructor
- Add to `Debug` impl

### Step 2: Pass trackers to LocalThreadExecutor

In `spawn_worker()`:
- Call `trackers.register(thread_id.clone())` to get worker's stats handle
- Pass `thread_id`, `worker_stats`, and `trackers` to executor constructor

### Step 3: Update ExecutorState

In `local.rs`:
- Add `worker_id`, `worker_stats`, `trackers`, `task_timer`, `depends_cycle_counts` fields
- Update `new()` constructor to accept these parameters
- Update `Clone` impl
- Update `Debug` impl

### Step 4: Integrate stats update

At start of `schedule_and_do_work()`:
- Gather current counts
- Call `worker_stats.update()` with new StatsSnapshot

### Step 5: Integrate fairness gate

In `request_global_task()`:
- Add `trackers.can_take(&worker_id)` check before popping from global queue

### Step 6: Integrate task timer

In `do_work()`:
- Call `task_timer.start(entry)` before iterator call
- Call `task_timer.stop(&entry)` after result
- Call `task_timer.remove(&entry)` in cleanup paths

### Step 7: Integrate unregister

On `ThreadActivity::Stopped`:
- Call `trackers.unregister(&thread_id)`

### Step 8: Integration tests -- fairness under load

Test scenario:
1. Create ThreadRegistry with 4 workers
2. Seed 100 tasks, each returns `TaskStatus::Delayed(100ms)` on first call
3. Verify no single worker accumulates more than ~25% of tasks
4. Repeat with 50% of tasks sleeping for 5 seconds
5. Verify fairness tracker prevents starvation

### Step 9: Integration tests -- TaskStatus::Depends with real signal flipper

Test scenario:
1. Create single-thread executor
2. Task returns `TaskStatus::Depends(signal)` where signal = false
3. In background thread, flip signal to true after 50ms
4. Verify task wakes up and completes
5. Test zombie detection: task whose signal never flips
6. Verify forced re-poll after 10,000 cycles

### Step 10: Full test suite

Run all tests across both packages to verify no regressions.

## Target Files

- `backends/foundation_core/src/valtron/executors/threads.rs` -- ThreadRegistry trackers field, spawn_worker integration, unregister on death
- `backends/foundation_core/src/valtron/executors/local.rs` -- ExecutorState fields, schedule_and_do_work stats update, request_global_task fairness gate, task_timer integration
- `backends/foundation_core/src/valtron/executors/local.rs` (tests) -- integration tests

## Tests

```bash
cargo test --package foundation_core -- valtron::executors::local::tests::integration_fairness
cargo test --package foundation_core -- valtron::executors::local::tests::integration_depends
cargo test --package foundation_core -- valtron::executors::threads::tests::integration
cargo test --package foundation_core
cargo test --package foundation_nostd
```

## Verification

```bash
cargo build --package foundation_core
cargo build --package foundation_nostd
cargo clippy --package foundation_core -- -D warnings
cargo clippy --package foundation_nostd -- -D warnings
cargo fmt -- --check
cargo test --package foundation_core
cargo test --package foundation_nostd
```

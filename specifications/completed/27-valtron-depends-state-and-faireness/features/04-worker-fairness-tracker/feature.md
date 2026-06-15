---
feature: "Worker Fairness Tracker"
description: "CAS-based fairness mechanism controlling which workers can take tasks from the global queue, preventing starvation"
status: "pending"
priority: "high"
depends_on: ["02-packed-atomic-utility"]
estimated_effort: "large"
created: 2026-05-15
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---

# Feature 04: Worker Fairness Tracker

## WHY: Problem Statement

Workers whose tasks all sleep for extended periods can accumulate many tasks while others sit idle. The current fairness mechanism (a simple counter forcing global queue checks every N calls) does not account for actual load distribution across workers.

User corrected my initial assumption: "No you are wrong, a worker can theoretically keep taking on more tasks if every tasks keeps going to sleep for extended period of time, that why we have this new faireness mechanism."

We need a CAS-based fairness mechanism that:
1. Tracks per-worker stats atomically (no mutex contention)
2. Decides which worker should take the next global task
3. Guarantees deadlock freedom when all workers are idle

## WHAT: Solution

### StatsSnapshot Bitfield (from Feature 02)

```
[bits 0-15]   total_tasks    (u16) - cumulative tasks this worker has handled
[bits 16-31]  active_tasks   (u16) - tasks currently being processed
[bits 32-47]  total_waiters  (u16) - sleeping + signal-waiting combined
[bits 48-63]  elapsed_ms     (u16) - ms since current task started, truncated to 65535
```

### Trackers Struct

```rust
pub struct Trackers {
    workers: RwLock<HashMap<ThreadId, PackedAtomic<StatsSnapshot>>>,
}

impl Trackers {
    /// Create empty trackers
    pub fn new() -> Self;

    /// Register a worker, returning its PackedAtomic<StatsSnapshot> handle.
    /// The worker uses this handle to update its own stats (no locking needed).
    pub fn register(&self, thread_id: ThreadId) -> PackedAtomic<StatsSnapshot>;

    /// Unregister a worker (called on ThreadActivity::Stopped).
    pub fn unregister(&self, thread_id: &ThreadId);

    /// Can this worker take a task from the global queue?
    /// Caller must already have active_tasks == 0.
    pub fn can_take(&self, asking_id: &ThreadId) -> bool;
}
```

### can_take() Logic

1. Read all workers' snapshots (acquire read lock on `workers`)
2. Identify candidates: workers with `active_tasks == 0` (caller already gates on this)
3. **Deadlock guarantee**: if ALL candidates have `total_waiters == 0` AND `total_tasks == 0`, return `true` for all (everyone is idle, free-for-all)
4. Otherwise, pick the winner:
   - lowest `total_waiters` wins
   - tiebreak: lowest `total_tasks` wins
   - tiebreak: lowest `elapsed_ms` wins
5. If asking worker IS the winner -> `true`, else `false`

### TimeTracker<K> (foundation_nostd)

```rust
pub struct TimeTracker<K: Eq + Hash + Clone> {
    entries: Arc<RwLock<HashMap<K, Instant>>>,
}

impl<K: Eq + Hash + Clone> TimeTracker<K> {
    /// Record Instant::now() for key
    pub fn start(&self, key: K);

    /// Remove and return elapsed duration for key
    pub fn stop(&self, key: K) -> Option<Duration>;

    /// Return the oldest entry's elapsed time
    pub fn elapsed_oldest(&self) -> Option<Duration>;

    /// Remove key without returning elapsed
    pub fn remove(&self, key: K) -> Option<Duration>;

    /// Return number of tracked entries
    pub fn count(&self) -> usize;
}
```

User: "Correction, we dont care for tasks to know each others duration, the worker tracks how long a tasks really takes to finish, both for metrics but also for better enhancement for us long term."

Used by `ExecutorState` -- call `start(entry)` when a task begins processing, `stop(entry)` when it finishes. The `elapsed_oldest()` feeds into `elapsed_ms` for the fairness tracker.

## HOW: Implementation Steps

### Step 1: Create TimeTracker

In `backends/foundation_nostd/src/atomics/time_tracker.rs`:
- Implement `TimeTracker<K>` with the 5 methods
- Use `Arc<RwLock<HashMap<K, Instant>>>` for storage
- `start()`: lock, insert `Instant::now()`
- `stop()`: lock, remove, return elapsed
- `elapsed_oldest()`: lock, find min Instant, return elapsed
- `remove()`: lock, remove, return Some(()) if existed
- `count()`: lock, return len

### Step 2: Implement `Trackers`

In `backends/foundation_core/src/valtron/executors/threads.rs` (or new file):
- `new()`: empty HashMap
- `register()`: create `PackedAtomic<StatsSnapshot>::new(StatsSnapshot::zero())`, insert into map, return clone
- `unregister()`: remove from map
- `can_take()`: implement priority logic

### Step 3: Implement StatsSnapshot

In `backends/foundation_nostd/src/atomics/packed.rs` (Feature 02):
- Already defined with `AtomicPackable` impl
- Add `StatsSnapshot::zero()` constructor
- Add `StatsSnapshot::update_from_timer(total_tasks, active_tasks, total_waiters, elapsed_ms)` helper

### Step 4: Unit Tests

**Trackers::can_take() test scenarios:**
1. All workers idle (total_waiters=0, total_tasks=0) -> all return true
2. One worker has lower total_waiters -> that worker wins
3. Equal total_waiters, one has lower total_tasks -> that worker wins
4. Equal total_waiters and total_tasks, one has lower elapsed_ms -> that worker wins
5. Asking worker is not the winner -> returns false
6. Unregistered worker calls can_take -> returns false

**TimeTracker<K> test scenarios:**
1. `start()` then `stop()` returns elapsed duration
2. `stop()` on non-existent key returns None
3. `elapsed_oldest()` with multiple entries returns oldest
4. `remove()` removes without returning duration
5. `count()` tracks active entries

## Target Files

- `backends/foundation_nostd/src/atomics/time_tracker.rs` (new) -- TimeTracker<K>
- `backends/foundation_nostd/src/atomics/mod.rs` -- export TimeTracker
- `backends/foundation_core/src/valtron/executors/threads.rs` -- Trackers struct (or new file)
- `backends/foundation_nostd/src/atomics/packed.rs` -- StatsSnapshot (from Feature 02)

## Tests

```bash
cargo test --package foundation_nostd -- atomics::time_tracker
cargo test --package foundation_core -- valtron::executors::threads::trackers
cargo test --package foundation_core -- valtron::executors::threads::tests::can_take
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

# Analysis: Valtron -- TaskStatus::Depends & Worker Fairness Tracker

## Files Context

- `backends/foundation_core/src/valtron/task.rs`
- `backends/foundation_core/src/valtron/executors/local.rs`
- `backends/foundation_core/src/valtron/executors/threads.rs`
- `backends/foundation_core/src/synca/sleepers.rs`
- `backends/foundation_nostd/src/`

---

## Current Architecture: Valtron Task Execution Engine

### Core Worker: `LocalThreadExecutor`

Each worker thread runs a `LocalThreadExecutor`, which owns an `ExecutorState`. The executor is the heart of task processing in Valtron.

#### `ExecutorState` (local.rs:456-529)

```
ExecutorState
+-- state_owner: String
+-- wakeup_priority: PriorityOrder
+-- global_tasks: Arc<ConcurrentQueue<BoxedSendExecutionIterator>>  -- shared queue
+-- current_task: Rc<RefCell<Option<Entry>>>
+-- local_tasks: Rc<RefCell<EntryList<BoxedExecutionIterator>>>
+-- task_graph: Rc<RefCell<HashMap<Entry, Entry>>>
+-- packed_tasks: Rc<RefCell<HashMap<Entry, bool>>>
+-- processing: Rc<RefCell<VecDeque<Entry>>>                         -- work queue
+-- rng: Rc<RefCell<ChaCha8Rng>>
+-- sleepers: Sleepers<Sleepable>                                    -- manages sleeping tasks
+-- idler: Rc<RefCell<IdleMan>>
+-- fairness_counter: Cell<usize>                                    -- forces global checks
+-- fairness_interval: Cell<usize>
```

Key properties:
- Workers share a global `ConcurrentQueue` for task distribution
- Workers never steal work -- tasks stay on their thread until completion
- A worker can hold at most 2 tasks (1 active + 1 overflow when the first sleeps), but theoretically more if all tasks sleep for extended periods
- Fairness is currently controlled by a simple counter (`fairness_counter`) that forces global queue checks every N calls

#### Core Execution Loop

```rust
pub fn schedule_and_do_work(&self, engine: BoxedExecutionEngine) -> ProgressIndicator {
    // Step 1: Try to get a task from the global queue
    match self.request_global_task() { ... }

    // Step 2: Wake up sleeping tasks whose time has come
    self.wakeup_ready_sleepers();

    // Step 3: Execute tasks from the processing queue
    match self.do_work(engine) { ... }
}
```

#### `do_work()` Task Status Handling (local.rs:1092+)

Tasks communicate their status via the `TaskStatus<D, P, S>` enum:

```rust
pub enum TaskStatus<D, P, S: ExecutionAction> {
    Delayed(time::Duration),     // Sleep for a duration
    Spawn(S),                    // Spawn an action
    Pending(P),                  // Not ready yet, try again next cycle
    Init,                        // Middle-point state
    Ready(D),                    // Task completed with result
    Ignore,                      // Skip this item
}
```

The `do_work()` method pattern-matches on the status and routes accordingly:
- `Delayed` -- register as `Sleepable::Timable`, remove from processing queue
- `Pending` -- push back to processing queue
- `Ready` -- mark task complete, wake up dependents
- `Spawn` -- execute the spawn action
- `Init` -- schedule for next cycle
- `Ignore` -- skip

### TaskStatus and Stream Conversion (task.rs:75-195)

`TaskStatus` implements `From<TaskStatus>` for `Stream<D, P>`:

```rust
impl<D, P, S: ExecutionAction> From<TaskStatus<D, P, S>> for Stream<D, P> {
    fn from(val: TaskStatus<D, P, S>) -> Self {
        match val {
            TaskStatus::Init => Stream::Init,
            TaskStatus::Spawn(_) => Stream::Ignore,
            TaskStatus::Ready(inner) => Stream::Next(inner),
            TaskStatus::Delayed(inner) => Stream::Delayed(inner),
            TaskStatus::Pending(inner) => Stream::Pending(inner),
            TaskStatus::Ignore => Stream::Ignore,
        }
    }
}
```

Also implements `PartialEq`, `Display`, and `Debug` -- each with an internal `TStatus` debug enum for rendering.

### Sleepable and Sleepers (local.rs:47-88)

```rust
pub enum Sleepable {
    Timable(DurationWaker<Entry>),       // Duration-based sleep
    Atomic(Arc<AtomicBool>, Entry),      // Signal-based sleep -- ALREADY EXISTS
}

impl Waiter for Sleepable {
    fn is_ready(&self) -> bool {
        match self {
            Sleepable::Timable(inner) => inner.is_ready(),
            Sleepable::Atomic(inner, _) => inner.load(atomic::Ordering::SeqCst),
        }
    }
}
```

**Critical finding:** `Sleepable::Atomic` already exists but is never connected to any `TaskStatus` variant. The `Waiter` impl properly loads the `AtomicBool` with `SeqCst` ordering. `Sleepers<Sleepable>` already manages it. The gap is purely the wire from `TaskStatus` to registration.

### ThreadRegistry (threads.rs:1283-1323)

```rust
pub struct ThreadRegistry {
    pub(crate) shared_tasks: SharedTaskQueue,
    pub(crate) latch: Arc<LockSignal>,
    pub(crate) kill_signal: Arc<OnSignal>,
    pub(crate) kill_latch: Arc<LockSignal>,
    activity_sender: mpp::Sender<ThreadActivity>,
    pub(crate) activity_receiver: mpp::Receiver<ThreadActivity>,
    pub(crate) waitgroup: WaitGroup,
    rng: Mutex<ChaCha8Rng>,
    pub(crate) max_threads: usize,
    pub(crate) live_threads: Arc<AtomicUsize>,
    idle_threads: Arc<AtomicUsize>,
    priority: PriorityOrder,
    op_read_time: time::Duration,
    yield_wait_time: time::Duration,
    thread_stack_size: Option<usize>,
    thread_max_idle_count: u32,
    thread_max_sleep_before_end: u32,
    thread_back_off_factor: u32,
    thread_back_off_jitter: f32,
    thread_back_min_duration: time::Duration,
    thread_back_max_duration: time::Duration,
    registry: SharedThreadRegistry,
    thread_handles: RwLock<HashMap<ThreadId, JoinHandle<ThreadExecutionResult<()>>>>,
    yielders: SharedThreadYielders,
}
```

Key methods:
- `new()` / `with_seed_and_threads()` -- constructors
- `spawn_worker()` -- creates a new worker thread with `LocalThreadExecutor`
- Thread lifecycle tracked via `ThreadActivity` channel (Started, Stopped, Panicked, etc.)

### Worker Spawn Flow (threads.rs:1598+)

```rust
pub fn spawn_worker(&self) -> ThreadExecutionResult<ThreadRef> {
    // 1. Create ThreadRef with shared_tasks, kill_signal, registry
    // 2. Register thread in SharedThreadRegistry -> gets ThreadId
    // 3. Register yielder for interrupt_all tracking
    // 4. Spawn thread via Builder::spawn()
    //    +- Inside thread:
    //       a. Send ThreadActivity::Started
    //       b. Create LocalThreadExecutor::from_seed()
    //       c. Run executor loop
    //       d. Send ThreadActivity::Stopped on exit
}
```

### Current Fairness Mechanism

The current fairness mechanism is a simple counter:
- `fairness_counter: Cell<usize>` increments on each `schedule_next()` call
- When `fairness_counter % fairness_interval == 0`, check global queue even if local tasks exist
- Default interval: 32 calls

This is a coarse heuristic that does not account for:
- How many tasks each worker is holding
- How many workers are idle vs. overloaded
- Whether tasks are sleeping for extended periods
- The actual load distribution across workers

---

## What Already Exists (Reusable Infrastructure)

### `Sleepable::Atomic` Variant (local.rs:56-58)

Already fully implemented with proper `Waiter` trait:

```rust
Sleepable::Atomic(sync::Arc<AtomicBool>, Entry)
```

The `Waiter::is_ready()` impl loads the atomic with `SeqCst` ordering, providing strong read-after-write guarantees. No changes needed here.

### `Sleepers<Sleepable>` (synca/sleepers.rs)

Already manages both `Timable` and `Atomic` sleepers. Provides:
- `add_sleeper()` -- register a new sleeper
- `wakeup_ready()` -- return entries whose sleepers are ready
- `remove_sleepers_for_entry()` -- cleanup for a specific entry (used in None/Panic/SpawnFailed paths)

No changes needed here either.

### `ConcurrentQueue` (foundation_nostd)

Provides lock-free queue with CAS-based operations. The fairness tracker's `Trackers` struct will follow similar patterns.

### `IdleMan` (synca/idleman.rs)

Manages idle backoff for the executor. Reusable as-is.

---

## Gap Analysis

### Gap 1: No `TaskStatus::Depends` Variant

**Current state:** `TaskStatus` has no way to express "wait for an external signal."
**Impact:** Tasks must use `TaskStatus::Delayed(duration)` which requires guessing timing. This leads to:
- **Undershooting:** Waking up too early, polling again, wasting cycles
- **Overshooting:** Waking up too late, missing low-latency signals
- **No guarantee:** Timing is not always the right communication mechanism

**Solution:** Add `TaskStatus::Depends(Arc<AtomicBool>)` variant. Wire it to the existing `Sleepable::Atomic` sleeper registration.

### Gap 2: No Zombie Detection for Signal Waiters

**Current state:** `Sleepers<Sleepable>` has no mechanism to detect signal waiters that never wake up.
**Impact:** A task whose signal is never flipped will sleep indefinitely.
**Solution:** Track cycle counts for `Sleepable::Atomic` entries only. Force re-poll after 10,000 cycles.

### Gap 3: No Atomic-Based Fairness Tracking

**Current state:** Fairness is a simple counter with no worker-level awareness.
**Impact:** Workers whose tasks all sleep for extended periods can accumulate many tasks while others sit idle.
**Solution:** Implement `Trackers` with `PackedAtomic<StatsSnapshot>` per worker, `can_take()` with priority rules.

### Gap 4: No Task Duration Tracking at Worker Level

**Current state:** No mechanism to track how long tasks take per worker.
**Impact:** Fairness decisions cannot factor in task execution time.
**Solution:** Add `TimeTracker<K>` to `ExecutorState`, start/stop around task execution, feed `elapsed_ms` into fairness snapshot.

### Gap 5: No `PackedAtomic<T>` Generic CAS Wrapper

**Current state:** No reusable CAS-based value wrapper in `foundation_nostd`.
**Impact:** Fairness tracker needs CAS-based worker stats, but Rust's stable only supports fixed-width atomics.
**Solution:** Implement `PackedAtomic<T: AtomicPackable>` -- encodes/decodes `T` to/from `u64`, CAS via `AtomicU64`.

### Gap 6: No Standalone `SignalWaiters<K>` Utility

**Current state:** No general-purpose signal waiter map.
**Impact:** Other code cannot easily manage keyed atomic signals.
**Solution:** Implement `SignalWaiters<K>` in `foundation_core/src/synca/` as a standalone utility (not used by valtron executor).

---

## Summary

The existing Valtron architecture already has the foundational pieces (`Sleepable::Atomic`, `Sleepers<Sleepable>`) to support signal-based task waiting. The gap is purely the wire from `TaskStatus` to registration. The fairness mechanism requires entirely new infrastructure (`Trackers`, `PackedAtomic`, `TimeTracker`) but follows established patterns (CAS-based operations, `ConcurrentQueue`-style design).

The implementation is well-scoped: 5 features, 3 independent (01, 02, 03), 1 dependent (04 on 02), and 1 integration feature (05 on 01, 04).

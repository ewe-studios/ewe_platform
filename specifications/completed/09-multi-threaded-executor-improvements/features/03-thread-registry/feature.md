---
feature: "ThreadRegistry"
description: "Central coordination replacing ThreadPool — holds shared queue, signals, activity channel, thread entries"
status: "completed"
priority: "high"
depends_on: ["02-waitgroup-and-pool-guard"]
estimated_effort: "large"
created: 2026-03-23
completed: 2026-03-26
author: "Main Agent"
tasks:
  completed: 12
  uncompleted: 0
  total: 12
  completion_percentage: 100%
---

# ThreadRegistry

## WHY: Problem Statement

The current `ThreadPool` struct (threads.rs:518-550) is a monolithic 30+ field struct that owns everything: the shared task queue, kill signals, latches, RNG, thread config, thread maps, thread handles, and the activity channel. It's tightly coupled — you can't use the lifecycle management without the thread spawning, can't use the task builder without the pool.

The analysis (see `analysis.md → threads.rs Component Review`) shows that `ThreadPoolTaskBuilder` only needs `SharedTaskQueue` + `Arc<LockSignal>`. The thread spawning logic in `create_thread_executor()` is good but buried inside `ThreadPool`. The activity listener (`listen_for_normal_activity`) has valuable logic for handling panics and respawning threads, but is coupled to `ThreadPool` internals.

## WHAT: Solution

### ThreadRegistry — What It Replaces

`ThreadRegistry` replaces `ThreadPool` as the coordination point. It takes the **useful parts** of `ThreadPool` and drops the rest:

**Kept from ThreadPool**:
- Shared task queue (`SharedTaskQueue`)
- Signal system (`OnSignal` for kill, `LockSignal` for latch and kill_latch)
- Activity channel (`mpp::Sender/Receiver<ThreadActivity>`)
- Thread config values (idle count, backoff, etc.)
- RNG for thread seeds
- Live/idle thread counters
- Thread handle storage

**Added**:
- `WaitGroup` for clean shutdown blocking
- `spawn_worker()` that gives each thread a `WaitGroupGuard`

**Removed** (no longer in the coordinator):
- `ThreadPool::spawn()` / `spawn2()` — callers construct `ThreadPoolTaskBuilder` directly
- `ThreadPool::kill()` — replaced by `PoolGuard::shutdown()` → `ThreadRegistry::shutdown()`
- `ThreadPool::run_until()` — replaced by `ThreadRegistry::block_until_done()`
- Parked/blocked thread tracking lists — each thread manages its own state

### Structure

```rust
pub struct ThreadRegistry {
    // Shared work distribution
    shared_tasks: SharedTaskQueue,
    latch: Arc<LockSignal>,

    // Kill coordination (uses existing signal system)
    kill_signal: Arc<OnSignal>,        // OnSignal::turn_on() broadcasts kill
    kill_latch: Arc<LockSignal>,       // sync between shutdown and activity loop

    // Activity monitoring
    activity_sender: mpp::Sender<ThreadActivity>,
    activity_receiver: mpp::Receiver<ThreadActivity>,

    // Shutdown coordination
    waitgroup: WaitGroup,

    // Thread management
    rng: Mutex<ChaCha8Rng>,
    max_threads: usize,
    live_threads: Arc<AtomicUsize>,
    idle_threads: Arc<AtomicUsize>,

    // Worker thread config
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

    // Thread tracking
    registry: SharedThreadRegistry,     // reuses existing SharedThreadRegistry
    thread_handles: RwLock<HashMap<ThreadId, JoinHandle<ThreadExecutionResult<()>>>>,
}
```

### Key Methods

#### `spawn_worker()` — Adapted from `ThreadPool::create_thread_executor()`

Same core logic: register thread in `SharedThreadRegistry`, create `ThreadYielder`, spawn `std::thread`, run `LocalThreadExecutor::from_seed()` + `block_on()` inside `catch_unwind`, send `ThreadActivity` events.

**Key difference**: Each spawned thread gets a `WaitGroupGuard` so `done()` is called on thread exit (including panics).

```rust
pub fn spawn_worker(&self) -> ThreadExecutionResult<ThreadRef> {
    // ... same setup as create_thread_executor ...

    let wg_guard = self.waitgroup.guard();  // NEW: WaitGroup tracking
    self.waitgroup.add(1);

    match builder.spawn(move || {
        let _wg = wg_guard;  // dropped on thread exit → calls done()

        match panic::catch_unwind(|| {
            sender.send(ThreadActivity::Started(sender_id.clone())).expect("send");

            let executor = LocalThreadExecutor::from_seed(
                seed, tasks, idle_man, priority, process, yield_wait,
                Some(kill_signal), Some(sender.clone()),
            );
            executor.block_on();

            sender.send(ThreadActivity::Stopped(sender_id.clone())).expect("send");
        }) {
            Ok(()) => Ok(()),
            Err(err) => {
                sender.send(ThreadActivity::Panicked(sender_id.clone(), err)).expect("send");
                sender.send(ThreadActivity::Stopped(sender_id)).expect("send");
                Ok(())
            }
        }
        // _wg dropped here → WaitGroup::done()
    }) {
        Ok(handle) => { /* store handle, increment live_threads */ }
        Err(err) => Err(ThreadExecutionError::FailedStart(Box::new(err))),
    }
}
```

#### `listen_for_activity()` — Adapted from `ThreadPool::listen_for_normal_activity()`

Same event handling logic. Key adaptations:
- On `ThreadActivity::Panicked`: prune dead thread, respawn if under `max_threads` and kill not signaled
- On `ThreadActivity::Stopped`: decrement live count, check if all dead
- Returns `None` when all threads stopped (loop exit condition)

#### `block_until_done()` — Replaces `run_until()` + `await_threads()`

```rust
pub fn block_until_done(&self) {
    // Run activity loop until all threads stopped
    loop {
        if self.listen_for_activity().is_none() {
            break;
        }
    }
    // Wait for WaitGroup (ensures all threads fully exited)
    self.waitgroup.wait();
    // Join all handles
    self.join_all_threads();
    // Signal kill_latch for anyone waiting
    self.kill_latch.signal_all();
}
```

#### `shutdown()` — Replaces `ThreadPool::kill()`

```rust
pub fn shutdown(&self) {
    self.kill_signal.turn_on();   // OnSignal broadcast
    self.latch.signal_all();      // Wake all blocked/idle threads
    self.waitgroup.wait();        // Block until all done
    self.join_all_threads();
    self.kill_latch.signal_all();
}
```

### Accessor Methods (for LocalPoolHandle)

```rust
impl ThreadRegistry {
    pub fn shared_tasks(&self) -> SharedTaskQueue { self.shared_tasks.clone() }
    pub fn latch(&self) -> Arc<LockSignal> { self.latch.clone() }
    pub fn kill_signal(&self) -> Arc<OnSignal> { self.kill_signal.clone() }
}
```

---

## What Gets Simplified in threads.rs

With `ThreadRegistry` as the coordinator, the following `ThreadPool` code can be **removed**:

| What | Lines | Why Removed |
|------|-------|-------------|
| `ThreadPool` struct definition | 518-550 | Replaced by `ThreadRegistry` |
| `ThreadPool::new()` constructor | 604-668 | Replaced by `ThreadRegistry::new()` |
| `ThreadPool::with_seed()` / `with_seed_and_threads()` / `with_rng()` | 556-596 | Convenience constructors for removed struct |
| `ThreadPool::spawn()` / `spawn2()` | 854-895 | Just `ThreadPoolTaskBuilder::new(tasks, latch)` — callers do this directly |
| `ThreadPool::kill()` | 914-928 | Replaced by `ThreadRegistry::shutdown()` + `PoolGuard` |
| `ThreadPool::run_until()` / `block_until()` | 975-1002 | Replaced by `ThreadRegistry::block_until_done()` |
| `ThreadPool::await_threads()` | 932-963 | Replaced by `ThreadRegistry::join_all_threads()` |
| `ThreadPool::create_thread_executor()` | 701-826 | Adapted as `ThreadRegistry::spawn_worker()` |
| `ThreadPool::schedule()` | 831-849 | Moved to free function or `LocalPoolHandle` |
| `ThreadPool::listen_for_normal_activity()` | 1028-1145 | Adapted as `ThreadRegistry::listen_for_activity()` |
| Parked/blocked thread list methods | 1004-1026 | Simplified — registry only tracks alive/dead |
| `parked_threads` / `blocked_threads` fields | 527-528 | No longer tracked centrally |

**What STAYS in threads.rs unchanged**:
- `ThreadYielder` + `ProcessController` impl (199-276)
- `ThreadId` (278-305)
- `ThreadActivity` enum (307-399)
- `SharedThreadRegistry` + `ThreadPoolRegistryInner` (402-514)
- `ThreadRef` (456-501)
- `ThreadExecutionError` (673-693)
- `ThreadPoolTaskBuilder` + all its methods (1148-1424)
- `get_allocatable_thread_count` / `get_max_threads` / `get_num_threads` (75-170)
- All test modules

---

## Tasks

- [ ] Define `ThreadRegistry` struct with fields from design above
- [ ] Implement `ThreadRegistry::new()` with defaults from `ThreadPool::new()`
- [ ] Implement `ThreadRegistry::spawn_worker()` adapted from `create_thread_executor()`
- [ ] Implement `ThreadRegistry::listen_for_activity()` adapted from `listen_for_normal_activity()`
- [ ] Implement `ThreadRegistry::block_until_done()` (activity loop + waitgroup + join)
- [ ] Implement `ThreadRegistry::shutdown()` using `OnSignal` + `LockSignal` + WaitGroup
- [ ] Implement `ThreadRegistry::join_all_threads()` adapted from `await_threads()`
- [ ] Implement accessor methods: `shared_tasks()`, `latch()`, `kill_signal()`
- [ ] Test: spawn N workers, all report `Started` activity
- [ ] Test: `shutdown()` kills all workers, `block_until_done()` returns
- [ ] Test: panicked worker gets replaced when under max_threads
- [ ] Test: panicked worker NOT replaced when kill signal is set

## Files Changed

- `backends/foundation_core/src/valtron/executors/threads.rs` (add ThreadRegistry, eventually remove ThreadPool)

## Verification

```bash
cargo test --package foundation_core -- thread_registry
cargo clippy --package foundation_core -- -D warnings
```

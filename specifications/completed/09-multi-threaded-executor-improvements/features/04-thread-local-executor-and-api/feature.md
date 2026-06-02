---
feature: "Thread-Local Executor and API Rewrite"
description: "thread_local! executor storage, LocalPoolHandle, rewrite multi/mod.rs public API"
status: "completed"
priority: "high"
depends_on: ["02-waitgroup-and-pool-guard", "03-thread-registry"]
estimated_effort: "large"
created: 2026-03-23
completed: 2026-03-26
author: "Main Agent"
tasks:
  completed: 11
  uncompleted: 0
  total: 11
  completion_percentage: 100%
---

# Thread-Local Executor and API Rewrite

## WHY: Problem Statement

The current `multi/mod.rs` stores a single `ThreadPool` in `OnceLock<ThreadPool>` and all public functions (`get_pool`, `spawn`, `spawn2`, `block_on`) access this global. This means:

- `get_pool()` returns the same `&'static ThreadPool` regardless of which thread calls it
- Cannot be reset between tests
- The `ctrlc::set_handler` is baked into `initialize_pool` — panics if handler already set
- No way to get deterministic cleanup (no drop handle)

## WHAT: Solution

### Thread-Local Storage

Each worker thread initialized by the registry gets its own `LocalThreadExecutor` stored in `thread_local!`:

```rust
thread_local! {
    static LOCAL_EXECUTOR: RefCell<Option<LocalThreadExecutor<ThreadYielder>>> =
        RefCell::new(None);
}
```

Worker threads set this up as part of `ThreadRegistry::spawn_worker()`. The main thread does NOT need a local executor — it only needs to submit tasks to the shared queue.

### LocalPoolHandle

A lightweight handle that provides the spawning API. It does NOT wrap a `LocalThreadExecutor` — it wraps the registry's shared queue and latch, which is all `ThreadPoolTaskBuilder` needs.

```rust
/// Handle for spawning tasks into the shared queue.
///
/// This is the new return type of `get_pool()`. It provides the same
/// spawn API surface as the old `&ThreadPool` by constructing
/// `ThreadPoolTaskBuilder` directly with the shared queue and latch.
pub struct LocalPoolHandle {
    shared_tasks: SharedTaskQueue,
    latch: Arc<LockSignal>,
    kill_signal: Arc<OnSignal>,
}

impl LocalPoolHandle {
    /// Create a task builder for scheduling work.
    /// Returns the SAME `ThreadPoolTaskBuilder` type as before.
    pub fn spawn<Task, Action>(&self) -> ThreadPoolTaskBuilder<
        Task::Ready, Task::Pending, Task::Spawner,
        Box<dyn TaskStatusMapper<...> + Send + 'static>,
        Box<dyn TaskReadyResolver<...> + Send + 'static>,
        Task,
    >
    where ...
    {
        ThreadPoolTaskBuilder::new(self.shared_tasks.clone(), self.latch.clone())
    }

    pub fn spawn2<Task, Action, Mapper, Resolver>(
        &self,
    ) -> ThreadPoolTaskBuilder<Task::Ready, Task::Pending, Action, Mapper, Resolver, Task>
    where ...
    {
        ThreadPoolTaskBuilder::new(self.shared_tasks.clone(), self.latch.clone())
    }

    /// Signal all threads to die.
    pub fn kill(&self) {
        self.kill_signal.turn_on();
        self.latch.signal_all();
    }
}
```

### Rewritten multi/mod.rs Public API

```rust
static REGISTRY: OnceLock<Arc<ThreadRegistry>> = OnceLock::new();

/// Initialize the thread pool.
///
/// Returns a `PoolGuard` — when dropped, kills all threads and blocks
/// until they're cleaned up. No ctrlc handler is registered.
///
/// The caller is responsible for signal handling. Example:
/// ```
/// let guard = initialize_pool(seed, Some(4));
/// ctrlc::set_handler(move || guard.shutdown()).ok();
/// ```
pub fn initialize_pool(seed: u64, thread_num: Option<usize>) -> PoolGuard {
    let registry = REGISTRY.get_or_init(|| {
        let num = thread_num.unwrap_or_else(get_allocatable_thread_count);
        let reg = Arc::new(ThreadRegistry::with_seed_and_threads(seed, num));
        for _ in 0..num {
            reg.spawn_worker().expect("should spawn worker");
        }
        reg
    });
    PoolGuard::new(registry.clone())
}

/// Get a handle for spawning tasks.
///
/// Works from ANY thread — the handle routes tasks to the shared queue
/// which all worker threads pull from.
///
/// # Panics
/// Panics if `initialize_pool` hasn't been called yet.
pub fn get_pool() -> LocalPoolHandle {
    match REGISTRY.get() {
        Some(registry) => LocalPoolHandle {
            shared_tasks: registry.shared_tasks(),
            latch: registry.latch(),
            kill_signal: registry.kill_signal(),
        },
        None => panic!("Thread pool not initialized, call initialize_pool first"),
    }
}

/// Initialize pool, run setup, block until all work completes.
///
/// The `PoolGuard` is held internally and dropped when this function
/// returns, ensuring all threads are cleaned up.
pub fn block_on<F>(seed: u64, thread_num: Option<usize>, setup: F)
where
    F: FnOnce(LocalPoolHandle),
{
    let guard = initialize_pool(seed, thread_num);
    let handle = get_pool();
    setup(handle);
    guard.registry.block_until_done();
    // guard drops here → shutdown
}

/// Spawn a task using boxed trait objects for mapper/resolver.
pub fn spawn<Task, Action>() -> ThreadPoolTaskBuilder<...> {
    get_pool().spawn::<Task, Action>()
}

/// Spawn a task with explicit mapper/resolver types.
pub fn spawn2<Task, Action, Mapper, Resolver>() -> ThreadPoolTaskBuilder<...> {
    get_pool().spawn2::<Task, Action, Mapper, Resolver>()
}
```

### What Gets Removed from multi/mod.rs

| What | Why |
|------|-----|
| `static GLOBAL_THREAD_POOL: OnceLock<ThreadPool>` | Replaced by `REGISTRY: OnceLock<Arc<ThreadRegistry>>` |
| `static CANCELATION_REGISTRATION: OnceLock<Option<()>>` | No ctrlc handler — caller manages signals |
| `ctrlc::set_handler(...)` in `initialize_pool` | Removed entirely — PoolGuard provides kill mechanism |
| Old `get_pool() -> &'static ThreadPool` | Replaced by `get_pool() -> LocalPoolHandle` |
| Old `initialize_pool() -> &'static ThreadPool` | Replaced by `initialize_pool() -> PoolGuard` |

---

## Tasks

- [ ] Define `LocalPoolHandle` struct with `spawn()`, `spawn2()`, `kill()`
- [ ] Define `thread_local!` storage for `LocalThreadExecutor` in worker threads
- [ ] Rewrite `initialize_pool()` to create `ThreadRegistry` + `PoolGuard`, no ctrlc
- [ ] Rewrite `get_pool()` to return `LocalPoolHandle` from `REGISTRY`
- [ ] Rewrite `block_on()` to use `PoolGuard` + `block_until_done()`
- [ ] Rewrite `spawn()` to delegate to `get_pool().spawn()`
- [ ] Rewrite `spawn2()` to delegate to `get_pool().spawn2()`
- [ ] Remove `GLOBAL_THREAD_POOL` and `CANCELATION_REGISTRATION` statics
- [ ] Remove `ctrlc::set_handler` from initialization path
- [ ] Ensure worker threads in `spawn_worker()` set up `thread_local!` executor
- [ ] Compile check: all types in `spawn()` return paths match existing `ThreadPoolTaskBuilder` signatures

## Files Changed

- `backends/foundation_core/src/valtron/executors/multi/mod.rs` (rewrite)
- `backends/foundation_core/src/valtron/executors/threads.rs` (LocalPoolHandle type)

## Verification

```bash
cargo build --package foundation_core
cargo clippy --package foundation_core -- -D warnings
```

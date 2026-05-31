---
description: "Add ValtronSingleton with Weak/Arc-backed singleton initialization to both single and multi-threaded valtron executors, providing explicit guard ownership, natural drop-based cleanup, and unified API surface."
status: "pending"
priority: "high"
created: 2026-06-01
updated: 2026-06-01
author: "Main Agent"
metadata:
  version: "1.0"
  estimated_effort: "small"
  tags:
    - valtron
    - singleton
    - single-threaded
    - multi-threaded
    - pool-guard
    - weak-arc
has_features: false
has_fundamentals: false
builds_on: []
related_specs:
  - "specifications/32-cf-serve-app"
---

# Valtron Executor — Singleton Initialization (Single & Multi)

## Problem

### Single Executor

The single-threaded valtron executor uses `thread_local! { static GLOBAL_LOCAL_EXECUTOR_ENGINE: OnceCell<LocalThreadExecutor> }` — the pool is hidden inside thread-local storage, and `initialize_pool(seed)` is a bare free function with no guard returned.

`PoolGuard` is a no-op stub (`#[derive(Default)] pub struct PoolGuard;`) that is not connected to the initialization lifecycle at all.

### Multi Executor

The multi-threaded executor uses two separate `Mutex<Option<...>>` statics:

```rust
static REGISTRY: Mutex<Option<Arc<ThreadRegistry>>> = Mutex::new(None);
static BG_REGISTRY: Mutex<Option<Arc<BackgroundJobRegistry>>> = Mutex::new(None);
```

`initialize_pool()` returns a `PoolGuard`, but it also stores the pool in globals independently. The guard and the statics are decoupled — dropping the guard doesn't clean up the globals, and the globals don't participate in ownership.

### Consequences (both executors)

- Users cannot hold a typed reference to the initialized pool.
- Cleanup is not natural — `Drop` does not trigger cleanup because the statics hold independent ownership.
- Tests that reset state must reach into internal globals manually.
- Inconsistent API surface between single (bare functions) and multi (functions + guard).

## Solution

Introduce `ValtronSingleton` — a zero-sized namespace struct backed by a **`Mutex<Weak<PoolState>>`** static — for both single and multi executors.

**Core design:**
- **Static holds a `Weak`** — never owns the pool, only tracks its existence.
- **`PoolGuard` holds the `Arc<PoolState>`** — strong ownership, natural `Drop` = cleanup.
- When **all** `PoolGuard` instances drop → `Arc` refcount hits 0 → `PoolState` drops → cleanup runs.
- The `Weak` in the static becomes dangling → next `get_or_init` re-initializes cleanly.
- No `unsafe`, no explicit `shutdown()` required, drop-based cleanup is natural.

## Architecture

```
Before (implicit, decoupled):

  Single:
    thread_local! { GLOBAL_LOCAL_EXECUTOR_ENGINE: OnceCell<LocalThreadExecutor> }
    initialize_pool(seed)          // bare function, no guard returned
    PoolGuard                      // decoupled stub

  Multi:
    static REGISTRY: Mutex<Option<Arc<ThreadRegistry>>> = Mutex::new(None);
    static BG_REGISTRY: Mutex<Option<...>> = Mutex::new(None);
    initialize_pool(seed, count)   // returns guard, but globals hold independent refs

After (explicit, unified):

  Both:
    static STATE: Mutex<Weak<PoolState>> = Mutex::new(Weak::new());
    PoolGuard { state: Arc<PoolState> }  // holds strong ownership
    ValtronSingleton::get_or_init(seed, setup) -> PoolGuard
    ValtronSingleton::guard() -> PoolGuard
    Drop → Arc refcount 0 → PoolState::drop → cleanup
```

## Part A: Single-Threaded Executor

### 1. `PoolState` — Pool Lifecycle Data

```rust
/// Internal pool state shared between all guards.
///
/// Holds the Arc<LocalThreadExecutor> that drives task scheduling and execution.
/// When all PoolGuard instances drop, the Arc refcount hits 0 and PoolState
/// drops — in single-threaded mode this is a no-op, but the structure is
/// ready for future cleanup hooks.
struct PoolState {
    engine: Arc<LocalThreadExecutor<DefaultController>>,
}

impl Drop for PoolState {
    fn drop(&mut self) {
        tracing::debug!("Single-threaded valtron pool dropped");
        // In single-threaded mode: no threads to join, no registries to clear.
        // The Arc<LocalThreadExecutor> drops naturally when all guards are gone.
    }
}
```

### 2. `PoolGuard` — Holds Strong Ownership

```rust
/// Lifecycle token for the single-threaded valtron executor.
///
/// Holds an `Arc<PoolState>` so the pool stays alive as long as any guard
/// instance exists. When all guards drop, `PoolState::drop` runs cleanup.
pub struct PoolGuard {
    state: Arc<PoolState>,
}

impl PoolGuard {
    fn new(state: Arc<PoolState>) -> Self {
        Self { state }
    }

    /// Access the underlying engine for spawning and execution.
    pub fn engine(&self) -> &LocalThreadExecutor<DefaultController> {
        &self.state.engine
    }

    /// Spawn a task into the executor's shared queue.
    pub fn spawn<Task, Action>(
        &self,
    ) -> ExecutionTaskIteratorBuilder<
        Task::Ready,
        Task::Pending,
        Task::Spawner,
        Box<dyn TaskStatusMapper<Task::Ready, Task::Pending, Task::Spawner> + 'static>,
        Box<dyn TaskReadyResolver<Task::Spawner, Task::Ready, Task::Pending> + 'static>,
        Task,
    >
    where
        Task::Ready: 'static,
        Task::Pending: 'static,
        Task: TaskIterator<Spawner = Action> + 'static,
        Action: ExecutionAction + 'static,
    {
        ExecutionTaskIteratorBuilder::new(self.state.engine.boxed_engine())
    }

    /// Run tasks until the checker closure returns true.
    pub fn run_until<S: Fn(ProgressIndicator) -> bool>(&self, checker: S) {
        self.state.engine.run_until(checker)
    }

    /// Run a single task tick.
    pub fn run_once(&self) -> ProgressIndicator {
        self.state.engine.run_once()
    }

    /// Run all pending tasks to completion.
    pub fn run_until_complete(&self) {
        self.state.engine.block_until_finished()
    }

    /// Shutdown is a no-op in single-threaded mode (no OS threads to join).
    pub fn shutdown(&self) {}
}

impl Clone for PoolGuard {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}
```

### 3. `ValtronSingleton` — Once-Init Namespace

```rust
/// Static tracking of pool existence. Never owns the pool — only holds a Weak.
///
/// When all PoolGuard instances drop, the Arc refcount hits 0 and the Weak
/// becomes dangling. Next get_or_init detects this and re-initializes.
static STATE: Mutex<Weak<PoolState>> = Mutex::new(Weak::new());

/// Zero-sized namespace for methods that manage the valtron executor singleton.
///
/// The pool is initialized exactly once per lifecycle. When all PoolGuard
/// instances drop, the pool is cleaned up and can be re-initialized.
pub struct ValtronSingleton;

impl ValtronSingleton {
    /// Initialize the valtron executor if not already initialized.
    ///
    /// Internally:
    ///   1. Lock STATE, check if existing Weak can be upgraded.
    ///   2. If yes, return a new PoolGuard wrapping the existing Arc.
    ///   3. If no, build the LocalThreadExecutor with the given seed,
    ///      wrap it in PoolState → Arc<PoolState> → PoolGuard,
    ///      store Weak in STATE, run setup closure, return guard.
    pub fn get_or_init<F: FnOnce(&PoolGuard)>(
        seed_for_rng: u64,
        setup: F,
    ) -> PoolGuard {
        let mut weak = STATE.lock().unwrap();
        if let Some(arc) = weak.upgrade() {
            return PoolGuard::new(arc);
        }
        let engine = LocalThreadExecutor::from_seed(
            seed_for_rng,
            "single-thread-pool".to_string(),
            Arc::new(ConcurrentQueue::unbounded()),
            // ... IdleMan, SleepyMan, etc. (same as current init)
        );
        let arc = Arc::new(PoolState {
            engine: Arc::new(engine),
        });
        *weak = Arc::downgrade(&arc);
        let guard = PoolGuard::new(arc.clone());
        setup(&guard);
        guard
    }

    /// Get a PoolGuard to the existing executor.
    ///
    /// Panics if no executor has been initialized.
    pub fn guard() -> PoolGuard {
        let weak = STATE.lock().unwrap();
        PoolGuard::new(
            weak.upgrade()
                .expect("Valtron executor not initialized — call ValtronSingleton::get_or_init() first"),
        )
    }

    /// Check if the executor has been initialized.
    pub fn is_initialized() -> bool {
        STATE.lock().unwrap().upgrade().is_some()
    }

    /// Force-reset the singleton state. cfg-gated to #[cfg(test)].
    ///
    /// Drops any existing pool by clearing the Weak. The Arc drops when
    /// all outstanding PoolGuard clones are gone. In tests, the guard
    /// returned by get_or_init is typically the only clone, so this
    /// effectively resets state immediately.
    #[cfg(test)]
    pub fn reset() {
        let mut weak = STATE.lock().unwrap();
        *weak = Weak::new();
    }
}
```

### 4. Free Functions — Delegate to Singleton

```rust
/// Initialize the valtron executor. Returns a PoolGuard.
///
/// Delegates to `ValtronSingleton::get_or_init` with an empty setup closure.
pub fn initialize_pool(seed_for_rng: u64) -> PoolGuard {
    ValtronSingleton::get_or_init(seed_for_rng, |_| {})
}

/// Run a single task tick. Panics if not initialized.
pub fn run_once() -> ProgressIndicator {
    ValtronSingleton::guard().run_once()
}

/// Run all pending tasks to completion. Panics if not initialized.
pub fn run_until_complete() {
    ValtronSingleton::guard().run_until_complete()
}

/// Run tasks until the checker closure returns true. Panics if not initialized.
pub fn run_until<S>(checker: S)
where
    S: Fn(ProgressIndicator) -> bool,
{
    ValtronSingleton::guard().run_until(checker)
}

/// Spawn a task into the executor. Panics if not initialized.
pub fn spawn<Task, Action>(
) -> ExecutionTaskIteratorBuilder<...>
where
    Task: TaskIterator + 'static,
    Action: ExecutionAction + 'static,
{
    ValtronSingleton::guard().spawn::<Task, Action>()
}
```

### 5. How Users Write a Single-Threaded Executor

**Explicit guard ownership (new pattern):**

```rust
let guard = ValtronSingleton::get_or_init(42, |pool| {
    pool.spawn::<MyTask, MyAction>()
        .with_resolver(Box::new(FnReady::new(|item, _| {
            println!("Got: {:?}", item);
        })))
        .schedule()
        .expect("should deliver task");
});

guard.run_until_complete();
// When guard drops → PoolState drops → cleanup
```

**Free functions (backward-compatible):**

```rust
let _guard = initialize_pool(42);

spawn::<MyTask, MyAction>()
    .with_resolver(resolver)
    .schedule()?;

run_until_complete();
```

**In WASM/CF Workers:**

```rust
#[wasm_bindgen]
pub fn init_worker() {
    ValtronSingleton::get_or_init(42, |pool| {
        // schedule tasks for each tick
    });
}

#[wasm_bindgen]
pub fn tick() {
    ValtronSingleton::guard().run_once();
}
```

---

## Part B: Multi-Threaded Executor

### 1. `PoolState` — Pool Lifecycle Data

```rust
/// Internal pool state shared between all guards.
///
/// Holds Arcs to both ThreadRegistry (task workers) and BackgroundJobRegistry.
/// When all PoolGuard instances drop, PoolState drops → signals threads to die,
/// joins all handles, clears registries.
struct PoolState {
    registry: Arc<ThreadRegistry>,
    bg_registry: Arc<BackgroundJobRegistry>,
    wait_group: Arc<WaitGroup>,
}

impl Drop for PoolState {
    fn drop(&mut self) {
        tracing::debug!("Multi-threaded valtron pool dropped — shutting down workers");
        // 1. Signal all threads to die
        self.registry.kill_signal().turn_on();
        self.bg_registry.kill_signal().turn_on();
        // 2. Wake all waiting threads
        self.registry.latch().signal_all();
        // 3. Wait for threads to finish
        self.wait_group.wait();
        // 4. Join all thread handles
        self.registry.join_all();
        self.bg_registry.join_all();
        // 5. Registries drop when Arc refcounts hit 0
    }
}
```

### 2. `PoolGuard` — Holds Strong Ownership

```rust
/// Lifecycle token for the multi-threaded valtron executor.
///
/// Holds an `Arc<PoolState>`. When all guards drop, `PoolState::drop`
/// kills workers, joins threads, and cleans up.
pub struct PoolGuard {
    state: Arc<PoolState>,
}

impl PoolGuard {
    fn new(state: Arc<PoolState>) -> Self {
        Self { state }
    }

    /// Get a handle for spawning tasks into the shared queue.
    pub fn pool_handle(&self) -> LocalPoolHandle {
        LocalPoolHandle {
            shared_tasks: self.state.registry.shared_tasks(),
            latch: self.state.registry.latch(),
            kill_signal: self.state.registry.kill_signal(),
            yielders: self.state.registry.yielders(),
        }
    }

    /// Spawn a task.
    pub fn spawn<Task, Action>(&self) -> ThreadPoolTaskBuilder<...>
    where
        Task: TaskIterator + Send + 'static,
        Action: ExecutionAction + Send + 'static,
    {
        self.pool_handle().spawn::<Task, Action>()
    }

    /// Signal all threads to die.
    pub fn kill(&self) {
        self.state.registry.kill_signal().turn_on();
        self.state.registry.latch().signal_all();
    }

    /// Wait for all tasks to complete.
    pub fn wait(&self) {
        self.state.wait_group.wait();
    }

    /// Shutdown: signal threads to die and wait for them.
    /// In multi-threaded mode this is meaningful — it kills workers.
    pub fn shutdown(&self) {
        self.kill();
        self.wait();
    }
}

impl Clone for PoolGuard {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}
```

### 3. `ValtronSingleton` — Same Namespace, Multi Implementation

The static is in the multi module, separate from single's static:

```rust
static STATE: Mutex<Weak<PoolState>> = Mutex::new(Weak::new());

impl ValtronSingleton {
    /// Initialize the multi-threaded executor if not already initialized.
    ///
    /// Internally:
    ///   1. Lock STATE, check if existing Weak can be upgraded.
    ///   2. If yes, return a new PoolGuard wrapping the existing Arc.
    ///   3. If no:
    ///      a. Split thread count between task workers and background jobs.
    ///      b. Create ThreadRegistry and BackgroundJobRegistry.
    ///      c. Spawn worker threads.
    ///      d. Wrap in PoolState → Arc → PoolGuard.
    ///      e. Store Weak in STATE, run setup closure, return guard.
    pub fn get_or_init<F: FnOnce(&PoolGuard)>(
        seed_for_rng: u64,
        user_thread_num: Option<usize>,
        setup: F,
    ) -> PoolGuard { ... }

    pub fn guard() -> PoolGuard { ... }
    pub fn is_initialized() -> bool { ... }

    #[cfg(test)]
    pub fn reset() {
        let mut weak = STATE.lock().unwrap();
        *weak = Weak::new();
    }
}
```

### 4. Free Functions — Delegate to Singleton

```rust
/// Initialize the multi-threaded executor. Returns a PoolGuard.
pub fn initialize_pool(seed_for_rng: u64, thread_num: Option<usize>) -> PoolGuard {
    ValtronSingleton::get_or_init(seed_for_rng, thread_num, |_| {})
}

/// Get a handle for spawning tasks.
pub fn get_pool() -> LocalPoolHandle {
    ValtronSingleton::guard().pool_handle()
}

/// Spawn tasks.
pub fn spawn<Task, Action>() -> ThreadPoolTaskBuilder<...> {
    get_pool().spawn::<Task, Action>()
}

/// block_on: init, setup, wait, return guard.
pub fn block_on<F>(seed: u64, threads: Option<usize>, setup: F) -> PoolGuard
where F: FnOnce(LocalPoolHandle)
{
    let guard = initialize_pool(seed, threads);
    setup(guard.pool_handle());
    guard.wait();
    guard
}
```

### 5. How Users Write a Multi-Threaded Executor

**Explicit guard ownership (new pattern):**

```rust
let guard = ValtronSingleton::get_or_init(42, Some(8), |pool| {
    pool.spawn::<MyTask, MyAction>()
        .with_resolver(resolver)
        .schedule()?;
});

// ... do other work ...

guard.wait();
// When guard drops → threads killed, joined, cleaned up
```

**block_on (convenience, backward-compatible):**

```rust
let _guard = block_on(42, None, |pool| {
    pool.spawn::<MyTask, MyAction>()
        .with_resolver(resolver)
        .schedule()?;
});
// When guard drops → cleanup
```

---

## File Changes

### Single Executor

| File | Change |
|------|--------|
| `foundation_core/src/valtron/executors/single/pool_guard.rs` | Rewrite `PoolGuard` to hold `Arc<PoolState>`; add `PoolState` struct with `Drop`; add `spawn`, `run_once`, `run_until`, `run_until_complete` methods |
| `foundation_core/src/valtron/executors/single/mod.rs` | Add `static STATE: Mutex<Weak<PoolState>>`; add `ValtronSingleton` with `get_or_init()`, `guard()`, `is_initialized()`, `reset()`; update free functions to delegate through `ValtronSingleton`; remove `thread_local!` and `GLOBAL_LOCAL_EXECUTOR_ENGINE` |

### Multi Executor

| File | Change |
|------|--------|
| `foundation_core/src/valtron/executors/multi/pool.rs` | Add `PoolState` struct with `Drop` (kill workers, join threads); rewrite `PoolGuard` to hold `Arc<PoolState>` instead of owning registries directly; add `static STATE: Mutex<Weak<PoolState>>`; add `ValtronSingleton`; update free functions to delegate |

No new dependencies. No changes outside `foundation_core/src/valtron/executors/`.

## Tasks

### Single Executor
1. [ ] Add `PoolState` struct to `pool_guard.rs` with `Arc<LocalThreadExecutor>` and `Drop` impl
2. [ ] Rewrite `PoolGuard` to hold `Arc<PoolState>` with `spawn`, `run_once`, `run_until`, `run_until_complete` methods
3. [ ] Add `static STATE: Mutex<Weak<PoolState>>` and `ValtronSingleton` to `mod.rs`
4. [ ] Update free functions to delegate through `ValtronSingleton`
5. [ ] Remove `thread_local! { GLOBAL_LOCAL_EXECUTOR_ENGINE }` and `OnceCell` imports
6. [ ] Update existing tests to use `ValtronSingleton::reset()` for test isolation

### Multi Executor
7. [ ] Add `PoolState` struct with `Drop` impl (kill workers, join threads, clear registries)
8. [ ] Rewrite `PoolGuard` to hold `Arc<PoolState>`
9. [ ] Add `static STATE: Mutex<Weak<PoolState>>` and `ValtronSingleton` to `pool.rs`
10. [ ] Update free functions to delegate through `ValtronSingleton`
11. [ ] Remove `REGISTRY` and `BG_REGISTRY` statics
12. [ ] Update existing tests to use `ValtronSingleton::reset()` for test isolation

### Verification
13. [ ] Verify native compilation: `cargo check -p foundation_core`
14. [ ] Verify wasm32 compilation: `cargo check -p foundation_core --target wasm32-unknown-unknown`
15. [ ] Run single executor tests: `cargo test -p foundation_core valtron::single`
16. [ ] Run multi executor tests: `cargo test -p foundation_core valtron::multi`

## Success Criteria

- [ ] `PoolGuard` holds `Arc<PoolState>` for both single and multi executors
- [ ] `ValtronSingleton::get_or_init()` initializes pool once and runs setup closure
- [ ] `ValtronSingleton::guard()` returns `PoolGuard` via `Weak::upgrade`, panics if not initialized
- [ ] `ValtronSingleton::is_initialized()` returns `bool`
- [ ] `ValtronSingleton::reset()` clears state in tests only (`#[cfg(test)]`)
- [ ] Free functions delegate through `ValtronSingleton` (backward-compatible)
- [ ] Drop of last `PoolGuard` triggers `PoolState::drop` → cleanup (threads killed, joined)
- [ ] All existing tests pass (may need `reset()` calls but test logic unchanged)
- [ ] `cargo check -p foundation_core` passes
- [ ] `cargo check -p foundation_core --target wasm32-unknown-unknown` passes

## Dependencies

This spec builds on the existing executor implementations:
- `LocalThreadExecutor` — `foundation_core/src/valtron/local.rs`
- `ThreadRegistry`, `BackgroundJobRegistry` — `foundation_core/src/valtron/executors/multi/`
- `PoolGuard` stubs — `foundation_core/src/valtron/executors/single/pool_guard.rs` and `foundation_core/src/valtron/executors/multi/threads.rs`
- `ExecutionTaskIteratorBuilder`, `SharedTaskQueue`, etc. — `foundation_core/src/valtron/`

---

_Created: 2026-06-01_

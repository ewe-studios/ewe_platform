# Analysis: Multi-Threaded Executor — Current State & Thread-Local Architecture

## Original Issues Identified

The following issues were identified in `backends/foundation_core/src/valtron/executors/multi/mod.rs` and form the basis for the architectural redesign:

### Issue 1: Global State Contamination Risk (Lines 16-17)
**Severity**: High — `OnceLock` statics cannot be reset between tests, causing potential interference between test runs.

```rust
static CANCELATION_REGISTRATION: OnceLock<Option<()>> = OnceLock::new();
static GLOBAL_THREAD_POOL: OnceLock<ThreadPool> = OnceLock::new();
```

### Issue 2: Panic-Based Error Handling (Lines 23-24, 97-99, 120-122)
**Severity**: High — `get_pool()`, `spawn()`, `spawn2()` all panic instead of returning Result types.

### Issue 3: Inefficient Lock Usage in DCounter (Lines 161-171)
**Severity**: Medium — `DCounter` acquires the same mutex 3 times in one method call.

### Issue 4: Signal Handler Conflicts (Lines 57-68)
**Severity**: Medium — `ctrlc::set_handler` uses `expect()`, panics if another handler already set.

### Issue 5: No Timeout Protection (Line 40)
**Severity**: Medium — `block_on` can block indefinitely if tasks don't complete.

### Issue 6: Lock Contention in Counter (Lines 195-205)
**Severity**: Medium — `Counter` holds mutex while sending on a channel that could block.

### Decision
Issues 3 and 6 (test helper lock optimizations) remain valid and should be fixed regardless.
Issues 1, 2, 4, and 5 are **superseded** by the thread-local architecture redesign below, which addresses all of them at an architectural level rather than as incremental patches.

---

## Current Architecture (Global Singleton)

### Problem Statement
The current `multi/mod.rs` uses a **global singleton pattern** with `OnceLock<ThreadPool>`:

```rust
static GLOBAL_THREAD_POOL: OnceLock<ThreadPool> = OnceLock::new();

pub fn get_pool() -> &'static ThreadPool {
    match GLOBAL_THREAD_POOL.get() {
        Some(pool) => pool,
        None => panic!("Thread pool not initialized"),
    }
}
```

### Issues with Current Approach

1. **Global Contention**: All threads access the same global queue, causing cache coherency traffic
2. **No Work Stealing**: Tasks stay in the global queue until picked up
3. **Centralized Management**: Single point of failure/bottleneck
4. **Test Isolation**: `OnceLock` cannot be reset between tests
5. **Complex Lifecycle**: ThreadPool manages worker threads internally, making cleanup difficult

### How It Works Now

1. `initialize_pool()` creates a `ThreadPool` with N worker threads
2. Each worker thread runs a `LocalThreadExecutor` with a shared global task queue
3. `spawn()` uses `GLOBAL_THREAD_POOL.get()` to access the pool
4. `execute_multi()` calls `multi::spawn()` which requires the global pool

---

## Proposed Architecture (Thread-Local Pools)

### Core Concept
Move from **one global pool** to **per-thread pools** where:
- Each thread owns its own `LocalThreadExecutor` in a `thread_local!` storage
- A global **registry** tracks thread handles (not pools) for lifecycle management
- `get_pool()` returns the **current thread's** local pool
- New threads are spawned on-demand when all existing threads are busy

### Architecture Components

#### 1. Thread-Local Pool Storage

```rust
thread_local! {
    static LOCAL_POOL: RefCell<Option<LocalThreadExecutor>> = RefCell::new(None);
}

/// Get the current thread's local pool, initializing if necessary
pub fn get_pool() -> LocalPoolGuard {
    LOCAL_POOL.with(|pool| {
        if pool.borrow().is_none() {
            // Initialize pool for this thread
            let new_pool = LocalThreadExecutor::new(...);
            *pool.borrow_mut() = Some(new_pool);

            // Register with global registry
            THREAD_REGISTRY.register_current_thread();
        }
        LocalPoolGuard { /* ... */ }
    })
}
```

#### 2. Thread Registry (Global)

Instead of a global pool, maintain a global **registry of thread handles**:

```rust
/// Information about a managed thread
struct ThreadInfo {
    id: ThreadId,
    handle: Option<JoinHandle<()>>,
    is_alive: Arc<AtomicBool>,
    spawned_at: Instant,
    task_count: Arc<AtomicUsize>,
}

/// Global registry of managed threads (not pools)
static THREAD_REGISTRY: OnceLock<Mutex<Vec<ThreadInfo>>> = OnceLock::new();

/// WaitGroup for clean shutdown
static SHUTDOWN_WAITGROUP: OnceLock<WaitGroup> = OnceLock::new();
```

#### 3. Pool Lifecycle Management

**Initialization Flow:**
1. First call to `get_pool()` on a thread initializes its local pool
2. Thread registers itself in the global registry
3. Registry tracks thread handle and liveness

**Work Distribution:**
- Tasks spawned via `spawn()` go to the **current thread's** local pool
- When a thread is at capacity, new tasks could:
  - a) Stay in that thread's queue (backpressure)
  - b) Be offloaded to a global work-stealing queue
  - c) Trigger spawning of a new thread

**Auto-Scaling:**
- When `get_pool()` is called and the registry shows all threads are busy:
  - Spawn a new background thread with its own local pool
  - Add to registry
  - Return the current thread's pool (not the new one)
  - The new thread starts processing its own queue

**Cleanup Flow:**
- Threads check their own liveness flag periodically
- When `kill()` is called, set global kill flag
- Each thread's pool sees the kill flag and shuts down
- Thread updates registry on exit
- `block_on` uses WaitGroup to wait for all threads

#### 4. API Compatibility

**Current API:**
```rust
pub fn spawn<Task, Action>() -> ThreadPoolTaskBuilder<...>
pub fn get_pool() -> &'static ThreadPool
pub fn block_on<F>(seed: u64, thread_num: Option<usize>, setup: F)
```

**New API (backward compatible):**
```rust
/// Returns a handle to the current thread's pool
/// (previously returned global pool)
pub fn get_pool() -> LocalPoolHandle

/// Spawns into the current thread's local pool
/// (same API, different implementation)
pub fn spawn<Task, Action>() -> LocalTaskBuilder<...>

/// Initializes thread-local pools and blocks
/// (same API, spawns threads with local pools)
pub fn block_on<F>(seed: u64, thread_num: Option<usize>, setup: F)
```

---

## Implementation Challenges

### Challenge 1: `spawn` from Non-Pool Threads

**Problem:** What if `spawn()` is called from a thread not in the registry?

**Solutions:**
1. **Auto-initialization**: `spawn()` calls `get_pool()` which auto-initializes
2. **Dedicated spawner thread**: Always have one "spawner" thread that receives tasks
3. **Global fallback queue**: Tasks go to global queue, picked up by any thread

**Recommended:** Auto-initialization (simplest, matches user expectations)

### Challenge 2: `execute` Functions

**Current:** `execute_multi()` calls `multi::spawn()` which needs global pool

**New:** `execute_multi()` should:
1. Check if we're already in a managed thread (has local pool)
2. If yes: spawn into local pool
3. If no: auto-initialize as "implicit" thread or spawn to dedicated executor

### Challenge 3: Thread Handle Management

**Current:** `ThreadPool` spawns and manages worker threads internally

**New:** Registry needs to:
- Store `JoinHandle` for each spawned thread
- Detect when threads panic (use `catch_unwind`)
- Prune dead threads from registry
- Potentially respawn threads if count drops below minimum

### Challenge 4: `block_on` Behavior

**Current:**
```rust
pub fn block_on<F>(seed: u64, thread_num: Option<usize>, setup: F) {
    let pool = initialize_pool(seed, thread_num); // Creates ThreadPool
    setup(pool);
    pool.run_until(); // Blocks until done
}
```

**New:**
```rust
pub fn block_on<F>(seed: u64, thread_num: Option<usize>, setup: F) {
    // Initialize registry with N threads, each with local pool
    let registry = initialize_registry(seed, thread_num);

    // Current thread also gets a local pool
    let local_pool = get_pool();

    setup(&local_pool);

    // Block until all threads complete
    registry.wait_for_all();
}
```

### Challenge 5: Cross-Thread Task Spawning

**Use Case:** Thread A wants to spawn a task that runs on Thread B

**Current:** All tasks go to global queue, any thread picks up

**New Options:**
1. **Local-only**: Tasks always spawn on current thread (simplest)
2. **Targeted spawn**: `spawn_to(thread_id)` for explicit routing
3. **Work stealing**: Idle threads steal from busy threads' queues

**Recommended:** Start with local-only, add work stealing later

---

## Migration Strategy

### Phase 1: Core Thread-Local Infrastructure
1. Create `THREAD_LOCAL_POOL` storage
2. Implement `get_pool()` for thread-local
3. Create thread registry
4. Make `spawn()` work with thread-local

### Phase 2: Lifecycle Management
1. Implement thread auto-spawning
2. Add thread cleanup on exit
3. Implement `WaitGroup` for `block_on`
4. Add thread liveness checking

### Phase 3: Optimization
1. Add work stealing between threads
2. Implement per-thread task statistics
3. Add dynamic thread scaling

### Phase 4: Cleanup
1. Remove `ThreadPool` global singleton
2. Deprecate old APIs
3. Update tests for new architecture

---

## Benefits of New Architecture

1. **Better Cache Locality**: Tasks tend to run on the thread that spawned them
2. **Reduced Contention**: No global queue mutex
3. **Better Test Isolation**: Each test can have its own thread-local pool
4. **Simpler Lifecycle**: Threads manage their own pools, no central coordinator
5. **Auto-Scaling**: New threads spawned on demand
6. **Graceful Degradation**: If thread dies, others continue working

---

## Open Questions

1. **Should we keep a global work queue as fallback?**
   - Pros: Easier load balancing
   - Cons: Reintroduces contention

2. **How to handle thread-local storage limits?**
   - Some platforms have limited TLS space
   - Pool size may be significant

3. **Should `get_pool()` be fallible?**
   - Currently panics if not initialized
   - With auto-init, should always succeed
   - But what about after shutdown?

4. **How to expose thread count metrics?**
   - Registry can track active thread count
   - Should this be exposed in API?

---

## Code Sketch: New `multi/mod.rs`

```rust
use std::cell::RefCell;
use std::sync::{Arc, Mutex, OnceLock, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::thread::{self, JoinHandle, ThreadId};

// ============================================================================
// Thread-Local Pool Storage
// ============================================================================

thread_local! {
    /// Each thread has its own LocalThreadExecutor
    static LOCAL_EXECUTOR: RefCell<Option<LocalThreadExecutor>> = RefCell::new(None);

    /// Flag indicating this thread is managed by the registry
    static IS_MANAGED_THREAD: RefCell<bool> = RefCell::new(false);
}

// ============================================================================
// Global Thread Registry
// ============================================================================

/// Information about a managed thread
#[derive(Debug)]
struct ThreadEntry {
    id: ThreadId,
    handle: JoinHandle<()>,
    is_alive: Arc<AtomicBool>,
    task_count: Arc<AtomicUsize>,
}

/// Global registry of all managed threads
static THREAD_REGISTRY: OnceLock<Mutex<Vec<ThreadEntry>>> = OnceLock::new();

/// Global kill signal for all threads
static GLOBAL_KILL_SIGNAL: OnceLock<Arc<OnSignal>> = OnceLock::new();

/// Current number of alive threads
static ALIVE_THREAD_COUNT: OnceLock<Arc<AtomicUsize>> = OnceLock::new();

/// Get or initialize the thread registry
fn get_registry() -> &'static Mutex<Vec<ThreadEntry>> {
    THREAD_REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

// ============================================================================
// Public API
// ============================================================================

/// Get the current thread's local executor, initializing if necessary
///
/// This replaces the old global singleton pattern. Each thread gets
/// its own LocalThreadExecutor stored in thread-local storage.
pub fn get_executor() -> LocalExecutorHandle {
    LOCAL_EXECUTOR.with(|executor| {
        if executor.borrow().is_none() {
            // Initialize executor for this thread
            init_thread_local_executor();
        }
        LocalExecutorHandle { /* ... */ }
    })
}

/// Legacy compatibility - returns a handle that wraps the local executor
pub fn get_pool() -> ThreadPoolHandle {
    ThreadPoolHandle::new(get_executor())
}

/// Initialize the thread-local executor for the current thread
fn init_thread_local_executor() {
    let kill_signal = GLOBAL_KILL_SIGNAL
        .get_or_init(|| Arc::new(OnSignal::new()))
        .clone();

    let executor = LocalThreadExecutor::new(
        /* rng, etc */
        kill_signal,
    );

    LOCAL_EXECUTOR.with(|ex| {
        *ex.borrow_mut() = Some(executor);
    });

    // Mark as managed if called from registry
    IS_MANAGED_THREAD.with(|managed| {
        if *managed.borrow() {
            register_with_registry();
        }
    });
}

/// Spawn a task into the current thread's local executor
pub fn spawn<Task, Action>() -> TaskBuilder<...>
where
    Task: TaskIterator + Send + 'static,
    // ...
{
    let executor = get_executor();
    executor.spawn_builder()
}

/// Initialize the thread pool with N threads
///
/// Each thread gets its own LocalThreadExecutor in thread-local storage.
/// This function spawns the worker threads and returns a registry handle.
pub fn initialize_pool(seed: u64, thread_num: Option<usize>) -> RegistryHandle {
    let num_threads = thread_num.unwrap_or_else(get_allocatable_thread_count);

    // Initialize global kill signal
    let kill_signal = GLOBAL_KILL_SIGNAL
        .get_or_init(|| Arc::new(OnSignal::new()))
        .clone();

    // Spawn N threads, each with its own local executor
    for i in 0..num_threads {
        spawn_managed_thread(seed, i, kill_signal.clone());
    }

    RegistryHandle::new(num_threads)
}

/// Spawn a managed thread with its own local executor
fn spawn_managed_thread(seed: u64, index: usize, kill_signal: Arc<OnSignal>) -> ThreadId {
    let builder = thread::Builder::new()
        .name(format!("valtron-worker-{}", index));

    let handle = builder.spawn(move || {
        // Mark this as a managed thread
        IS_MANAGED_THREAD.with(|m| *m.borrow_mut() = true);

        // Initialize thread-local executor
        init_thread_local_executor();

        // Register with registry
        let thread_entry = ThreadEntry {
            id: thread::current().id(),
            // ...
        };

        // Run the executor loop
        LOCAL_EXECUTOR.with(|ex| {
            if let Some(ref mut executor) = *ex.borrow_mut() {
                executor.run_until_complete_or_killed();
            }
        });

        // Cleanup: mark as dead in registry
        mark_thread_dead(thread::current().id());
    }).expect("failed to spawn thread");

    thread::current().id()
}

/// Block the current thread, running the executor until complete
pub fn block_on<F>(seed: u64, thread_num: Option<usize>, setup: F)
where
    F: FnOnce(&ThreadPoolHandle),
{
    // Initialize managed threads
    let registry = initialize_pool(seed, thread_num);

    // Current thread also needs an executor
    let local_handle = get_pool();

    // Run setup
    setup(&local_handle);

    // Wait for all managed threads to complete
    registry.wait_for_all();
}

// ============================================================================
// Cleanup and Lifecycle
// ============================================================================

/// Mark a thread as dead in the registry
fn mark_thread_dead(thread_id: ThreadId) {
    if let Ok(mut registry) = get_registry().lock() {
        registry.retain(|entry| entry.id != thread_id);
    }

    if let Some(counter) = ALIVE_THREAD_COUNT.get() {
        counter.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Kill all managed threads
pub fn kill_all() {
    if let Some(signal) = GLOBAL_KILL_SIGNAL.get() {
        signal.signal();
    }
}

/// Check if we need to spawn more threads (auto-scaling)
fn maybe_spawn_additional_threads() {
    let registry = get_registry();
    let threads = registry.lock().unwrap();

    // Check if all threads are busy
    let all_busy = threads.iter().all(|t| {
        t.task_count.load(Ordering::Relaxed) > SOME_THRESHOLD
    });

    if all_busy && threads.len() < MAX_THREADS {
        // Spawn additional thread
        spawn_managed_thread(...);
    }
}
```

---

## Summary

The proposed architecture replaces the **global ThreadPool singleton** with **per-thread LocalThreadExecutor instances** stored in `thread_local!` storage. A global registry tracks thread handles for lifecycle management, allowing:

1. **Better performance** through cache locality and reduced contention
2. **Better testability** through thread-local isolation
3. **Better scalability** through on-demand thread spawning
4. **Better resilience** through independent thread lifecycles

The key insight is that `get_pool()` should return the **current thread's** pool, not a global one, enabling a more distributed execution model while maintaining API compatibility.

---

## threads.rs Component Review

A detailed review of `backends/foundation_core/src/valtron/executors/threads.rs` to determine what to **keep**, **adapt**, and **remove** in the new architecture.

### KEEP — Reusable Primitives

| Component | Lines | Why Keep |
|-----------|-------|----------|
| `ThreadYielder` | 199-276 | Implements `ProcessController` for thread parking/blocking via `LockSignal`. Each worker thread needs this to yield when idle. The new registry spawns workers that still need a `ProcessController` implementation. |
| `ThreadId` | 278-305 | Custom thread identity wrapping `Entry` + name. Used throughout the activity system. Reusable as-is. |
| `ThreadActivity` enum | 307-399 | Full lifecycle event enum (Started, Stopped, Blocked, Unblocked, Parked, Unparked, Panicked, BroadcastedTask). The new registry's activity listener needs this unchanged. |
| `ThreadRef` | 456-501 | Thread metadata struct (seed, task queue, kill signal, process controller). The registry will store these per-thread. |
| `SharedThreadRegistry` | 402-451 | Registry wrapper with `register_thread()`. Needs adaptation but core pattern is reusable. |
| `ThreadPoolRegistryInner` | 503-514 | `EntryList<ThreadRef>` storage. Usable as inner storage for new registry. |
| `ThreadPoolTaskBuilder` | 1148-1392 | The entire task builder with `schedule()`, `schedule_iter()`, `stream_iter()`, `ready_iter()`. This is the **main API surface** that `spawn()` returns. Critically, it only depends on `SharedTaskQueue` and `Arc<LockSignal>` — NOT on `ThreadPool`. Can be used unchanged. |
| `ThreadExecutionError` | 673-693 | Error type for thread spawn failures. Reusable. |
| `get_allocatable_thread_count()` | 75-106 | Thread count calculation logic. Reusable. |
| `get_max_threads()` / `get_num_threads()` | 141-170 | System thread detection + `VALTRON_NUM_THREADS` env var. Reusable. |
| Thread config constants | (from `executors/constants`) | `BACK_OFF_*`, `MAX_ROUNDS_*`, `DEFAULT_*` — all reusable for worker thread configuration. |

### ADAPT — Needs Modification

| Component | Lines | What Changes |
|-----------|-------|-------------|
| `ThreadPool::create_thread_executor()` | 701-826 | The thread spawn logic is good but currently registers into `ThreadPool`'s internal maps. Needs to register into the new `ThreadRegistry` instead. The `LocalThreadExecutor::from_seed()` call, `catch_unwind`, and activity sender pattern all stay. |
| `ThreadPool::schedule()` | 831-849 | Push to shared queue + signal latch. This logic moves to `LocalPoolHandle` or becomes a free function. |
| `ThreadPool::listen_for_normal_activity()` | 1028-1145 | Activity loop that handles Started/Stopped/Panicked/Parked events, prunes dead threads, respawns on panic. This is the **heart of lifecycle management**. Moves to the `ThreadRegistry` and now also signals the WaitGroup. The existing respawn-on-panic logic (lines 1131-1140) is exactly what the new registry needs. |
| `ThreadPool::kill()` | 914-928 | Kill signal + latch signaling + wait for death. In the new model, the **drop handler** on `PoolGuard` does this. Uses `OnSignal::turn_on()` (existing signal system) to broadcast kill to all threads, then WaitGroup wait. |
| `ThreadPool::run_until()` / `block_until()` | 975-1002 | Main blocking loop. In the new model, `block_on` calls registry's activity listener loop + WaitGroup wait instead. |
| `ThreadPool::await_threads()` | 932-963 | Join all thread handles. Moves to `ThreadRegistry::join_all()`. |

### REMOVE — No Longer Needed

| Component | Lines | Why Remove |
|-----------|-------|------------|
| `ThreadPool` struct | 518-550 | The central struct itself. Its fields are distributed: task queue → shared, kill signal → registry, thread maps → registry, latch → registry, rng → registry. The struct as a monolith is replaced by `ThreadRegistry` + thread-local executors. |
| `ThreadPool::new()` constructor | 604-668 | Creates the pool and eagerly spawns N threads. This logic moves to `initialize_pool()` in `multi/mod.rs` with the new registry. |
| `ThreadPool::with_seed()` / `with_seed_and_threads()` / `with_rng()` | 556-596 | Convenience constructors for the removed struct. |
| `ThreadPool::spawn()` / `spawn2()` | 854-895 | These just create `ThreadPoolTaskBuilder::new(self.tasks.clone(), self.latch.clone())`. In the new model, `multi::spawn()` does this directly with the shared queue and latch from the registry. |
| `parked_threads` / `blocked_threads` tracking | 1004-1026 | Parked/blocked thread lists tracked centrally. With thread-local pools, each thread manages its own idle state. The registry only needs alive/dead tracking. |

### Key Insight: ThreadPoolTaskBuilder is Independent

The most important finding is that `ThreadPoolTaskBuilder` (lines 1148-1392) only needs two things:
1. `SharedTaskQueue` — the shared `Arc<ConcurrentQueue<BoxedSendExecutionIterator>>`
2. `Arc<LockSignal>` — the latch to signal worker threads

It does **not** reference `ThreadPool` at all. This means `spawn()` in `multi/mod.rs` can construct a `ThreadPoolTaskBuilder` directly from the registry's shared queue and latch without any wrapper type.

### Signal System Usage

The existing signal primitives are used as follows and should be preserved:
- **`OnSignal`** — Global kill signal (`global_kill_signal`). Atomic flag threads poll to know when to die. The drop handler calls `turn_on()`.
- **`LockSignal`** — Thread wake-up latch. Signals idle threads to check for new work. Also used as `kill_latch` for coordinating the `kill()` → `run_until()` handshake.
- **`ThreadActivity` channel** — `mpp::Sender/Receiver<ThreadActivity>` for lifecycle events. The registry listens on this to detect thread death, panic, parking, etc.

The new architecture keeps all three mechanisms. The `ctrlc::set_handler` is **removed** from `initialize_pool` — users get a `PoolGuard` drop handle instead.

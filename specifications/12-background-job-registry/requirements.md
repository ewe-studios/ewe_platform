---
description: "Add a BackgroundJobRegistry to valtron that owns a fixed pool of background worker threads for executing blocking closures via a ConcurrentQueue, replacing ad-hoc thread spawning in ThreadedFuture and exposing a unified run_background_job API across single/multi/unified modules."
status: "pending"
priority: "high"
created: 2026-03-29
author: "Main Agent"
metadata:
  version: "1.0"
  last_updated: 2026-03-29
  estimated_effort: "medium"
  tags:
    - valtron
    - threading
    - background-jobs
    - executor
    - thread-pool
  skills:
    - rust-patterns
    - valtron-executors
  tools:
    - Rust
    - cargo
builds_on:
  - "specifications/09-multi-threaded-executor-improvements"
related_specs:
  - "specifications/08-valtron-async-iterators"
has_features: true
has_fundamentals: false
features:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0%
---

# Background Job Registry

## Overview

Introduce a `BackgroundJobRegistry` — a fixed-size thread pool dedicated to executing blocking, fire-and-forget closures. Unlike the `ThreadRegistry` (which drives `TaskIterator`-based cooperative work), the `BackgroundJobRegistry` runs submitted functions to completion on whichever worker thread picks them up, then loops back for more work.

This replaces the unbounded `std::thread::spawn` pattern currently used by `ThreadedFuture::execute()` and provides a general-purpose `run_background_job` function for any code that needs to offload blocking work without spawning a new OS thread each time.

## The Problem

`ThreadedFuture::execute()` (in `future_task.rs`) spawns a **new OS thread per call**:

```rust
let handle = std::thread::spawn(move || {
    // poll future, stream results...
});
```

This is problematic because:

1. **Unbounded thread creation** — Each `ThreadedFuture` call creates a new thread with no upper limit. Under load, this leads to thread explosion (OS scheduling overhead, stack memory exhaustion).
2. **No reuse** — Threads are created and destroyed per operation. The OS thread lifecycle cost (allocation, TLS setup, kernel scheduling) is paid every time.
3. **Disconnect from valtron** — The valtron executor carefully manages a fixed thread pool via `ThreadRegistry`, but `ThreadedFuture` bypasses this entirely, creating a parallel unmanaged threading system.
4. **No backpressure on spawning** — There is no mechanism to signal "too many background jobs in flight" — callers just keep spawning.

## Goals

1. **`BackgroundJobRegistry` struct** — Owns a fixed number of worker threads that listen on a `ConcurrentQueue<Box<dyn FnOnce() + Send>>` for jobs
2. **Thread allocation formula** — Split `thread_num` between `ThreadRegistry` and `BackgroundJobRegistry` so background jobs get ~30% (min 1) of threads
3. **Integration into `initialize_pool`** — Both registries created and stored during pool initialization in `multi/mod.rs`
4. **`run_background_job` API** — Unified function exposed through `single/mod.rs`, `multi/mod.rs`, and `unified.rs`
5. **`ThreadedFuture` migration** — Replace `std::thread::spawn` with `run_background_job` call
6. **Panic protection** — Worker threads catch panics and continue processing the next job

## Non-Goals

- No result channel or future-like return value from `run_background_job` — it returns `Result<()>` indicating submission success only. The submitted function is responsible for communicating its results (e.g., via `mpp::Sender`, shared state, etc.)
- No priority levels or job scheduling — simple FIFO queue
- No dynamic thread scaling — fixed count at initialization

---

## Architecture

### Thread Allocation Formula

When `thread_num` is the total number of allocatable threads:

```
bg_threads = max(1, thread_num / 3)
task_threads = thread_num - bg_threads
```

| thread_num | task_threads (ThreadRegistry) | bg_threads (BackgroundJobRegistry) |
|------------|-------------------------------|------------------------------------|
| 2          | 1                             | 1                                  |
| 3          | 2                             | 1                                  |
| 4          | 3                             | 1                                  |
| 5          | 4                             | 1                                  |
| 6          | 4                             | 2                                  |
| 7          | 5                             | 2                                  |
| 8          | 6                             | 2                                  |
| 9          | 6                             | 3                                  |
| 10         | 7                             | 3                                  |
| 12         | 8                             | 4                                  |
| 16         | 11                            | 5                                  |

The formula ensures `ThreadRegistry` always gets the larger share and `BackgroundJobRegistry` always gets at least 1 thread.

### Component Layout

```
BackgroundJobRegistry
├── job_queue: Arc<ConcurrentQueue<BackgroundJob>>    // bounded or unbounded
├── kill_signal: Arc<OnSignal>                        // shared with ThreadRegistry
├── waitgroup: WaitGroup                              // for shutdown coordination
├── thread_handles: Vec<JoinHandle<()>>               // worker join handles
├── yielder_config: ProcessYielderConfig              // yield duration when idle
└── num_threads: usize

BackgroundJob = Box<dyn FnOnce() + Send + 'static>
```

### Worker Thread Loop

Each background worker thread runs:

```
loop {
    if kill_signal.is_on() { break; }

    match job_queue.pop() {
        Ok(job) => {
            // Panic protection: catch_unwind
            let result = std::panic::catch_unwind(AssertUnwindSafe(job));
            if let Err(panic) = result {
                tracing::error!("Background job panicked: {:?}", panic);
                // Thread continues — does NOT die on panic
            }
        }
        Err(_) => {
            // Queue empty — yield for configured duration
            process_yielder.yield_for(yield_duration);
        }
    }
}
```

### Lifecycle

1. **Initialization** (`multi::initialize_pool`):
   - Compute `bg_threads` and `task_threads` from `thread_num`
   - Create `ThreadRegistry` with `task_threads`
   - Create `BackgroundJobRegistry` with `bg_threads`, sharing the same `kill_signal`
   - Store both in module-level statics

2. **Submission** (`run_background_job`):
   - Push closure onto `job_queue`
   - Return `Ok(())` if pushed, `Err(...)` if queue is closed (shutdown in progress)

3. **Shutdown** (`PoolGuard::drop`):
   - `kill_signal.turn_on()` (shared — stops both registries)
   - Wait on `BackgroundJobRegistry::waitgroup`
   - Join all background worker handles
   - Then proceed with existing `ThreadRegistry` shutdown

---

## Features

### Feature 01: BackgroundJobRegistry Core

**File**: `backends/foundation_core/src/valtron/executors/background.rs` (new)

Implement the `BackgroundJobRegistry` struct:

- `BackgroundJobRegistry::new(num_threads, kill_signal, yield_duration)` — Creates the registry and spawns worker threads
- `BackgroundJobRegistry::submit(job: impl FnOnce() + Send + 'static) -> Result<()>` — Pushes a job onto the queue
- `BackgroundJobRegistry::shutdown()` — Signals kill, waits for workers, joins handles
- Worker loop with `catch_unwind` panic protection
- `ProcessYielder`-style idle yielding when queue is empty (configurable duration via constant, similar to `DEFAULT_YIELD_WAIT_TIME`)
- `WaitGroup` integration for clean shutdown

**Tasks:**
- [ ] Define `BackgroundJob` type alias
- [ ] Implement `BackgroundJobRegistry` struct with fields
- [ ] Implement `new()` — spawn worker threads with WaitGroup tracking
- [ ] Implement worker loop with panic protection and idle yielding
- [ ] Implement `submit()` — push to ConcurrentQueue
- [ ] Implement `shutdown()` — kill signal + waitgroup + join
- [ ] Add module to `executors/mod.rs` and re-export
- [ ] Unit tests: submit and execute a job, panic recovery, shutdown semantics

### Feature 02: Thread Allocation and Pool Integration

**File**: `backends/foundation_core/src/valtron/executors/multi/mod.rs` (modify)

Integrate `BackgroundJobRegistry` into the multi-threaded pool:

- Add `BG_REGISTRY: Mutex<Option<Arc<BackgroundJobRegistry>>>` static alongside existing `REGISTRY`
- Modify `initialize_pool` to:
  1. Compute `bg_threads` and `task_threads` using the 30% formula
  2. Create `ThreadRegistry` with `task_threads`
  3. Create `BackgroundJobRegistry` with `bg_threads`, sharing `kill_signal` from `ThreadRegistry`
  4. Store both in their respective statics
- Modify `PoolGuard` to hold and shut down both registries
- Add `run_background_job()` function in `multi` module that calls `BG_REGISTRY.submit()`

**Tasks:**
- [ ] Add thread allocation formula function: `split_thread_count(total) -> (task_threads, bg_threads)`
- [ ] Add `BG_REGISTRY` static
- [ ] Modify `initialize_pool` to create and store both registries
- [ ] Modify `PoolGuard` to shut down `BackgroundJobRegistry` on drop
- [ ] Add `pub fn run_background_job(job) -> Result<()>` to multi module
- [ ] Tests: verify thread split at various `thread_num` values
- [ ] Tests: verify both registries start and stop correctly

### Feature 03: Single-Threaded and Unified API

**Files**:
- `backends/foundation_core/src/valtron/executors/single/mod.rs` (modify)
- `backends/foundation_core/src/valtron/executors/unified.rs` (modify)

Provide `run_background_job` across all execution modes:

**single/mod.rs:**
- Add `pub fn run_background_job(job: impl FnOnce() + Send + 'static) -> Result<()>` that **immediately executes** the closure inline (no threads in single-threaded mode)
- Wrap in `catch_unwind` for consistency with multi behavior

**unified.rs:**
- Add `pub fn run_background_job(job: impl FnOnce() + Send + 'static) -> Result<()>` that delegates:
  - WASM: calls `single::run_background_job`
  - Native without `multi`: calls `single::run_background_job`
  - Native with `multi`: calls `multi::run_background_job`
- Follows the exact same `#[cfg]` dispatch pattern as existing `execute()`, `send()`, etc.

**Tasks:**
- [ ] Add `run_background_job` to `single/mod.rs` (inline execution + catch_unwind)
- [ ] Add `run_background_job` to `unified.rs` with cfg-based dispatch
- [ ] Tests: single-threaded execution works correctly
- [ ] Tests: unified dispatch selects correct backend

### Feature 04: ThreadedFuture Migration

**File**: `backends/foundation_core/src/valtron/executors/future_task.rs` (modify)

Replace `std::thread::spawn` in `ThreadedFuture::execute()` with `unified::run_background_job`:

- The worker closure currently passed to `std::thread::spawn` becomes the closure passed to `run_background_job`
- `execute()` return type changes: no more `WorkerHandle` (the background registry owns the thread). Returns only the `Receiver<ThreadedValue<T, E>>`
- `WorkerHandle` struct can be removed (or kept as a no-op wrapper for API compatibility if needed, decision left to implementer)
- The `queue_size` and `backpressure_sleep` configuration remains on `ThreadedFuture` — these control the `mpp::bounded` channel behavior inside the submitted closure, not thread management

**Tasks:**
- [ ] Replace `std::thread::spawn` call with `unified::run_background_job`
- [ ] Update `execute()` return type (remove or adapt `WorkerHandle`)
- [ ] Update existing `ThreadedFuture` tests
- [ ] Verify `ThreadedFuture` works correctly with background registry

---

## Key Design Decisions

### Why a separate registry instead of reusing ThreadRegistry?

`ThreadRegistry` workers run `LocalThreadExecutor` which expects `TaskIterator`-based cooperative tasks with scheduling, priority, and idle management. Background jobs are fundamentally different — they're blocking, fire-and-forget closures that run to completion. Mixing these into the same worker loop would compromise the cooperative scheduling model.

### Why bounded vs unbounded queue?

Start with **unbounded** queue (matching `ThreadRegistry`'s task queue pattern). The `submit()` return value signals queue-closed (shutdown), not queue-full. If backpressure is needed later, switching to bounded is a one-line change in `BackgroundJobRegistry::new`.

### Why share kill_signal?

Both registries should shut down together. Sharing `kill_signal` from `ThreadRegistry` means `PoolGuard` only needs to turn on one signal to stop everything. The separate `WaitGroup` on `BackgroundJobRegistry` ensures we wait for background workers independently.

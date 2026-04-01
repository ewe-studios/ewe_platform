---
description: "Add a BackgroundJobRegistry to valtron that owns a fixed pool of background worker threads for executing blocking closures via a ConcurrentQueue, replacing ad-hoc thread spawning in ThreadedIterFuture and exposing a unified run_background_job API across single/multi/unified modules."
status: "completed"
priority: "high"
created: 2026-03-29
author: "Main Agent"
metadata:
  version: "1.0"
  last_updated: 2026-04-01
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
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
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
- [x] Define `BackgroundJob` type alias
- [x] Implement `BackgroundJobRegistry` struct with fields
- [x] Implement `new()` — spawn worker threads with WaitGroup tracking
- [x] Implement worker loop with panic protection and idle yielding
- [x] Implement `submit()` — push to ConcurrentQueue
- [x] Implement `shutdown()` — kill signal + waitgroup + join
- [x] Add module to `executors/mod.rs` and re-export
- [x] Unit tests: submit and execute a job, panic recovery, shutdown semantics

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
- [x] Add thread allocation formula function: `split_thread_count(total) -> (task_threads, bg_threads)`
- [x] Add `BG_REGISTRY` static
- [x] Modify `initialize_pool` to create and store both registries
- [x] Modify `PoolGuard` to shut down `BackgroundJobRegistry` on drop
- [x] Add `pub fn run_background_job(job) -> Result<()>` to multi module
- [x] Tests: verify thread split at various `thread_num` values
- [x] Tests: verify both registries start and stop correctly

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
- [x] Add `run_background_job` to `single/mod.rs` (inline execution + catch_unwind)
- [x] Add `run_background_job` to `unified.rs` with cfg-based dispatch
- [x] Tests: single-threaded execution works correctly
- [x] Tests: unified dispatch selects correct backend

### Feature 04: ThreadedIterFuture Migration

**File**: `backends/foundation_core/src/valtron/executors/future_task.rs` (modify)

Replace `std::thread::spawn` in `ThreadedIterFuture::execute()` with `unified::run_background_job`:

- The worker closure currently passed to `std::thread::spawn` becomes the closure passed to `run_background_job`
- `execute()` return type changes: no more `WorkerHandle` (the background registry owns the thread). Returns only the `Receiver<ThreadedValue<T, E>>`
- `WorkerHandle` struct can be removed (or kept as a no-op wrapper for API compatibility if needed, decision left to implementer)
- The `queue_size` and `backpressure_sleep` configuration remains on `ThreadedIterFuture` — these control the `mpp::bounded` channel behavior inside the submitted closure, not thread management

**Tasks:**
- [x] Replace `std::thread::spawn` call with `unified::run_background_job`
- [x] Update `execute()` return type (remove or adapt `WorkerHandle`)
- [x] Update existing `ThreadedIterFuture` tests
- [x] Verify `ThreadedIterFuture` works correctly with background registry

---

## Key Design Decisions

### Why a separate registry instead of reusing ThreadRegistry?

`ThreadRegistry` workers run `LocalThreadExecutor` which expects `TaskIterator`-based cooperative tasks with scheduling, priority, and idle management. Background jobs are fundamentally different — they're blocking, fire-and-forget closures that run to completion. Mixing these into the same worker loop would compromise the cooperative scheduling model.

### Why bounded vs unbounded queue?

Start with **unbounded** queue (matching `ThreadRegistry`'s task queue pattern). The `submit()` return value signals queue-closed (shutdown), not queue-full. If backpressure is needed later, switching to bounded is a one-line change in `BackgroundJobRegistry::new`.

### Why share kill_signal?

Both registries should shut down together. Sharing `kill_signal` from `ThreadRegistry` means `PoolGuard` only needs to turn on one signal to stop everything. The separate `WaitGroup` on `BackgroundJobRegistry` ensures we wait for background workers independently.

---

## Feature 05: ThreadedFuture and FutureIterator Feature Gating

**File**: `backends/foundation_core/src/valtron/executors/future_task.rs` (modify)

Provide two implementations of `ThreadedFuture` with identical APIs but different execution strategies based on the `multi` feature:

### Multi-threaded mode (`cfg(all(feature = "std", feature = "multi"))`)

`ThreadedFuture` uses the `BackgroundJobRegistry` via `run_background_job`:

- `execute()` submits a closure to the background pool
- The closure polls the future and streams results through an `mpp::Sender`
- Returns `impl Iterator<Item = ThreadedValue<T, E>>` that wraps the receiver

### Single-threaded std mode (`cfg(all(feature = "std", not(feature = "multi")))`)

`ThreadedFuture` returns a `FutureIterator` that polls the future inline:

- `execute()` returns `FutureIterator` directly (no channel, no thread spawn)
- Each `next()` call creates a waker/context and polls the future until it yields
- Once the inner iterator is acquired, subsequent `next()` calls pull from it
- Returns `impl Iterator<Item = ThreadedValue<T, E>>`

### No-std mode (`cfg(all(not(feature = "multi"), not(feature = "std")))`)

Same as single-threaded std mode, but uses `spin_loop()` instead of `thread::yield_now()`:

- `FutureIterator` polls the future with no-op waker
- Uses `core::hint::spin_loop()` for backpressure/yielding

### ThreadedValue

Common result type across all modes:

```rust
pub enum ThreadedValue<T, E> {
    Value(Result<T, E>),
}
```

### API Surface (identical across all modes)

```rust
let threaded = ThreadedFuture::new(|| async {
    Ok::<_, Error>(some_iterator)
});

for value in threaded.execute() {
    match value {
        ThreadedValue::Value(Ok(item)) => { /* process */ }
        ThreadedValue::Value(Err(e)) => { /* handle error */ }
    }
}
```

### Detailed Code Implementation

#### Common imports and helpers (unconditional)

```rust
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

#[cfg(feature = "std")]
use std::boxed::Box;

#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::boxed::Box;

/// Common result type across all modes
#[derive(Debug)]
pub enum ThreadedValue<T, E> {
    Value(Result<T, E>),
}

/// No-op waker for polling without async runtime
fn create_noop_waker() -> Waker {
    const VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RAW_WAKER,
        |_| {},
        |_| {},
        |_| {},
    );
    const RAW_WAKER: RawWaker = RawWaker::new(core::ptr::null(), &VTABLE);
    unsafe { Waker::from_raw(RAW_WAKER) }
}

#[cfg(feature = "std")]
fn get_noop_waker() -> Waker {
    thread_local! {
        static NOOP_WAKER: Waker = create_noop_waker();
    }
    NOOP_WAKER.with(std::clone::Clone::clone)
}

#[cfg(not(feature = "std"))]
fn get_noop_waker() -> Waker {
    create_noop_waker()
}
```

#### Multi-threaded implementation (`cfg(all(feature = "std", feature = "multi"))`)

```rust
/// Iterator wrapper around Receiver for ThreadedFuture.
///
/// Uses `Receiver.into_recv_iter()` which already provides the Iterator
/// implementation with proper blocking/consumption logic.
pub struct ThreadedFutureIter<T, E> {
    iter: mpp::RecvIter<ThreadedValue<T, E>>,
}

impl<T, E> Iterator for ThreadedFutureIter<T, E> {
    type Item = ThreadedValue<T, E>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// A future executor that runs !Send operations on a background worker thread.
pub struct ThreadedFuture<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<I, E>> + 'static,
    I: Iterator<Item = Result<T, E>> + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    future_fn: F,
    queue_size: usize,
    backpressure_sleep: Option<std::time::Duration>,
    _phantom: PhantomData<(Fut, I, T, E)>,
}

impl<F, Fut, I, T, E> ThreadedFuture<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<I, E>> + 'static,
    I: Iterator<Item = Result<T, E>> + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    pub fn new(future_fn: F) -> Self {
        Self {
            future_fn,
            queue_size: 16,
            backpressure_sleep: Some(std::time::Duration::from_millis(10)),
            _phantom: PhantomData,
        }
    }

    pub fn with_queue_size(future_fn: F, queue_size: usize) -> Self {
        Self {
            future_fn,
            queue_size: queue_size.max(2),
            backpressure_sleep: Some(std::time::Duration::from_millis(10)),
            _phantom: PhantomData,
        }
    }

    pub fn execute(
        self,
    ) -> crate::valtron::GenericResult<impl Iterator<Item = ThreadedValue<T, E>>> {
        let (sender, receiver) = mpp::bounded::<ThreadedValue<T, E>>(self.queue_size);
        let backpressure_sleep = self.backpressure_sleep;

        super::unified::run_background_job(move || {
            let waker = create_noop_waker();
            let mut cx = Context::from_waker(&waker);
            let mut future = Box::pin((self.future_fn)());

            // Poll future until it produces iterator or error
            let iterator = loop {
                match future.as_mut().poll(&mut cx) {
                    Poll::Ready(Ok(iter)) => break Some(iter),
                    Poll::Ready(Err(e)) => {
                        let _ = sender.send(ThreadedValue::Value(Err(e)));
                        break None;
                    }
                    Poll::Pending => std::thread::yield_now(),
                }
            };

            // Stream iterator results through the channel
            if let Some(iter) = iterator {
                for result in iter {
                    let mut value = ThreadedValue::Value(result);
                    loop {
                        match sender.send(value) {
                            Ok(()) => break,
                            Err(SenderError::Full(v)) => {
                                value = v;
                                if let Some(sleep_dur) = backpressure_sleep {
                                    std::thread::sleep(sleep_dur);
                                } else {
                                    std::hint::spin_loop();
                                }
                            }
                            Err(SenderError::Closed(_)) => return,
                        }
                    }
                }
            }

            let _ = sender.close();
        })?;

        // Use receiver.into_recv_iter() which already implements Iterator
        Ok(ThreadedFutureIter {
            iter: receiver.into_recv_iter(),
        })
    }
}
```

#### Single-threaded std implementation (`cfg(all(feature = "std", not(feature = "multi")))`)

```rust
/// Iterator that polls a future on each `next()` call.
pub struct FutureIterator<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<I, E>>,
    I: Iterator<Item = Result<T, E>>,
{
    future: Option<F>,
    inner_iter: Option<I>,
    _phantom: PhantomData<(Fut, T, E)>,
}

impl<F, Fut, I, T, E> Iterator for FutureIterator<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<I, E>>,
    I: Iterator<Item = Result<T, E>>,
{
    type Item = ThreadedValue<T, E>;

    fn next(&mut self) -> Option<Self::Item> {
        // If we already have the inner iterator, pull from it
        if let Some(iter) = &mut self.inner_iter {
            return iter.next().map(ThreadedValue::Value);
        }

        // Otherwise, poll the future
        let mut future = match self.future.take() {
            Some(f) => Box::pin((f)()),
            None => return None,
        };

        loop {
            let waker = get_noop_waker();
            let mut cx = Context::from_waker(&waker);

            match future.as_mut().poll(&mut cx) {
                Poll::Ready(Ok(iter)) => {
                    self.inner_iter = Some(iter);
                    // Return first item from the newly acquired iterator
                    if let Some(iter) = &mut self.inner_iter {
                        return iter.next().map(ThreadedValue::Value);
                    }
                    return None;
                }
                Poll::Ready(Err(e)) => return Some(ThreadedValue::Value(Err(e))),
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }
}

/// A future executor for single-threaded std environments.
pub struct ThreadedFuture<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<I, E>>,
    I: Iterator<Item = Result<T, E>>,
{
    future_fn: F,
    _phantom: PhantomData<(Fut, I, T, E)>,
}

impl<F, Fut, I, T, E> ThreadedFuture<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<I, E>>,
    I: Iterator<Item = Result<T, E>>,
{
    pub fn new(future_fn: F) -> Self {
        Self {
            future_fn,
            _phantom: PhantomData,
        }
    }

    pub fn with_queue_size(future_fn: F, _queue_size: usize) -> Self {
        Self {
            future_fn,
            _phantom: PhantomData,
        }
    }

    pub fn execute(self) -> impl Iterator<Item = ThreadedValue<T, E>> {
        FutureIterator {
            future: Some(self.future_fn),
            inner_iter: None,
            _phantom: PhantomData,
        }
    }
}
```

#### No-std implementation (`cfg(all(not(feature = "multi"), not(feature = "std")))`)

```rust
/// Iterator that polls a future on each `next()` call (no_std version).
pub struct FutureIterator<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<I, E>>,
    I: Iterator<Item = Result<T, E>>,
{
    future: Option<F>,
    inner_iter: Option<I>,
    _phantom: PhantomData<(Fut, T, E)>,
}

impl<F, Fut, I, T, E> Iterator for FutureIterator<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<I, E>>,
    I: Iterator<Item = Result<T, E>>,
{
    type Item = ThreadedValue<T, E>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(iter) = &mut self.inner_iter {
            return iter.next().map(ThreadedValue::Value);
        }

        let mut future = match self.future.take() {
            Some(f) => Box::pin((f)()),
            None => return None,
        };

        loop {
            let waker = get_noop_waker();
            let mut cx = Context::from_waker(&waker);

            match future.as_mut().poll(&mut cx) {
                Poll::Ready(Ok(iter)) => {
                    self.inner_iter = Some(iter);
                    if let Some(iter) = &mut self.inner_iter {
                        return iter.next().map(ThreadedValue::Value);
                    }
                    return None;
                }
                Poll::Ready(Err(e)) => return Some(ThreadedValue::Value(Err(e))),
                Poll::Pending => core::hint::spin_loop(),
            }
        }
    }
}

/// A future executor for no_std environments.
pub struct ThreadedFuture<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<I, E>>,
    I: Iterator<Item = Result<T, E>>,
{
    future_fn: F,
    _phantom: PhantomData<(Fut, I, T, E)>,
}

impl<F, Fut, I, T, E> ThreadedFuture<F, Fut, I, T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<I, E>>,
    I: Iterator<Item = Result<T, E>>,
{
    pub fn new(future_fn: F) -> Self {
        Self {
            future_fn,
            _phantom: PhantomData,
        }
    }

    pub fn with_queue_size(future_fn: F, _queue_size: usize) -> Self {
        Self {
            future_fn,
            _phantom: PhantomData,
        }
    }

    pub fn execute(self) -> impl Iterator<Item = ThreadedValue<T, E>> {
        FutureIterator {
            future: Some(self.future_fn),
            inner_iter: None,
            _phantom: PhantomData,
        }
    }
}
```

### Test Implementation

#### Multi-threaded tests (`backends/foundation_core/tests/threaded_future.rs`)

```rust
//! Tests for ThreadedFuture executor (multi-threaded mode).

#![cfg(feature = "multi")]

use foundation_core::valtron::{ThreadedFuture, ThreadedValue};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn test_threaded_future_basic() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    let threaded = ThreadedFuture::new(|| async {
        Ok::<_, ()>(vec![Ok::<i32, ()>(1), Ok(2), Ok(3)].into_iter())
    });

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            _ => None,
        })
        .collect();

    assert_eq!(results, vec![1, 2, 3]);
}

#[test]
#[traced_test]
fn test_threaded_future_error() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    let threaded = ThreadedFuture::new(|| async {
        Err::<std::vec::IntoIter<Result<i32, ()>>, ()>("future failed")
    });

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<Result<i32, ()>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
            _ => None,
        })
        .collect();

    assert_eq!(results.len(), 1);
    assert!(results[0].is_err());
}

#[test]
#[traced_test]
fn test_threaded_future_empty_iterator() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    let threaded = ThreadedFuture::new(|| async {
        Ok::<_, ()>(vec![].into_iter())
    });

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<Result<i32, ()>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
            _ => None,
        })
        .collect();

    assert!(results.is_empty());
}

#[test]
#[traced_test]
fn test_threaded_future_backpressure() {
    let _guard = foundation_core::valtron::initialize_pool(42, Some(4));

    // Small queue forces backpressure: 100 items with queue size 4
    let threaded = ThreadedFuture::with_queue_size(
        || async { Ok::<_, ()>((0..100).map(Ok).collect::<Vec<_>>().into_iter()) },
        4,
    );

    let iter = threaded.execute().expect("should submit job");
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            ThreadedValue::Value(Err(e)) => panic!("Unexpected error: {e:?}"),
            _ => None,
        })
        .collect();

    assert_eq!(
        results.len(),
        100,
        "Should have received all 100 items despite backpressure (got {})",
        results.len()
    );
}
```

#### Single-threaded std tests (`backends/foundation_core/tests/threaded_future_std.rs`)

```rust
//! Tests for ThreadedFuture executor (single-threaded std mode).

#![cfg(all(feature = "std", not(feature = "multi")))]

use foundation_core::valtron::{ThreadedFuture, ThreadedValue};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn test_threaded_future_basic() {
    let threaded = ThreadedFuture::new(|| async {
        Ok::<_, ()>(vec![Ok::<i32, ()>(1), Ok(2), Ok(3)].into_iter())
    });

    let iter = threaded.execute();
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            _ => None,
        })
        .collect();

    assert_eq!(results, vec![1, 2, 3]);
}

#[test]
#[traced_test]
fn test_threaded_future_error() {
    let threaded = ThreadedFuture::new(|| async {
        Err::<std::vec::IntoIter<Result<i32, ()>>, &'static str>("future failed")
    });

    let iter = threaded.execute();
    let results: Vec<Result<i32, &'static str>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
            _ => None,
        })
        .collect();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Err("future failed"));
}

#[test]
#[traced_test]
fn test_threaded_future_empty_iterator() {
    let threaded = ThreadedFuture::new(|| async {
        Ok::<_, ()>(vec![].into_iter())
    });

    let iter = threaded.execute();
    let results: Vec<Result<i32, ()>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
            _ => None,
        })
        .collect();

    assert!(results.is_empty());
}

#[test]
#[traced_test]
fn test_threaded_future_large_iterator() {
    let threaded = ThreadedFuture::new(|| async {
        Ok::<_, ()>((0..1000).map(Ok).collect::<Vec<_>>().into_iter())
    });

    let iter = threaded.execute();
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            _ => None,
        })
        .collect();

    assert_eq!(results.len(), 1000);
    assert_eq!(results[0], 0);
    assert_eq!(results[999], 999);
}
```

#### No-std tests (`backends/foundation_core/tests/threaded_future_nostd.rs`)

```rust
//! Tests for ThreadedFuture executor (no_std mode).

#![cfg(all(not(feature = "multi"), not(feature = "std")))]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use foundation_core::valtron::{ThreadedFuture, ThreadedValue};

#[test]
fn test_threaded_future_basic() {
    let threaded = ThreadedFuture::new(|| async {
        Ok::<_, ()>(alloc::vec![Ok::<i32, ()>(1), Ok(2), Ok(3)].into_iter())
    });

    let iter = threaded.execute();
    let results: Vec<i32> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(Ok(val)) => Some(val),
            _ => None,
        })
        .collect();

    assert_eq!(results, alloc::vec![1, 2, 3]);
}

#[test]
fn test_threaded_future_error() {
    let threaded = ThreadedFuture::new(|| async {
        Err::<alloc::vec::IntoIter<Result<i32, &'static str>>, &'static str>("future failed")
    });

    let iter = threaded.execute();
    let results: Vec<Result<i32, &'static str>> = iter
        .filter_map(|v| match v {
            ThreadedValue::Value(result) => Some(result),
            _ => None,
        })
        .collect();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0], Err("future failed"));
}
```

### foundation_db Integration Tests

```rust
//! Tests for foundation_db with ThreadedFuture in different feature modes.

#[cfg(test)]
mod tests {
    use foundation_db::memory_backend::MemoryStorage;
    use foundation_core::valtron::DataValue;

    #[test]
    fn test_memory_query_with_threaded_future() {
        let storage = MemoryStorage::new();

        // Create table and insert data
        storage.execute("CREATE TABLE test (id INTEGER)", &[]).unwrap();
        storage.execute("INSERT INTO test VALUES (1), (2), (3)", &[]).unwrap();

        // Query using ThreadedFuture
        let stream = storage.query("SELECT * FROM test", &[]).unwrap();

        // Collect results
        let results: Vec<_> = stream.collect();
        assert_eq!(results.len(), 3);
    }
}
```

**Tasks:**
- [x] Gate existing `ThreadedFuture` to `cfg(all(feature = "std", feature = "multi"))`
- [x] Change `execute()` to return `impl Iterator<Item = ThreadedValue<T, E>>` (wrap receiver in iterator struct)
- [x] Add `FutureIterator` struct for `cfg(all(feature = "std", not(feature = "multi")))`
- [x] Add `FutureIterator` struct for `cfg(all(not(feature = "multi"), not(feature = "std")))`
- [x] Ensure `ThreadedValue<T, E>` is exported unconditionally
- [x] Move tests from `backends/foundation_core/tests/threaded_future.rs` to feature-gated
- [x] Add single-threaded std tests
- [x] Tests: multi-threaded mode works with background registry (5 tests passing)
- [x] Tests: single-threaded mode polls future correctly (4 tests passing)
- [x] Tests: foundation_db compiles and works with both feature configurations

---

## Feature 06: Unified run_future_iter API

**File**: `backends/foundation_core/src/valtron/executors/future_task.rs` (modify)

Consolidate the three feature-gated `run_future_iter` functions into a single unified function that uses internal `cfg` blocks to select the correct implementation, following the same pattern as `unified::send()` and `unified::execute()`:

### Unified Function Signature

```rust
/// Execute a future that produces an iterator, returning results as an Iterator.
///
/// WHY: Single unified API across all feature configurations (multi, std, no_std)
/// WHAT: Creates ThreadedIterFuture with configured queue_size and backpressure,
///       calls execute(), and returns the resulting iterator
///
/// In multi-threaded mode: Spawns background job, returns Result<Iterator, Error>
/// In single-threaded/no-std mode: Polls inline, returns Ok(Iterator)
pub fn run_future_iter<F, Fut, I, T, E>(
    future_fn: F,
    queue_size: Option<usize>,
    backpressure_sleep: Option<std::time::Duration>,
) -> crate::valtron::GenericResult<impl Iterator<Item = ThreadedValue<T, E>>>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<I, E>> + 'static,
    I: Iterator<Item = Result<T, E>> + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    #[cfg(feature = "multi")]
    {
        tracing::debug!("Executing as a multi-threaded future iterator");
        let threaded = ThreadedIterFuture::with_backpressure_sleep(
            future_fn,
            queue_size.unwrap_or(16),
            backpressure_sleep.or(Some(std::time::Duration::from_millis(10))),
        );
        threaded.execute()
    }

    #[cfg(not(feature = "multi"))]
    {
        #[cfg(feature = "std")]
        {
            tracing::debug!("Executing as a single-threaded std future iterator");
        }
        #[cfg(not(feature = "std"))]
        {
            tracing::debug!("Executing as a no_std future iterator");
        }
        // Single-threaded and no_std modes don't use queue_size or backpressure_sleep
        let _ = (queue_size, backpressure_sleep);
        Ok(ThreadedIterFuture::new(future_fn).execute())
    }
}
```

### Key Design Decisions

**1. Consistent return type across all modes**

All modes return `GenericResult<impl Iterator<Item = ThreadedValue<T, E>>>`:
- Multi-threaded: Can fail when submitting to background job registry (queue closed during shutdown)
- Single-threaded/no_std: Always returns `Ok(iterator)` since there's no failure mode

This allows callers to use identical error handling code without `#[cfg]` blocks.

**2. Internal cfg dispatch, not feature-gated functions**

Instead of three separate functions gated by `#[cfg(...)]`, use a single function with internal `cfg` blocks:

```rust
// BEFORE: Three separate functions
#[cfg(all(feature = "std", feature = "multi"))]
pub fn run_future_iter(...) { ... }

#[cfg(all(feature = "std", not(feature = "multi")))]
pub fn run_future_iter(...) { ... }

#[cfg(all(not(feature = "multi"), not(feature = "std")))]
pub fn run_future_iter(...) { ... }

// AFTER: Single unified function
pub fn run_future_iter(...) {
    #[cfg(feature = "multi")]
    { ... }

    #[cfg(not(feature = "multi"))]
    { ... }
}
```

This pattern:
- Provides a single API surface for all users
- Follows the established pattern from `unified::send()` and `unified::execute()`
- Makes code easier to discover and document

**3. Renaming ThreadedFuture to ThreadedIterFuture**

The type was renamed from `ThreadedFuture` to `ThreadedIterFuture` to better reflect its behavior:
- It doesn't return a `Future` - it returns an `impl Iterator`
- The name clarifies that it's an adapter that produces an iterator of results

### foundation_db Integration

With the unified `run_future_iter` API, foundation_db backends can use a single code path:

```rust
// BEFORE: Required cfg blocks for different return types
#[cfg(feature = "multi")]
let iter = threaded.execute().map_err(|e| StorageError::Backend(e.to_string()))?;

#[cfg(not(feature = "multi"))]
let iter = threaded.execute();

// AFTER: Single unified call
let iter = run_future_iter(
    move || async move {
        // ... async database operation ...
        Ok::<_, StorageError>(RowsIterator::new(rows))
    },
    None, // default queue size
    None, // default backpressure sleep
)
.map_err(|e| StorageError::Backend(e.to_string()))?;
```

### Test Results

| Test Suite | Mode | Tests | Status |
|------------|------|-------|--------|
| `threaded_future.rs` | multi | 5 | Passing |
| `threaded_future_std.rs` | std (no multi) | 4 | Passing |
| `foundation_db` | default | 11 | Passing |

**Tasks:**
- [x] Consolidate three feature-gated `run_future_iter` functions into one
- [x] Use internal `cfg` blocks following `unified::send()` pattern
- [x] Return `GenericResult<impl Iterator>` from all modes for consistent API
- [x] Add tracing debug logs for execution path selection
- [x] Update foundation_db backends (turso, libsql) to use unified API
- [x] Remove `#[cfg(feature = "multi")]` blocks from foundation_db code
- [x] Rename `ThreadedFuture` to `ThreadedIterFuture` throughout codebase
- [x] Rename `ThreadedFutureIter` to `FutureIterator` for clarity
- [x] Tests: All existing tests pass with unified API

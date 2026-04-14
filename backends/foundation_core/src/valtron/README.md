# Valtron
An experimental async runtime built on iterators that do not specifically rely on the async future paradigm nor follow the current `async/wait` paradigm most know for a more elaborate iterator and trait syntax that both provide a unified set of principle and control.

## Troubleshooting

### Multi-threaded Executor: Tests Hanging or Not Receiving Values

**Symptom:** Integration tests using `WebSocketClient`, `DrivenStreamIterator`, or other multi-threaded valtron-based iterators hang indefinitely or return `Stream::Ignore` immediately without processing any values.

**Cause:** The `PoolGuard` returned by `initialize_pool()` must be kept alive for the duration of test execution. Dropping it (e.g., `let _ = initialize_pool(...)`) immediately shuts down all worker threads, preventing any task execution.

In multi-threaded mode (`feature = "multi"`), tasks run on worker threads managed by the pool. When `PoolGuard` is dropped, the pool shuts down and all worker threads exit. This means queued tasks never execute, so the concurrent queue never receives any values. The iterator then yields `Stream::Ignore` or loops waiting for values that will never arrive.

**Fix:** Keep `PoolGuard` alive for the test duration:

```rust
// WRONG - guard dropped immediately, worker threads shut down
let _ = foundation_core::valtron::initialize_pool(42, None);

// CORRECT - guard kept alive for test duration
let _pool_guard: PoolGuard = foundation_core::valtron::initialize_pool(42, None);
```

**Note:** An initial fix attempt added a `thread::sleep` wait loop in `run_until_stream_has_value` for multi-threaded mode, but this was unnecessary. The real issue was the dropped `PoolGuard` - once the guard is kept alive, worker threads remain active and tasks execute normally.

**Files affected:**
- `tests/backends/foundation_core/websocket/echo_tests.rs` - Test fix
- `tests/backends/foundation_core/websocket/subprotocol_tests.rs` - Test fix
- `tests/backends/foundation_core/websocket/reconnection_tests.rs` - Test fix
- `tests/backends/foundation_core/event_source/consumer_integration_tests.rs` - Test fix
- `backends/foundation_core/src/valtron/executors/drivers.rs` - Removed unnecessary multi-threaded wait code

---

### Multi-threaded Executor: Parallel Test Interference

**Symptom:** Integration tests using `initialize_pool()` exhibit intermittent hangs or timeouts when running tests in parallel. Different tests fail on each run, with symptoms showing worker threads starting but never completing, or WebSocket connections established but messages never received.

**Root Cause:** The valtron executor uses global static registries (`REGISTRY` and `BG_REGISTRY`) to store the `ThreadRegistry` and `BackgroundJobRegistry`. When tests run in parallel:

1. Test A starts, calls `initialize_pool()` → sets `REGISTRY = Arc_A`
2. Test B starts, calls `initialize_pool()` → **overwrites** `REGISTRY = Arc_B`
3. Test A's worker threads now reference the wrong registry
4. When Test A's `PoolGuard` drops, it shuts down workers that may be serving Test B

This race condition causes intermittent failures that are non-deterministic and vary between test runs.

**Fix:** Tests that use `initialize_pool()` must be serialized using the `#[serial]` attribute with a named lock group. All tests within the same file share the same lock name:

```rust
use serial_test::serial;

// All tests in this file use the same serial lock to prevent PoolGuard interference
#[test]
#[traced_test]
#[ntest::timeout(60000)]
#[serial(websocket_pool)]  // Named lock groups tests by feature area
fn test_text_message_echo() {
    let _pool_guard: PoolGuard = foundation_core::valtron::initialize_pool(42, None);
    // ... test code
}
```

**Named lock groups by test file:**
- `echo_tests.rs` → `#[serial(websocket_pool)]`
- `subprotocol_tests.rs` → `#[serial(subprotocol_tests)]`
- `reconnection_tests.rs` → `#[serial(reconnection_tests)]`
- `http_redirect_integration.rs` → `#[serial(http_redirect)]`
- `consumer_integration_tests.rs` → `#[serial(sse_consumer_tests)]`

**Why not fix the global state?** The global registry design allows for a simple single-instance executor model. Adding per-test isolation would require significant architectural changes. The `#[serial]` attribute provides a pragmatic solution that:
- Keeps tests isolated with their own clean pool
- Allows parallel execution across different test groups (websocket vs http vs event_source)
- Maintains test independence without shared state

**Additional safeguards:**
- `ntest::timeout` attribute on all integration tests prevents indefinite hangs
- `PoolGuard::drop()` now clears global registries to prevent state leakage between sequential tests

**Files updated:**
- `backends/foundation_core/src/valtron/executors/threads.rs` - Added cleanup callback to PoolGuard
- `backends/foundation_core/src/valtron/executors/multi/mod.rs` - PoolGuard::with_cleanup() clears registries on drop
- All integration test files in `tests/backends/foundation_core/` - Added `#[serial(...)]` attributes

---

## Tasks
Tasks in Valtron are a custom iterator and in another sense can also become regular interators, they specifically provide a way to represent a asynchronous task as a series of values that can be iterated over and also provide a clear distinction when they are busy or when they are ready to give you a value.

```rust
use foundation_core::{
    valtron::{FnReady, NoSpawner, TaskIterator},
};

struct Counter(usize, usize);

impl Counter {
    pub fn new(val: usize) -> Self {
        Self(0, val)
    }
}

impl TaskIterator for Counter {
    /// Pending means how we wish to communicate that
    /// the task is not yet ready to send its next `Done`
    /// value
    type Pending = ();

    /// Done means the actual value we really care about
    /// it can be returend as many times as possible.
    type Ready = usize;

    /// Spawner provides us a way to declare a type that
    /// if returned is provided an handle to the executor
    /// to spawn sub-tasks that are related to this task.
    type Spawner = NoSpawner;

    fn next(
        &mut self,
    ) -> Option<crate::valtron::TaskStatus<Self::Done, Self::Pending, Self::Spawner>> {
        let current = self.0;
        let next = current + 1;

        if next == self.0 {
            return None;
        }

        self.0 = next;
        Some(crate::valtron::TaskStatus::Ready(next))
    }
}
```

## Executors
The crate provides two main means of driving execution:

### Mulit-Threaded Executor

Provided in the [multi](./executors/multi) module which allows you start a multi-threaded executor that utilizes as much threads as you allow it where work is distributed across these threads but one unique behaviour of the executor is that they never steal work, once a job has being allocated to a thread executor that job is forever owned by it till completion hence removing concerns on runtime cost to move work across threads and memory boundaries.

#### Example 1

```rust
use core::time;
use std::thread;

use rand::RngCore;
use foundation_core::valtron::FnReady;
use foundation_core::valtron::multi::{block_on, get_pool};

fn main() {
    let seed = rand::thread_rng().next_u64();

    let handler_kill = thread::spawn(move || {
        thread::sleep(time::Duration::from_millis(3000));
        get_pool().kill();
    });

    block_on(seed, None, |pool| {
        pool.spawn()
            .with_task(Counter::new(5))
            .with_resolver(Box::new(FnReady::new(|item, _executor| {
                // We can hook into the value returned by the task
                // for us to perform something with it or spawn some other task
                // from that value.
                println!("Received next: {}", item);
            })))
            .schedule()
            .expect("should deliver task");
    });

    handler_kill.join().expect("should finish");
}
```

#### Example 2


```rust
use core::time;
use std::thread;

use rand::RngCore;
use foundation_core::valtron::FnReady;
use foundation_core::valtron::multi::{block_on, get_pool};

fn main() {
    let seed = rand::thread_rng().next_u64();

    let handler_kill = thread::spawn(move || {
        thread::sleep(time::Duration::from_millis(3000));
        get_pool().kill();
    });

    // spawn a task and get back a iterator that returns the state 
    // this means it return immediately with Pending or Done, so it never 
    // blocks but lets you spin things or control polling of iterator.
    for status in pool.spawn().with_task(Counter::new(5)).iter() {
        // do something with the status
    }

    handler_kill.join().expect("should finish");
}
```

#### Example 3


```rust
use core::time;
use std::thread;

use rand::RngCore;
use foundation_core::valtron::FnReady;
use foundation_core::valtron::multi::{block_on, get_pool};

fn main() {
    let seed = rand::thread_rng().next_u64();

    let handler_kill = thread::spawn(move || {
        thread::sleep(time::Duration::from_millis(3000));
        get_pool().kill();
    });

    // spawn a task but block the thread till we get a Done 
    // item, so it consumes the pending and blocks till 
    // a `Done` is received.
    //
    // Its iterable until all the the None value is received that 
    // closes the iterator.
    for item in pool.spawn().with_task(Counter::new(5)).blocking_iter() {
        // do something with the item
    }

    handler_kill.join().expect("should finish");
}
```

### Single-Threaded Executor
Valtron also provides a single-threaded module that provides a more finegrained and controllable executor useful in environments like WebAssembly or embedded systems. We get great control to determine when a executor actually processes all queued up tasks to completion.

A benefit of single-threaded executor is the freedom to not be forced to think in multi-threaded contexts and require use of Send-safe smart pointers and wrappers but work with normal smart pointers like RefCell and Rc.

```rust
use std::{cell::RefCell, rc::Rc};

use rand::RngCore;
use tracing_test::traced_test;

use foundations_core::valtron::{
    single::{initialize, run_until_complete, spawn},
    FnReady, NoSpawner, TaskIterator,
};

struct Counter(usize, Rc<RefCell<Vec<usize>>>);

impl Counter {
    pub fn new(val: usize, list: Rc<RefCell<Vec<usize>>>) -> Self {
        Self(val, list)
    }
}

impl TaskIterator for Counter {
    type Pending = ();

    type Ready = usize;

    type Spawner = NoSpawner;

    fn next(
        &mut self,
    ) -> Option<crate::valtron::TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let item_size = self.1.borrow().len();

        if item_size == self.0 {
            return None;
        }

        self.1.borrow_mut().push(item_size);

        Some(crate::valtron::TaskStatus::Ready(self.1.borrow().len()))
    }
}

```

#### Example 1

```rust

fn main() {
    let seed = rand::thread_rng().next_u64();

    let shared_list = Rc::new(RefCell::new(Vec::new()));
    let counter = Counter::new(5, shared_list.clone());

    // initialize executor with a predictable seed
    // your task can get a random number generator that
    // can provide predictable random numbers every single
    // time if the seed provided is the same.
    initialize(seed);

    // spawn a task.
    spawn()
        .with_task(counter)
        .with_resolver(Box::new(FnReady::new(|item, _| {
            tracing::info!("Received next: {:?}", item)
        })))
        .schedule()
        .expect("should deliver task");

    // run tasks queued to completion
    run_until_complete();

    // check output is what we expect
    let items = shared_list.borrow().clone(); // should be: vec![0, 1, 2, 3, 4]);
}
```

#### Example 2

```rust

use foundations_core::valtron::single::task_iter;

fn main() {
    let seed = rand::thread_rng().next_u64();

    let shared_list = Rc::new(RefCell::new(Vec::new()));
    let counter = Counter::new(5, shared_list.clone());

    // initialize executor with a predictable seed
    // your task can get a random number generator that
    // can provide predictable random numbers every single
    // time if the seed provided is the same.
    initialize(seed);

    // spawn a task and get back a iterator that returns the state 
    // this means it return immediately with Pending or Done, so it never 
    // blocks but lets you spin things or control polling of iterator.
    for status in task_iter(spawn().with_task(counter)) {
        // do something with status
    }

    // check output is what we expect
    let items = shared_list.borrow().clone(); // should be: vec![0, 1, 2, 3, 4]);
}
```

#### Example 3

```rust

use foundations_core::valtron::single::block_iter;

fn main() {
    let seed = rand::thread_rng().next_u64();

    let shared_list = Rc::new(RefCell::new(Vec::new()));
    let counter = Counter::new(5, shared_list.clone());

    // initialize executor with a predictable seed
    // your task can get a random number generator that
    // can provide predictable random numbers every single
    // time if the seed provided is the same.
    initialize(seed);

    // spawn a task but block the thread till we get a Done 
    // item, so it consumes the pending and blocks till 
    // a `Done` is received.
    //
    // Its iterable until all the the None value is received that 
    // closes the iterator.
    for status in block_iter(spawn().with_task(counter)) {
        // do something with status
    }

    // check output is what we expect
    let items = shared_list.borrow().clone(); // should be: vec![0, 1, 2, 3, 4]);
}
```

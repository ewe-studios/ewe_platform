# Valtron
An experimental async runtime built on iterators that do not specifically rely on the async future paradigm nor follow the current `async/wait` paradigm most know for a more elaborate iterator and trait syntax that both provide a unified set of principle and control.

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

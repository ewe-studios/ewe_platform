use std::panic;
use std::time::Duration;
use std::{
    sync::{Arc, Mutex},
    thread,
};

use foundation_core::synca::mpp;
use foundation_core::valtron::{
    block_on, get_pool, valtron_test, FnReady, NoSpawner, NotificationItem, Stream, TaskIterator,
    TaskStatus,
};
use tracing_test::traced_test;

use foundation_core::synca::WaitGroup;
use foundation_core::valtron::multi::{PoolGuard, ThreadRegistry};

struct DCounter(usize, Arc<Mutex<Vec<usize>>>);

impl DCounter {
    pub fn new(val: usize, list: Arc<Mutex<Vec<usize>>>) -> Self {
        Self(val, list)
    }
}

impl TaskIterator for DCounter {
    type Pending = ();

    type Ready = usize;

    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let mut items = self.1.lock().unwrap();
        let item_size = items.len();

        if item_size == self.0 {
            return None;
        }

        items.push(item_size);
        let new_len = items.len();

        Some(TaskStatus::Ready(new_len))
    }
}

struct Counter(usize, Arc<Mutex<Vec<usize>>>, mpp::Sender<()>);

impl Counter {
    pub fn new(val: usize, list: Arc<Mutex<Vec<usize>>>) -> (Self, mpp::Receiver<()>) {
        let (sender, receiver) = mpp::bounded(10);
        (Counter(val, list, sender), receiver)
    }
}

impl TaskIterator for Counter {
    type Pending = ();

    type Ready = usize;

    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        tracing::debug!("Counter Task is running");

        let result = {
            let mut items = self.1.lock().unwrap();
            let item_size = items.len();

            if item_size == self.0 {
                None // signal "done" — will send after lock release
            } else {
                items.push(item_size);
                Some(TaskStatus::Ready(items.len()))
            }
        }; // lock released here

        match result {
            None => {
                tracing::debug!("Sending signal with sender");
                self.2.send(()).expect("send signal");
                None
            }
            some => some,
        }
    }
}

#[derive(Default)]
struct PanicCounter;

impl TaskIterator for PanicCounter {
    type Pending = ();

    type Ready = usize;

    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        tracing::debug!("PanicCounter Task is running");
        panic!("Bad stuff");
    }
}

// No #[test]/#[serial]: `#[valtron_test]` emits its own `#[test]`, and the
// pool lifecycle lock (initialize_pool) serializes pool users process-wide.
#[traced_test]
#[valtron_test(threads = 2)]
fn can_queue_and_complete_task_with_iterator() {
    let shared_list = Arc::new(Mutex::new(Vec::new()));
    let counter = DCounter::new(5, shared_list.clone());

    let iter = get_pool()
        .spawn()
        .with_task(counter)
        .schedule_iter(Duration::from_nanos(50))
        .expect("should deliver task");

    let complete: Vec<usize> = iter
        .map(|item| match item {
            NotificationItem::Ready(inner) => match inner {
                TaskStatus::Ready(value) => Some(value),
                _ => None,
            },
            NotificationItem::None => None,
        })
        .take_while(|t: &Option<usize>| t.is_some())
        .map(|t: Option<usize>| t.unwrap())
        .collect();

    assert_eq!(complete, vec![1, 2, 3, 4, 5]);
}

#[traced_test]
#[valtron_test(threads = 2)]
fn can_queue_use_stream_iterator_from_task_iterator() {
    let shared_list = Arc::new(Mutex::new(Vec::new()));
    let counter = DCounter::new(5, shared_list.clone());

    let iter = get_pool()
        .spawn()
        .with_task(counter)
        .stream_iter_with_config(
            Duration::from_nanos(20), // wait_cycle (used as park_duration)
            10,                       // max_turns
        )
        .expect("should deliver task");

    let complete: Vec<usize> = iter
        .map(|item| match item {
            Stream::Next(value) => Some(value),
            _ => None,
        })
        .take_while(|t| t.is_some())
        .map(|t| t.unwrap())
        .collect();

    assert_eq!(complete, vec![1, 2, 3, 4, 5]);
}

// No #[serial]: the pool lifecycle gate serializes pool users. The kill handler
// runs and joins INSIDE block_on using a cloned pool handle, so it kills the
// pool (letting block_on's waitgroup().wait() return) without ever calling
// get_pool() after the guard clears the global registry.
#[traced_test]
fn can_finish_even_when_task_panics() {
    let seed = fastrand::u64(..);

    block_on(seed, None, |pool| {
        let pool_clone = pool.clone();
        let handler_kill = thread::spawn(move || {
            tracing::debug!("Waiting for kill signal");
            thread::sleep(Duration::from_secs(1));
            tracing::debug!("Got kill signal");
            pool_clone.kill();
            tracing::debug!("Closing thread");
        });

        pool.spawn()
            .with_task(PanicCounter)
            .with_resolver(Box::new(FnReady::new(|item, _| {
                tracing::info!("Received next: {item:?}");
            })))
            .schedule()
            .expect("should deliver task");

        tracing::info!("Wait for kill thread to finish");
        handler_kill.join().expect("should finish");
    });
}

// No #[serial]: the pool lifecycle gate (initialize_pool) serializes all pool
// users process-wide. The kill handler runs and joins INSIDE block_on, using a
// cloned pool handle — it must call kill() before block_on's internal
// waitgroup().wait() can return, and must not touch get_pool() after the guard
// drops (which clears the global registry).
#[traced_test]
fn can_queue_and_complete_task() {
    let seed = fastrand::u64(..);

    let shared_list = Arc::new(Mutex::new(vec![]));
    let (counter, receiver) = Counter::new(5, shared_list.clone());

    block_on(seed, None, |pool| {
        // Kill handler: waits for the counter task to signal completion, then
        // kills the pool so block_on's waitgroup().wait() can unblock.
        let pool_clone = pool.clone();
        let handler_kill = thread::spawn(move || {
            tracing::debug!("Waiting for kill signal");
            receiver
                .recv_timeout(Duration::from_secs(2))
                .expect("receive signal");
            tracing::debug!("Got kill signal");
            pool_clone.kill();
            tracing::debug!("Closing thread");
        });

        tracing::debug!("Spawning new task into pool");
        pool.spawn()
            .with_task(counter)
            .with_resolver(Box::new(FnReady::new(|item, _| {
                tracing::info!("Received next: {item:?}");
            })))
            .schedule()
            .expect("should deliver task");
        tracing::debug!("Task spawned");

        handler_kill.join().expect("should finish");
    });

    assert_eq!(shared_list.lock().unwrap().clone(), vec![0, 1, 2, 3, 4]);
}

// ============================================================================
// Tests for WaitGroup and PoolGuard
// ============================================================================

/// Test 1: WaitGroup add/done/wait basic functionality
#[test]
fn test_waitgroup_add_done_unblocks_wait() {
    let wg = WaitGroup::new();

    wg.add(3);

    let wg_clone = wg.clone();
    let handle = thread::spawn(move || {
        wg_clone.wait();
    });

    // Give the thread time to start waiting
    thread::sleep(Duration::from_millis(50));

    // Decrement 3 times
    wg.done();
    wg.done();
    wg.done();

    // Wait should complete
    handle.join().expect("thread should complete");
}

/// Test 2: WaitGroup with zero count returns immediately
#[test]
fn test_waitgroup_zero_count_returns_immediately() {
    let wg = WaitGroup::new();
    // Don't add anything, count is 0
    wg.wait(); // Should return immediately
}

/// Test 3: WaitGroupGuard calls done() on drop
#[test]
fn test_waitgroup_guard_calls_done_on_drop() {
    let wg = WaitGroup::new();
    wg.add(1);

    {
        let _guard = wg.guard();
        // Guard is alive, count is still 1
    }
    // Guard dropped, count should be 0

    wg.wait(); // Should return immediately since count is 0
}

/// Test 4: WaitGroupGuard calls done() even during panic
#[test]
#[cfg_attr(
    cranelift_backend,
    ignore = "cranelift does not support panic unwinding"
)]
fn test_waitgroup_guard_calls_done_during_panic() {
    let wg = WaitGroup::new();
    wg.add(1);

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        let _guard = wg.guard();
        panic!("intentional panic");
    }));

    assert!(result.is_err(), "panic should have occurred");
    // Guard was dropped, so done() was called
    wg.wait(); // Should return immediately
}

/// Test 5: Multiple WaitGroupGuards
#[test]
fn test_waitgroup_multiple_guards() {
    let wg = WaitGroup::new();
    wg.add(3);

    let guard1 = wg.guard();
    let guard2 = wg.guard();
    let guard3 = wg.guard();

    // All guards alive, count is still 3

    drop(guard1); // count -> 2
    drop(guard2); // count -> 1
    drop(guard3); // count -> 0

    wg.wait(); // Should return immediately
}

/// Test 6: PoolGuard shutdown is idempotent
#[test]
fn test_poolguard_shutdown_idempotent() {
    let registry = Arc::new(ThreadRegistry::with_seed_and_threads(123, 2));
    let guard = PoolGuard::new(registry.clone());

    // First shutdown
    guard.shutdown();

    // Second shutdown - should not panic
    guard.shutdown();

    // Third shutdown - still no panic
    guard.shutdown();
}

/// Test 7: PoolGuard drop triggers shutdown
#[test]
fn test_poolguard_drop_triggers_shutdown() {
    let registry = Arc::new(ThreadRegistry::with_seed_and_threads(123, 2));
    let guard = PoolGuard::new(registry.clone());

    // Spawn a worker thread
    registry.spawn_worker().expect("should spawn worker");

    // Drop the guard - it will kill all threads
    drop(guard);

    // If we get here, shutdown completed
}

/// Test 8: WaitGroup across multiple threads
#[test]
fn test_waitgroup_multiple_threads() {
    let wg = WaitGroup::new();
    let num_threads = 5;

    wg.add(num_threads);

    let mut handles = vec![];

    for i in 0..num_threads {
        let wg_clone = wg.clone();
        let handle = thread::spawn(move || {
            // Simulate some work
            thread::sleep(Duration::from_millis(10 * (i as u64 + 1)));
            wg_clone.done();
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    wg.wait();

    // All threads should have called done()
    for handle in handles {
        handle.join().expect("all threads should complete");
    }
}

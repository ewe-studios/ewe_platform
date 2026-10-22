//! Tests for the single-threaded executor (`valtron::single`).
//!
//! `#[valtron_test]` brings the default engine up around each case; the cases
//! still call `single::initialize_pool` themselves because they drive the
//! thread-local single executor directly (which is not the default engine
//! when the `multi` feature is on).

use std::{cell::RefCell, rc::Rc};

use foundation_compact::rng::Rng;

use foundation_core::valtron::{
    single::{initialize_pool, run_until, run_until_complete, spawn},
    valtron_test, Ctx, FnReady, NotificationItem, Stream, TaskIterator, TaskStatus,
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

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let item_size = self.1.borrow().len();

        if item_size == self.0 {
            return None;
        }

        self.1.borrow_mut().push(item_size);

        Some(TaskStatus::Ready(self.1.borrow().len()))
    }
}

#[valtron_test]
fn can_queue_task_only() {
    let seed = foundation_compact::rng::rng().next_u64();

    let shared_list = Rc::new(RefCell::new(Vec::new()));
    let counter = Counter::new(5, shared_list.clone());

    initialize_pool(seed);

    spawn()
        .with_task(counter)
        .with_resolver(Box::new(FnReady::new(|item, _| {
            tracing::info!("Received next: {:?}", item)
        })))
        .as_scheduled()
        .spawn()
        .expect("should deliver task");

    assert_eq!(shared_list.borrow().len(), 0);
}

#[valtron_test]
fn can_queue_and_complete_task_with_run_until() {
    let seed = foundation_compact::rng::rng().next_u64();

    let shared_list = Rc::new(RefCell::new(Vec::new()));
    let counter = Counter::new(5, shared_list.clone());

    initialize_pool(seed);

    spawn()
        .with_task(counter)
        .with_resolver(Box::new(FnReady::new(|item, _| {
            tracing::info!("Received next: {:?}", item);
        })))
        .as_scheduled()
        .spawn()
        .expect("should deliver task");

    let handle = shared_list.clone();
    run_until(|_| {
        if handle.borrow_mut().len() >= 5 {
            return true;
        }
        false
    });

    assert_eq!(shared_list.borrow().clone(), vec![0, 1, 2, 3, 4]);
}

#[valtron_test]
fn can_queue_and_complete_task() {
    let seed = foundation_compact::rng::rng().next_u64();

    let shared_list = Rc::new(RefCell::new(Vec::new()));
    let counter = Counter::new(5, shared_list.clone());

    initialize_pool(seed);

    spawn()
        .with_task(counter)
        .with_resolver(Box::new(FnReady::new(|item, _| {
            tracing::info!("Received next: {:?}", item)
        })))
        .as_scheduled()
        .spawn()
        .expect("should deliver task");

    run_until_complete();

    assert_eq!(shared_list.borrow().clone(), vec![0, 1, 2, 3, 4]);
}

#[valtron_test]
fn can_queue_and_complete_task_with_iterator() {
    let seed = foundation_compact::rng::rng().next_u64();

    let shared_list = Rc::new(RefCell::new(Vec::new()));
    let counter = Counter::new(5, shared_list.clone());

    initialize_pool(seed);

    let iter = spawn()
        .with_task(counter)
        .as_scheduled()
        .recv(std::time::Duration::from_nanos(50))
        .expect("should deliver task");

    run_until_complete();

    let complete: Vec<usize> = iter
        .filter_map(|item| match item {
            NotificationItem::Ready(TaskStatus::Ready(value)) => Some(value),
            _ => None,
        })
        .collect();

    assert_eq!(complete, vec![1, 2, 3, 4, 5]);
    assert_eq!(shared_list.borrow().clone(), vec![0, 1, 2, 3, 4]);
}

#[valtron_test]
fn can_queue_and_complete_stream_with_iterator() {
    let seed = foundation_compact::rng::rng().next_u64();

    let shared_list = Rc::new(RefCell::new(Vec::new()));
    let counter = Counter::new(5, shared_list.clone());

    initialize_pool(seed);

    let iter = spawn()
        .with_task(counter)
        .as_scheduled()
        .stream(std::time::Duration::from_nanos(50))
        .expect("should deliver task");

    run_until_complete();

    let complete: Vec<usize> = iter
        .map(|item| match item {
            Stream::Next(value) => Some(value),
            _ => None,
        })
        .take_while(|t| t.is_some())
        .map(|t| t.unwrap())
        .collect();

    assert_eq!(complete, vec![1, 2, 3, 4, 5]);
    assert_eq!(shared_list.borrow().clone(), vec![0, 1, 2, 3, 4]);
}

/// `is_initialized` reports whether this thread's engine exists, so host resume
/// callbacks (JS timers queued by a yield) can skip `run_until` instead of
/// panicking on a thread that never created a pool. The engine is thread-local,
/// so a fresh thread starts uninitialized.
#[test]
fn is_initialized_tracks_this_threads_engine() {
    std::thread::spawn(|| {
        assert!(!foundation_core::valtron::single::is_initialized());
        initialize_pool(7);
        assert!(foundation_core::valtron::single::is_initialized());
    })
    .join()
    .expect("probe thread panicked");
}

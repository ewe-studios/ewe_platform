//! Tests for NoThreadController with SpinWaiter
//!
//! These tests verify that NoThreadController properly uses SpinWaiter
//! for iteration-based waiting in single-threaded/WASM environments.

#![cfg(not(feature = "multi"))]

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
use foundation_compact::Instant;

use foundation_core::valtron::{
    single::{initialize_pool, run_until_complete, spawn, NoThreadController},
    FnReady, NoSpawner, ProcessController, TaskIterator, TaskStatus,
};

/// Task that returns Ready a limited number of times then completes
struct FiniteTask {
    remaining: usize,
    value: usize,
}

impl TaskIterator for FiniteTask {
    type Ready = usize;
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.remaining == 0 {
            return None; // Task complete
        }
        self.remaining -= 1;
        Some(TaskStatus::Ready(self.value))
    }
}

/// Task that counts up to max then completes
struct CounterTask {
    max: usize,
    current: usize,
}

impl TaskIterator for CounterTask {
    type Ready = usize;
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.current >= self.max {
            return None; // Task complete
        }
        self.current += 1;
        Some(TaskStatus::Ready(self.current))
    }
}

#[test]
fn single_threaded_can_spawn_and_run_tasks() {
    let seed = 42u64;

    // Use Rc<RefCell<>> for single-threaded collection
    let results: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
    let results_clone = Rc::clone(&results);

    initialize_pool(seed);

    spawn()
        .with_task(FiniteTask {
            remaining: 1,
            value: 42,
        })
        .with_resolver(Box::new(FnReady::new(move |item, _| {
            if let TaskStatus::Ready(val) = item {
                results_clone.borrow_mut().push(val);
            }
        })))
        .schedule()
        .expect("should schedule");

    run_until_complete();

    assert_eq!(results.borrow().len(), 1);
    assert_eq!(results.borrow()[0], 42);
}

#[test]
fn single_threaded_spawns_multiple_tasks() {
    let seed = 42u64;

    let results: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));

    initialize_pool(seed);

    for i in 0..10 {
        let results_clone = Rc::clone(&results);
        spawn()
            .with_task(FiniteTask {
                remaining: 1,
                value: i,
            })
            .with_resolver(Box::new(FnReady::new(move |item, _| {
                if let TaskStatus::Ready(val) = item {
                    results_clone.borrow_mut().push(val);
                }
            })))
            .schedule()
            .expect("should schedule");
    }

    run_until_complete();

    // All 10 tasks should have run once
    assert_eq!(results.borrow().len(), 10);
    // Sum should be 0+1+2+...+9 = 45
    assert_eq!(results.borrow().iter().sum::<usize>(), 45);
}

#[test]
fn single_threaded_runs_counter_to_completion() {
    let seed = 42u64;

    let results: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
    let results_clone = Rc::clone(&results);

    initialize_pool(seed);

    spawn()
        .with_task(CounterTask { max: 5, current: 0 })
        .with_resolver(Box::new(FnReady::new(move |item, _| {
            if let TaskStatus::Ready(val) = item {
                results_clone.borrow_mut().push(val);
            }
        })))
        .schedule()
        .expect("should schedule");

    run_until_complete();

    // Should have 5 values: 1, 2, 3, 4, 5
    assert_eq!(results.borrow().len(), 5);
    assert_eq!(results.borrow()[..], [1, 2, 3, 4, 5]);
}

#[test]
fn nothreadcontroller_uses_spin_waiter() {
    let controller = NoThreadController::new();

    let start = Instant::now();
    controller.yield_for(Duration::from_millis(10));
    let elapsed = start.elapsed();

    // Should take some time (not return immediately like before)
    // But timing is approximate due to spin-loop nature
    assert!(elapsed >= Duration::from_micros(100));
}

#[test]
fn nothreadcontroller_is_cloneable() {
    let controller = NoThreadController::new();
    let _cloned = controller.clone();
    // Should compile and not panic
}

#[test]
fn nothreadcontroller_has_default() {
    let controller = NoThreadController::default();
    let start = Instant::now();
    controller.yield_for(Duration::from_millis(5));
    let elapsed = start.elapsed();
    assert!(elapsed >= Duration::from_micros(50));
}

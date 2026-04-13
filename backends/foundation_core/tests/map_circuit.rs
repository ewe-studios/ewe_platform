//! Tests for the `map_circuit` combinator on `StreamIterator` and `TaskIterator`.
//!
//! These tests verify that:
//! 1. `ShortCircuit::Continue` continues iteration
//! 2. `ShortCircuit::ReturnAndStop` returns the value and stops
//! 3. `ShortCircuit::Stop` stops without returning
//! 4. Combinators work correctly with boxed and mutex-wrapped iterators

use foundation_core::valtron::{
    NoAction, ShortCircuit, Stream, StreamIteratorExt, TaskIteratorExt, TaskShortCircuit,
    TaskStatus,
};
use std::sync::{Arc, Mutex};

// ============================================================================
// StreamIterator map_circuit tests
// ============================================================================

/// A simple test iterator that yields numbers 0..max
#[derive(Clone)]
struct CountingStreamIterator {
    current: usize,
    max: usize,
}

impl CountingStreamIterator {
    fn new(max: usize) -> Self {
        Self { current: 0, max }
    }
}

impl Iterator for CountingStreamIterator {
    type Item = Stream<usize, &'static str>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.max {
            return None;
        }
        let val = self.current;
        self.current += 1;
        Some(Stream::Next(val))
    }
}

#[test]
fn test_map_circuit_continue() {
    let iter = CountingStreamIterator::new(5);
    let mut circuit_iter = iter.map_circuit(|item| match item {
        Stream::Next(v) if v < 3 => ShortCircuit::Continue(Stream::Next(v * 2)),
        _ => ShortCircuit::Stop,
    });

    assert_eq!(circuit_iter.next(), Some(Stream::Next(0)));
    assert_eq!(circuit_iter.next(), Some(Stream::Next(2)));
    assert_eq!(circuit_iter.next(), Some(Stream::Next(4)));
    // Should stop at 3
    assert_eq!(circuit_iter.next(), None);
}

#[test]
fn test_map_circuit_return_and_stop() {
    let iter = CountingStreamIterator::new(5);
    let mut circuit_iter = iter.map_circuit(|item| match item {
        Stream::Next(v) if v == 3 => ShortCircuit::ReturnAndStop(Stream::Next(v)),
        Stream::Next(v) => ShortCircuit::Continue(Stream::Next(v)),
        _ => ShortCircuit::Stop,
    });

    assert_eq!(circuit_iter.next(), Some(Stream::Next(0)));
    assert_eq!(circuit_iter.next(), Some(Stream::Next(1)));
    assert_eq!(circuit_iter.next(), Some(Stream::Next(2)));
    assert_eq!(circuit_iter.next(), Some(Stream::Next(3)));
    // Should have stopped after returning 3
    assert_eq!(circuit_iter.next(), None);
}

#[test]
fn test_map_circuit_stop_silent() {
    let iter = CountingStreamIterator::new(5);
    let mut circuit_iter = iter.map_circuit(|item| match item {
        Stream::Next(2) => ShortCircuit::Stop,
        Stream::Next(v) => ShortCircuit::Continue(Stream::Next(v)),
        _ => ShortCircuit::Stop,
    });

    assert_eq!(circuit_iter.next(), Some(Stream::Next(0)));
    assert_eq!(circuit_iter.next(), Some(Stream::Next(1)));
    // Should stop silently at 2, not returning it
    assert_eq!(circuit_iter.next(), None);
}

#[test]
fn test_map_circuit_with_box() {
    let iter = Box::new(CountingStreamIterator::new(3));
    let mut circuit_iter = iter.map_circuit(|item| match item {
        Stream::Next(v) => ShortCircuit::Continue(Stream::Next(v + 10)),
        _ => ShortCircuit::Stop,
    });

    assert_eq!(circuit_iter.next(), Some(Stream::Next(10)));
    assert_eq!(circuit_iter.next(), Some(Stream::Next(11)));
    assert_eq!(circuit_iter.next(), Some(Stream::Next(12)));
    assert_eq!(circuit_iter.next(), None);
}

#[test]
fn test_map_circuit_with_mutex() {
    let iter = Arc::new(Mutex::new(CountingStreamIterator::new(3)));

    let guard = iter.lock().unwrap();
    let mut circuit_iter = guard.clone().map_circuit(|item| match item {
        Stream::Next(v) => ShortCircuit::Continue(Stream::Next(v * 10)),
        _ => ShortCircuit::Stop,
    });

    assert_eq!(circuit_iter.next(), Some(Stream::Next(0)));
    assert_eq!(circuit_iter.next(), Some(Stream::Next(10)));
    assert_eq!(circuit_iter.next(), Some(Stream::Next(20)));
    assert_eq!(circuit_iter.next(), None);
}

#[test]
fn test_map_circuit_chained_with_map_done() {
    let iter = CountingStreamIterator::new(4);
    let mut result = iter
        .map_circuit(|item| match item {
            Stream::Next(v) if v < 3 => ShortCircuit::Continue(item),
            _ => ShortCircuit::Stop,
        })
        .map_done(|v| v * 100);

    assert_eq!(result.next(), Some(Stream::Next(0)));
    assert_eq!(result.next(), Some(Stream::Next(100)));
    assert_eq!(result.next(), Some(Stream::Next(200)));
    assert_eq!(result.next(), None);
}

// ============================================================================
// TaskIterator map_circuit tests
// ============================================================================

/// A simple test task iterator
#[derive(Clone)]
struct CountingTaskIterator {
    current: usize,
    max: usize,
}

impl CountingTaskIterator {
    fn new(max: usize) -> Self {
        Self { current: 0, max }
    }
}

impl Iterator for CountingTaskIterator {
    type Item = TaskStatus<usize, &'static str, NoAction>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.max {
            return None;
        }
        let val = self.current;
        self.current += 1;
        Some(TaskStatus::Ready(val))
    }
}

#[test]
fn test_task_map_circuit_continue() {
    let iter = CountingTaskIterator::new(5);
    let mut circuit_iter = iter.map_circuit(|status| match status {
        TaskStatus::Ready(v) if v < 3 => TaskShortCircuit::Continue(TaskStatus::Ready(v * 2)),
        _ => TaskShortCircuit::Stop,
    });

    assert_eq!(circuit_iter.next(), Some(TaskStatus::Ready(0)));
    assert_eq!(circuit_iter.next(), Some(TaskStatus::Ready(2)));
    assert_eq!(circuit_iter.next(), Some(TaskStatus::Ready(4)));
    assert_eq!(circuit_iter.next(), None);
}

#[test]
fn test_task_map_circuit_return_and_stop() {
    let iter = CountingTaskIterator::new(5);
    let mut circuit_iter = iter.map_circuit(|status| match status {
        TaskStatus::Ready(v) if v == 3 => TaskShortCircuit::ReturnAndStop(TaskStatus::Ready(v)),
        TaskStatus::Ready(v) => TaskShortCircuit::Continue(TaskStatus::Ready(v)),
        _ => TaskShortCircuit::Stop,
    });

    assert_eq!(circuit_iter.next(), Some(TaskStatus::Ready(0)));
    assert_eq!(circuit_iter.next(), Some(TaskStatus::Ready(1)));
    assert_eq!(circuit_iter.next(), Some(TaskStatus::Ready(2)));
    assert_eq!(circuit_iter.next(), Some(TaskStatus::Ready(3)));
    assert_eq!(circuit_iter.next(), None);
}

#[test]
fn test_task_map_circuit_with_box() {
    let iter = Box::new(CountingTaskIterator::new(3));
    let mut circuit_iter = iter.map_circuit(|status| match status {
        TaskStatus::Ready(v) => TaskShortCircuit::Continue(TaskStatus::Ready(v + 10)),
        _ => TaskShortCircuit::Stop,
    });

    assert_eq!(circuit_iter.next(), Some(TaskStatus::Ready(10)));
    assert_eq!(circuit_iter.next(), Some(TaskStatus::Ready(11)));
    assert_eq!(circuit_iter.next(), Some(TaskStatus::Ready(12)));
    assert_eq!(circuit_iter.next(), None);
}

#[test]
fn test_task_map_circuit_chained_with_map_ready() {
    let iter = CountingTaskIterator::new(4);
    let mut result = iter
        .map_circuit(|status| match status {
            TaskStatus::Ready(v) if v < 3 => TaskShortCircuit::Continue(status),
            _ => TaskShortCircuit::Stop,
        })
        .map_ready(|v| v * 100);

    assert_eq!(result.next(), Some(TaskStatus::Ready(0)));
    assert_eq!(result.next(), Some(TaskStatus::Ready(100)));
    assert_eq!(result.next(), Some(TaskStatus::Ready(200)));
    assert_eq!(result.next(), None);
}

// ============================================================================
// Error handling pattern test
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
enum ResultValue {
    Success(String),
    Error(String),
}

/// Iterator simulating operations that might fail
struct FallibleIterator {
    items: Vec<ResultValue>,
    index: usize,
}

impl FallibleIterator {
    fn new(items: Vec<ResultValue>) -> Self {
        Self { items, index: 0 }
    }
}

impl Iterator for FallibleIterator {
    type Item = Stream<ResultValue, &'static str>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.items.len() {
            return None;
        }
        let item = self.items[self.index].clone();
        self.index += 1;
        Some(Stream::Next(item))
    }
}

#[test]
fn test_map_circuit_error_handling_pattern() {
    let items = vec![
        ResultValue::Success("first".to_string()),
        ResultValue::Success("second".to_string()),
        ResultValue::Error("something went wrong".to_string()),
        ResultValue::Success("third".to_string()), // Should not reach here
    ];

    let iter = FallibleIterator::new(items);
    let mut circuit_iter = iter.map_circuit(|item| match &item {
        Stream::Next(ResultValue::Error(err)) => {
            // Return the error and stop immediately
            ShortCircuit::ReturnAndStop(Stream::Next(ResultValue::Error(err.clone())))
        }
        Stream::Next(_) => ShortCircuit::Continue(item),
        _ => ShortCircuit::Stop,
    });

    assert_eq!(
        circuit_iter.next(),
        Some(Stream::Next(ResultValue::Success("first".to_string())))
    );
    assert_eq!(
        circuit_iter.next(),
        Some(Stream::Next(ResultValue::Success("second".to_string())))
    );
    // Should return the error and stop
    assert_eq!(
        circuit_iter.next(),
        Some(Stream::Next(ResultValue::Error(
            "something went wrong".to_string()
        )))
    );
    // Should not reach "third"
    assert_eq!(circuit_iter.next(), None);
}

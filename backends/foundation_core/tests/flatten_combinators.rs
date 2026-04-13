//! Integration tests for `flatten/flat_map` combinators on `TaskIterator` and `StreamIterator`.
//!
//! These tests validate the behavior of:
//! - `TaskIterator`: `flatten_ready()`, `flatten_pending()`, `flat_map_ready()`, `flat_map_pending()`
//! - `StreamIterator`: `flatten_next()`, `flatten_pending()`, `flat_map_next()`, `flat_map_pending()`
//!
//! Key design principles tested:
//! 1. Non-blocking semantics - returns Ignore when setting up inner iterators
//! 2. Inner iterator draining over multiple `next()` calls
//! 3. Pass-through of non-target states unchanged

use foundation_core::valtron::{
    NoAction, Stream, StreamIteratorExt, TaskIteratorExt, TaskStatus,
};
use std::time::Duration;

// ============================================================================
// Test Utilities
// ============================================================================

/// Simple `TaskIterator` for testing that yields predefined `TaskStatus` values
struct TestTaskIterator<R, P> {
    items: Vec<TaskStatus<R, P, NoAction>>,
    index: usize,
}

impl<R, P> TestTaskIterator<R, P>
where
    R: Clone,
    P: Clone,
{
    fn new(items: Vec<TaskStatus<R, P, NoAction>>) -> Self {
        Self { items, index: 0 }
    }
}

impl<R, P> Iterator for TestTaskIterator<R, P>
where
    R: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type Item = TaskStatus<R, P, NoAction>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.items.len() {
            let item = self.items[self.index].clone();
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

/// Simple `StreamIterator` for testing that yields predefined Stream values
struct TestStreamIterator<D, P> {
    items: Vec<Stream<D, P>>,
    index: usize,
}

impl<D, P> TestStreamIterator<D, P>
where
    D: Clone,
    P: Clone,
{
    fn new(items: Vec<Stream<D, P>>) -> Self {
        Self { items, index: 0 }
    }
}

impl<D, P> Iterator for TestStreamIterator<D, P>
where
    D: Clone + Send + 'static,
    P: Clone + Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.items.len() {
            let item = self.items[self.index].clone();
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

// ============================================================================
// TaskIterator - flatten_ready() Tests
// ============================================================================

/// Test: `flatten_ready()` flattens Vec<Ready> into individual Ready values
#[test]
fn test_task_flatten_ready_basic() {
    let items: Vec<TaskStatus<Vec<u32>, String, NoAction>> = vec![
        TaskStatus::Ready(vec![1u32, 2u32]),
        TaskStatus::Pending("wait".to_string()),
        TaskStatus::Ready(vec![3u32, 4u32, 5u32]),
    ];
    let task = TestTaskIterator::new(items);

    let mut flattened = task.flatten_ready();

    // First Ready(vec![1, 2]) -> Ignore (setting up inner), then 1, then 2
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(1))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(2))
    ));

    // Pending passes through unchanged
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Pending(_))
    ));

    // Second Ready(vec![3, 4, 5]) -> Ignore (setting up inner), then 3, 4, 5
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(3))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(4))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(5))
    ));

    // Exhausted
    assert_eq!(Iterator::next(&mut flattened), None);
}

/// Test: `flatten_ready()` with empty inner vectors
#[test]
fn test_task_flatten_ready_empty_inner() {
    let items: Vec<TaskStatus<Vec<u32>, String, NoAction>> = vec![
        TaskStatus::Ready(vec![1u32]),
        TaskStatus::Ready(vec![]), // Empty - should skip to next
        TaskStatus::Ready(vec![2u32]),
    ];
    let task = TestTaskIterator::new(items);

    let mut flattened = task.flatten_ready();

    // First Ready(vec![1]) -> Ignore, then 1
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(1))
    ));

    // Empty vec -> Ignore (setup), then immediately setup next
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));
    // Now setting up vec![2]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(2))
    ));

    assert_eq!(Iterator::next(&mut flattened), None);
}

/// Test: `flatten_ready()` preserves all non-Ready states
#[test]
fn test_task_flatten_ready_preserves_states() {
    let items: Vec<TaskStatus<Vec<u32>, String, NoAction>> = vec![
        TaskStatus::Ready(vec![1u32]),
        TaskStatus::Pending("p1".to_string()),
        TaskStatus::Delayed(Duration::from_millis(100)),
        TaskStatus::Init,
        TaskStatus::Ignore,
        TaskStatus::Ready(vec![2u32]),
    ];
    let task = TestTaskIterator::new(items);

    let mut flattened = task.flatten_ready();

    // First Ready -> Ignore, 1
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(1))
    ));

    // All non-Ready states pass through unchanged
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Pending(_))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Delayed(_))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Init)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));

    // Second Ready -> Ignore, 2
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(2))
    ));

    assert_eq!(Iterator::next(&mut flattened), None);
}

// ============================================================================
// TaskIterator - flatten_pending() Tests
// ============================================================================

/// Test: `flatten_pending()` flattens Vec<Pending> into individual Pending values
#[test]
fn test_task_flatten_pending_basic() {
    let items: Vec<TaskStatus<u32, Vec<String>, NoAction>> = vec![
        TaskStatus::Pending(vec!["a".to_string(), "b".to_string()]),
        TaskStatus::Ready(42u32),
        TaskStatus::Pending(vec!["c".to_string()]),
    ];
    let task = TestTaskIterator::new(items);

    let mut flattened = task.flatten_pending();

    // First Pending(vec!["a", "b"]) -> Ignore (setting up inner), then "a", then "b"
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Pending(ref s)) if s == "a"
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Pending(ref s)) if s == "b"
    ));

    // Ready passes through unchanged
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(42))
    ));

    // Second Pending(vec!["c"]) -> Ignore, then "c"
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Pending(ref s)) if s == "c"
    ));

    assert_eq!(Iterator::next(&mut flattened), None);
}

// ============================================================================
// TaskIterator - flat_map_ready() Tests
// ============================================================================

/// Test: `flat_map_ready()` transforms and flattens Ready values
#[test]
fn test_task_flat_map_ready_basic() {
    let items: Vec<TaskStatus<u32, String, NoAction>> = vec![
        TaskStatus::Ready(2u32),
        TaskStatus::Pending("wait".to_string()),
        TaskStatus::Ready(3u32),
    ];
    let task = TestTaskIterator::new(items);

    // Map each number to range [0, n), then flatten
    let mut flattened = task.flat_map_ready(|n| 0..n);

    // 2 -> [0, 1]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(0))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(1))
    ));

    // Pending passes through
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Pending(_))
    ));

    // 3 -> [0, 1, 2]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(0))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(1))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(2))
    ));

    assert_eq!(Iterator::next(&mut flattened), None);
}

// ============================================================================
// TaskIterator - flat_map_pending() Tests
// ============================================================================

/// Test: `flat_map_pending()` transforms and flattens Pending values
#[test]
fn test_task_flat_map_pending_basic() {
    let items: Vec<TaskStatus<u32, u32, NoAction>> = vec![
        TaskStatus::Pending(2u32),
        TaskStatus::Ready(99u32),
        TaskStatus::Pending(3u32),
    ];
    let task = TestTaskIterator::new(items);

    // Map each pending number to range [0, n), then flatten
    let mut flattened = task.flat_map_pending(|n| 0..n);

    // 2 -> [0, 1]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Pending(0))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Pending(1))
    ));

    // Ready passes through
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ready(99))
    ));

    // 3 -> [0, 1, 2]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Pending(0))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Pending(1))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(TaskStatus::Pending(2))
    ));

    assert_eq!(Iterator::next(&mut flattened), None);
}

// ============================================================================
// StreamIterator - flatten_next() Tests
// ============================================================================

/// Test: `flatten_next()` flattens Vec<Next> into individual Next values
#[test]
fn test_stream_flatten_next_basic() {
    let items: Vec<Stream<Vec<u32>, String>> = vec![
        Stream::Next(vec![1u32, 2u32]),
        Stream::Pending("wait".to_string()),
        Stream::Next(vec![3u32, 4u32, 5u32]),
    ];
    let stream = TestStreamIterator::new(items);

    let mut flattened = stream.flatten_next();

    // First Next(vec![1, 2]) -> Ignore (setting up inner), then 1, then 2
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(1))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(2))
    ));

    // Pending passes through unchanged
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(_))
    ));

    // Second Next(vec![3, 4, 5]) -> Ignore (setting up inner), then 3, 4, 5
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(3))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(4))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(5))
    ));

    // Exhausted
    assert_eq!(Iterator::next(&mut flattened), None);
}

/// Test: `flatten_next()` with empty inner vectors
#[test]
fn test_stream_flatten_next_empty_inner() {
    let items: Vec<Stream<Vec<u32>, String>> = vec![
        Stream::Next(vec![1u32]),
        Stream::Next(vec![]), // Empty - should skip to next
        Stream::Next(vec![2u32]),
    ];
    let stream = TestStreamIterator::new(items);

    let mut flattened = stream.flatten_next();

    // First Next(vec![1]) -> Ignore, then 1
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(1))
    ));

    // Empty vec -> Ignore (setup), then immediately setup next
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    // Now setting up vec![2]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(2))
    ));

    assert_eq!(Iterator::next(&mut flattened), None);
}

/// Test: `flatten_next()` preserves all non-Next states
#[test]
fn test_stream_flatten_next_preserves_states() {
    let items: Vec<Stream<Vec<u32>, String>> = vec![
        Stream::Next(vec![1u32]),
        Stream::Pending("p1".to_string()),
        Stream::Delayed(Duration::from_millis(100)),
        Stream::Init,
        Stream::Ignore,
        Stream::Next(vec![2u32]),
    ];
    let stream = TestStreamIterator::new(items);

    let mut flattened = stream.flatten_next();

    // First Next -> Ignore, 1
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(1))
    ));

    // All non-Next states pass through unchanged
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(_))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Delayed(_))
    ));
    assert!(matches!(Iterator::next(&mut flattened), Some(Stream::Init)));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));

    // Second Next -> Ignore, 2
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(2))
    ));

    assert_eq!(Iterator::next(&mut flattened), None);
}

// ============================================================================
// StreamIterator - flatten_pending() Tests
// ============================================================================

/// Test: `flatten_pending()` flattens Vec<Pending> into individual Pending values
#[test]
fn test_stream_flatten_pending_basic() {
    let items: Vec<Stream<u32, Vec<String>>> = vec![
        Stream::Pending(vec!["a".to_string(), "b".to_string()]),
        Stream::Next(42u32),
        Stream::Pending(vec!["c".to_string()]),
    ];
    let stream = TestStreamIterator::new(items);

    let mut flattened = stream.flatten_pending();

    // First Pending(vec!["a", "b"]) -> Ignore (setting up inner), then "a", then "b"
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(ref s)) if s == "a"
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(ref s)) if s == "b"
    ));

    // Next passes through unchanged
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(42))
    ));

    // Second Pending(vec!["c"]) -> Ignore, then "c"
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(ref s)) if s == "c"
    ));

    assert_eq!(Iterator::next(&mut flattened), None);
}

// ============================================================================
// StreamIterator - flat_map_next() Tests
// ============================================================================

/// Test: `flat_map_next()` transforms and flattens Next values
#[test]
fn test_stream_flat_map_next_basic() {
    let items: Vec<Stream<u32, String>> = vec![
        Stream::Next(2u32),
        Stream::Pending("wait".to_string()),
        Stream::Next(3u32),
    ];
    let stream = TestStreamIterator::new(items);

    // Map each number to range [0, n), then flatten
    let mut flattened = stream.flat_map_next(|n| 0..n);

    // 2 -> [0, 1]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(0))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(1))
    ));

    // Pending passes through
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(_))
    ));

    // 3 -> [0, 1, 2]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(0))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(1))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(2))
    ));

    assert_eq!(Iterator::next(&mut flattened), None);
}

// ============================================================================
// StreamIterator - flat_map_pending() Tests
// ============================================================================

/// Test: `flat_map_pending()` transforms and flattens Pending values
#[test]
fn test_stream_flat_map_pending_basic() {
    let items: Vec<Stream<u32, u32>> = vec![
        Stream::Pending(2u32),
        Stream::Next(99u32),
        Stream::Pending(3u32),
    ];
    let stream = TestStreamIterator::new(items);

    // Map each pending number to range [0, n), then flatten
    let mut flattened = stream.flat_map_pending(|n| 0..n);

    // 2 -> [0, 1]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(0))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(1))
    ));

    // Next passes through
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Next(99))
    ));

    // 3 -> [0, 1, 2]
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Ignore)
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(0))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(1))
    ));
    assert!(matches!(
        Iterator::next(&mut flattened),
        Some(Stream::Pending(2))
    ));

    assert_eq!(Iterator::next(&mut flattened), None);
}

// ============================================================================
// Edge Cases and Comprehensive Tests
// ============================================================================

/// Test: All states exhausted correctly for `TaskIterator`
#[test]
fn test_task_all_combinators_exhaust() {
    // flatten_ready
    let items: Vec<TaskStatus<Vec<u32>, String, NoAction>> =
        vec![TaskStatus::Ready(vec![1u32, 2u32])];
    let mut iter = TestTaskIterator::new(items).flatten_ready();
    assert!(matches!(iter.next(), Some(TaskStatus::Ignore)));
    assert!(matches!(iter.next(), Some(TaskStatus::Ready(1))));
    assert!(matches!(iter.next(), Some(TaskStatus::Ready(2))));
    assert_eq!(iter.next(), None);

    // flatten_pending
    let items: Vec<TaskStatus<u32, Vec<u32>, NoAction>> =
        vec![TaskStatus::Pending(vec![1u32, 2u32])];
    let mut iter = TestTaskIterator::new(items).flatten_pending();
    assert!(matches!(iter.next(), Some(TaskStatus::Ignore)));
    assert!(matches!(iter.next(), Some(TaskStatus::Pending(1))));
    assert!(matches!(iter.next(), Some(TaskStatus::Pending(2))));
    assert_eq!(iter.next(), None);

    // flat_map_ready
    let items: Vec<TaskStatus<u32, String, NoAction>> = vec![TaskStatus::Ready(2u32)];
    let mut iter = TestTaskIterator::new(items).flat_map_ready(|n| 0..n);
    assert!(matches!(iter.next(), Some(TaskStatus::Ignore)));
    assert!(matches!(iter.next(), Some(TaskStatus::Ready(0))));
    assert!(matches!(iter.next(), Some(TaskStatus::Ready(1))));
    assert_eq!(iter.next(), None);

    // flat_map_pending
    let items: Vec<TaskStatus<u32, u32, NoAction>> = vec![TaskStatus::Pending(2u32)];
    let mut iter = TestTaskIterator::new(items).flat_map_pending(|n| 0..n);
    assert!(matches!(iter.next(), Some(TaskStatus::Ignore)));
    assert!(matches!(iter.next(), Some(TaskStatus::Pending(0))));
    assert!(matches!(iter.next(), Some(TaskStatus::Pending(1))));
    assert_eq!(iter.next(), None);
}

/// Test: All states exhausted correctly for `StreamIterator`
#[test]
fn test_stream_all_combinators_exhaust() {
    // flatten_next
    let items: Vec<Stream<Vec<u32>, String>> = vec![Stream::Next(vec![1u32, 2u32])];
    let mut iter = TestStreamIterator::new(items).flatten_next();
    assert!(matches!(iter.next(), Some(Stream::Ignore)));
    assert!(matches!(iter.next(), Some(Stream::Next(1))));
    assert!(matches!(iter.next(), Some(Stream::Next(2))));
    assert_eq!(iter.next(), None);

    // flatten_pending
    let items: Vec<Stream<u32, Vec<u32>>> = vec![Stream::Pending(vec![1u32, 2u32])];
    let mut iter = TestStreamIterator::new(items).flatten_pending();
    assert!(matches!(iter.next(), Some(Stream::Ignore)));
    assert!(matches!(iter.next(), Some(Stream::Pending(1))));
    assert!(matches!(iter.next(), Some(Stream::Pending(2))));
    assert_eq!(iter.next(), None);

    // flat_map_next
    let items: Vec<Stream<u32, String>> = vec![Stream::Next(2u32)];
    let mut iter = TestStreamIterator::new(items).flat_map_next(|n| 0..n);
    assert!(matches!(iter.next(), Some(Stream::Ignore)));
    assert!(matches!(iter.next(), Some(Stream::Next(0))));
    assert!(matches!(iter.next(), Some(Stream::Next(1))));
    assert_eq!(iter.next(), None);

    // flat_map_pending
    let items: Vec<Stream<u32, u32>> = vec![Stream::Pending(2u32)];
    let mut iter = TestStreamIterator::new(items).flat_map_pending(|n| 0..n);
    assert!(matches!(iter.next(), Some(Stream::Ignore)));
    assert!(matches!(iter.next(), Some(Stream::Pending(0))));
    assert!(matches!(iter.next(), Some(Stream::Pending(1))));
    assert_eq!(iter.next(), None);
}

/// Test: Empty outer iterator produces no output
#[test]
fn test_flatten_empty_outer() {
    // TaskIterator
    let items: Vec<TaskStatus<Vec<u32>, String, NoAction>> = vec![];
    let mut iter = TestTaskIterator::new(items).flatten_ready();
    assert_eq!(iter.next(), None);

    // StreamIterator
    let items: Vec<Stream<Vec<u32>, String>> = vec![];
    let mut iter = TestStreamIterator::new(items).flatten_next();
    assert_eq!(iter.next(), None);
}

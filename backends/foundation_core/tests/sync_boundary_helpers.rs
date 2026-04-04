//! WHY: Tests for the sync boundary helpers (`collect_result`, `collect_one`,
//! `sync_one`, `sync_collect_one`, `sync_all`) in valtron's unified executor module.
//!
//! WHAT: Validates that these helpers correctly drain streams, block until results
//! are available, and handle edge cases (empty streams, multiple values, pending states).
//!
//! HOW: Uses `CounterTask` (a simple `TaskIterator` that yields configurable Pending
//! states then a single Ready value) and direct `Stream` iterators to test each helper.

use foundation_core::valtron::Stream;
use foundation_core::valtron::{
    collect_one, collect_result, initialize_pool, sync_all, sync_collect_one, sync_one, NoAction,
    TaskIterator, TaskStatus,
};

// ---------------------------------------------------------------------------
// Test task: produces N pending states then a single Ready value
// ---------------------------------------------------------------------------

/// WHY: Provides a controllable TaskIterator for testing executor helpers.
///
/// WHAT: Yields `pending_count` Pending states, then one Ready with `ready_value`.
///
/// HOW: Simple counter-based state machine.
struct CounterTask {
    pending_count: u32,
    current: u32,
    ready_value: u32,
}

impl CounterTask {
    fn new(pending_count: u32, ready_value: u32) -> Self {
        Self {
            pending_count,
            current: 0,
            ready_value,
        }
    }
}

impl Iterator for CounterTask {
    type Item = TaskStatus<u32, u32, NoAction>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.pending_count {
            self.current += 1;
            Some(TaskStatus::Pending(self.current))
        } else if self.current == self.pending_count {
            self.current += 1;
            Some(TaskStatus::Ready(self.ready_value))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Test task: produces multiple Ready values
// ---------------------------------------------------------------------------

/// WHY: Tests that `collect_result` drains ALL `Next` values, not just the first.
///
/// WHAT: Yields multiple Ready values in sequence.
///
/// HOW: Iterates through a Vec of values, yielding each as Ready.
struct MultiValueTask {
    values: Vec<u32>,
    index: usize,
}

impl MultiValueTask {
    fn new(values: Vec<u32>) -> Self {
        Self { values, index: 0 }
    }
}

impl Iterator for MultiValueTask {
    type Item = TaskStatus<u32, (), NoAction>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.values.len() {
            let val = self.values[self.index];
            self.index += 1;
            Some(TaskStatus::Ready(val))
        } else {
            None
        }
    }
}

// ============================================================================
// collect_result tests
// ============================================================================

/// collect_result on a stream with a single Next value returns a Vec with one item.
#[test]
fn test_collect_result_single_next() {
    let stream: Vec<Stream<u32, ()>> = vec![Stream::Next(42)];
    let results = collect_result(stream.into_iter());
    assert_eq!(results, vec![42]);
}

/// collect_result on a stream with multiple Next values collects all of them.
#[test]
fn test_collect_result_multiple_next() {
    let stream: Vec<Stream<u32, ()>> = vec![Stream::Next(1), Stream::Next(2), Stream::Next(3)];
    let results = collect_result(stream.into_iter());
    assert_eq!(results, vec![1, 2, 3]);
}

/// collect_result skips Pending, Delayed, Init, and Ignore items.
#[test]
fn test_collect_result_skips_non_next() {
    let stream: Vec<Stream<u32, String>> = vec![
        Stream::Init,
        Stream::Pending("waiting".to_string()),
        Stream::Ignore,
        Stream::Delayed(std::time::Duration::from_millis(10)),
        Stream::Next(99),
        Stream::Pending("more waiting".to_string()),
        Stream::Next(100),
    ];
    let results = collect_result(stream.into_iter());
    assert_eq!(results, vec![99, 100]);
}

/// collect_result on an empty stream returns an empty Vec.
#[test]
fn test_collect_result_empty_stream() {
    let stream: Vec<Stream<u32, ()>> = vec![];
    let results = collect_result(stream.into_iter());
    assert!(results.is_empty());
}

/// collect_result on a stream with no Next values returns an empty Vec.
#[test]
fn test_collect_result_no_next_values() {
    let stream: Vec<Stream<u32, &str>> =
        vec![Stream::Init, Stream::Pending("waiting"), Stream::Ignore];
    let results = collect_result(stream.into_iter());
    assert!(results.is_empty());
}

// ============================================================================
// sync_one tests
// ============================================================================

/// sync_one executes a task that produces a single value after pending states.
#[test]
fn test_sync_one_single_value_task() {
    let _guard = initialize_pool(42, None);

    let task = CounterTask::new(3, 77);
    let results = sync_one(task).expect("sync_one should succeed");
    assert_eq!(results, vec![77]);
}

/// sync_one executes a task that produces a value with no pending states.
#[test]
fn test_sync_one_immediate_ready() {
    let _guard = initialize_pool(42, None);

    let task = CounterTask::new(0, 55);
    let results = sync_one(task).expect("sync_one should succeed");
    assert_eq!(results, vec![55]);
}

/// sync_one collects multiple Ready values from a multi-value task.
#[test]
fn test_sync_one_multi_value_task() {
    let _guard = initialize_pool(42, None);

    let task = MultiValueTask::new(vec![10, 20, 30]);
    let results = sync_one(task).expect("sync_one should succeed");
    assert_eq!(results, vec![10, 20, 30]);
}

// ============================================================================
// sync_all tests
// ============================================================================

/// sync_all executes multiple tasks in parallel and collects all results.
#[test]
fn test_sync_all_multiple_tasks() {
    let _guard = initialize_pool(42, None);

    let tasks = vec![
        CounterTask::new(1, 10),
        CounterTask::new(2, 20),
        CounterTask::new(0, 30),
    ];
    let results = sync_all(tasks).expect("sync_all should succeed");
    assert_eq!(results.len(), 3);
    assert!(results.contains(&10));
    assert!(results.contains(&20));
    assert!(results.contains(&30));
}

/// sync_all with a single task behaves like sync_one.
#[test]
fn test_sync_all_single_task() {
    let _guard = initialize_pool(42, None);

    let tasks = vec![CounterTask::new(1, 42)];
    let results = sync_all(tasks).expect("sync_all should succeed");
    assert_eq!(results, vec![42]);
}

/// sync_all with an empty task list returns an error (no results produced).
#[test]
fn test_sync_all_empty_tasks() {
    let _guard = initialize_pool(42, None);

    let tasks: Vec<CounterTask> = vec![];
    let result = sync_all(tasks);
    assert!(result.is_err(), "sync_all with empty tasks should error");
}

/// sync_all collects results from tasks with varying pending counts.
#[test]
fn test_sync_all_varying_pending_counts() {
    let _guard = initialize_pool(42, None);

    let tasks = vec![
        CounterTask::new(0, 1),  // immediate
        CounterTask::new(5, 2),  // 5 pending states
        CounterTask::new(10, 3), // 10 pending states
        CounterTask::new(1, 4),  // 1 pending state
    ];
    let results = sync_all(tasks).expect("sync_all should succeed");
    assert_eq!(results.len(), 4);
    assert!(results.contains(&1));
    assert!(results.contains(&2));
    assert!(results.contains(&3));
    assert!(results.contains(&4));
}

// ============================================================================
// collect_one tests
// ============================================================================

/// collect_one on a stream with a single Next value returns Some(value).
#[test]
fn test_collect_one_single_next() {
    let stream: Vec<Stream<u32, ()>> = vec![Stream::Next(42)];
    let result = collect_one(stream.into_iter());
    assert_eq!(result, Some(42));
}

/// collect_one returns the first Next, ignoring subsequent ones.
#[test]
fn test_collect_one_returns_first_next_only() {
    let stream: Vec<Stream<u32, ()>> = vec![Stream::Next(1), Stream::Next(2), Stream::Next(3)];
    let result = collect_one(stream.into_iter());
    assert_eq!(result, Some(1));
}

/// collect_one skips Pending/Init/Ignore/Delayed before finding Next.
#[test]
fn test_collect_one_skips_non_next() {
    let stream: Vec<Stream<u32, String>> = vec![
        Stream::Init,
        Stream::Pending("waiting".to_string()),
        Stream::Ignore,
        Stream::Delayed(std::time::Duration::from_millis(5)),
        Stream::Next(99),
    ];
    let result = collect_one(stream.into_iter());
    assert_eq!(result, Some(99));
}

/// collect_one on an empty stream returns None.
#[test]
fn test_collect_one_empty_stream() {
    let stream: Vec<Stream<u32, ()>> = vec![];
    let result = collect_one(stream.into_iter());
    assert_eq!(result, None);
}

/// collect_one on a stream with no Next values returns None.
#[test]
fn test_collect_one_no_next_values() {
    let stream: Vec<Stream<u32, &str>> =
        vec![Stream::Init, Stream::Pending("waiting"), Stream::Ignore];
    let result = collect_one(stream.into_iter());
    assert_eq!(result, None);
}

// ============================================================================
// sync_collect_one tests
// ============================================================================

/// sync_collect_one executes a task and returns the single result directly.
#[test]
fn test_sync_collect_one_single_value() {
    let _guard = initialize_pool(42, None);

    let task = CounterTask::new(3, 77);
    let result = sync_collect_one(task).expect("sync_collect_one should succeed");
    assert_eq!(result, 77);
}

/// sync_collect_one with immediate ready (no pending states).
#[test]
fn test_sync_collect_one_immediate_ready() {
    let _guard = initialize_pool(42, None);

    let task = CounterTask::new(0, 55);
    let result = sync_collect_one(task).expect("sync_collect_one should succeed");
    assert_eq!(result, 55);
}

/// sync_collect_one with a multi-value task returns the first value.
#[test]
fn test_sync_collect_one_multi_value_returns_first() {
    let _guard = initialize_pool(42, None);

    let task = MultiValueTask::new(vec![10, 20, 30]);
    let result = sync_collect_one(task).expect("sync_collect_one should succeed");
    assert_eq!(result, 10);
}

//! Integration tests for Feature 04: Notification-Based Waiting
//!
//! These tests verify that the CondVar-based notification mechanism works correctly
//! in single-threaded mode. CRITICAL: In single-threaded mode, wait timeouts must be
//! very short (microseconds, not milliseconds) because if we block for too long
//! waiting for a notification, nothing else runs to produce the notification -
//! causing a deadlock.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use foundation_core::valtron::{
    InlineSendAction, InlineSendActionBehaviour, LocalThreadExecutor, NotifyQueue,
    NotifyQueueStreamIterator, NotifyRecvIter, NotifyRecvIterator, PriorityOrder,
};
use foundation_core::valtron::{
    BoxedSendExecutionIterator, ExecutionAction, NoSpawner, TaskIterator, TaskStatus,
    WrapTask,
};
use foundation_core::{
    retries::ExponentialBackoffDecider,
    synca::{IdleMan, SleepyMan},
    valtron::{ProcessController, ProgressIndicator, Stream},
};
use concurrent_queue::ConcurrentQueue;
use tracing_test::traced_test;

/// No-op process controller for tests
#[derive(Default, Clone)]
struct NoYielder;

impl ProcessController for NoYielder {
    fn yield_for(&self, _duration: Duration) {}
}

// ============================================================================
// NotifyQueue Tests
// ============================================================================

/// Test that NotifyQueue works correctly with very short timeouts in single-threaded mode.
/// This simulates the scenario where the executor must not block for long.
#[test]
fn test_notify_queue_short_timeout_single_threaded() {
    let queue: NotifyQueue<i32> = NotifyQueue::unbounded();

    // In single-threaded mode, timeout must be very short (microseconds)
    // Longer timeouts would block the only thread, preventing progress
    let very_short_timeout = Duration::from_micros(10);

    // Queue starts empty, should return None immediately-ish
    let start = Instant::now();
    let result = queue.wait_for_item(very_short_timeout);
    let elapsed = start.elapsed();

    // Should return None since queue is empty
    assert!(result.is_none());
    // Should complete quickly (not block for a long time)
    assert!(elapsed < Duration::from_millis(5), "Wait took too long: {:?}", elapsed);
}

/// Test that NotifyQueue correctly receives items with notification.
#[test]
fn test_notify_queue_receives_item_with_notification() {
    let queue: NotifyQueue<i32> = NotifyQueue::unbounded();

    // Push an item
    queue.push(42).expect("push should succeed");

    // Should receive immediately with short timeout
    let result = queue.wait_for_item(Duration::from_micros(100));
    assert_eq!(result, Some(42));
}

/// Test NotifyQueue handles multiple items with short timeouts.
/// This simulates a stream of values being processed.
#[test]
fn test_notify_queue_multiple_items_short_timeout() {
    let queue: NotifyQueue<i32> = NotifyQueue::unbounded();
    let short_timeout = Duration::from_micros(50);

    // Push multiple items
    for i in 0..5 {
        queue.push(i).expect("push should succeed");
    }

    // Receive all items with short timeouts
    let mut received = Vec::new();
    for _ in 0..5 {
        if let Some(item) = queue.wait_for_item(short_timeout) {
            received.push(item);
        }
    }

    assert_eq!(received, vec![0, 1, 2, 3, 4]);
}

/// Test that demonstrates the timeout race condition handling.
/// Item added between initial check and lock acquisition.
#[test]
fn test_notify_queue_race_condition_handling() {
    let queue: NotifyQueue<i32> = NotifyQueue::unbounded();

    // Push item before wait
    queue.push(100).unwrap();

    // Wait with timeout should return immediately with the item
    // This tests the race condition where item is added before lock
    let start = Instant::now();
    let result = queue.wait_for_item(Duration::from_millis(100));
    let elapsed = start.elapsed();

    assert_eq!(result, Some(100));
    // Should be very fast since item was already there
    assert!(elapsed < Duration::from_millis(10), "Race handling took too long");
}

/// Test bounded NotifyQueue capacity handling in single-threaded mode.
#[test]
fn test_notify_queue_bounded_capacity() {
    let queue: NotifyQueue<i32> = NotifyQueue::bounded(2);

    // Fill to capacity
    queue.push(1).unwrap();
    queue.push(2).unwrap();

    // Third push should fail (bounded)
    let result = queue.push(3);
    assert!(result.is_err(), "Push should fail when bounded queue is full");

    // Pop one to make space
    assert_eq!(queue.pop(), Ok(1));

    // Now push should succeed
    queue.push(3).unwrap();
}

/// Test that very short timeouts (1 microsecond) still work correctly.
/// This is the extreme case for single-threaded mode.
#[test]
fn test_notify_queue_extremely_short_timeout() {
    let queue: NotifyQueue<i32> = NotifyQueue::unbounded();

    // 1 microsecond timeout - extremely short
    let ultra_short = Duration::from_micros(1);

    // Empty queue should return None even with ultra-short timeout
    let start = Instant::now();
    let result = queue.wait_for_item(ultra_short);
    let elapsed = start.elapsed();

    assert!(result.is_none());
    // Even with 1us timeout, actual elapsed might be higher due to scheduling
    // but should still be reasonably fast
    assert!(elapsed < Duration::from_millis(10), "Ultra-short timeout took too long");
}

// ============================================================================
// NotifyRecvIter Tests
// ============================================================================

/// Test NotifyRecvIter with very short timeout in single-threaded context.
#[test]
fn test_notify_recv_iterator_short_timeout() {
    let queue: Arc<NotifyQueue<i32>> = Arc::new(NotifyQueue::unbounded());
    let recv_iter = NotifyRecvIter::new(queue.clone());

    // Use microsecond timeout (critical for single-threaded)
    let timeout = Duration::from_micros(10);

    // Should return None on empty queue
    let start = Instant::now();
    let result = recv_iter.block_recv(timeout);
    let elapsed = start.elapsed();

    assert!(result.is_none());
    assert!(elapsed < Duration::from_millis(5), "Block recv took too long: {:?}", elapsed);
}

/// Test NotifyRecvIterator correctly iterates with notification.
#[test]
fn test_notify_recv_iterator_iterates_with_notification() {
    let queue: Arc<NotifyQueue<i32>> = Arc::new(NotifyQueue::unbounded());
    let iterator = NotifyRecvIterator::from_notify_queue(
        queue.clone(),
        Duration::from_micros(100),
    );

    // Push items
    queue.push(1).unwrap();
    queue.push(2).unwrap();
    queue.push(3).unwrap();

    // Collect items through iterator
    let items: Vec<_> = iterator.take(3).collect();
    assert_eq!(items, vec![1, 2, 3]);
}

// ============================================================================
// NotifyQueueStreamIterator Tests
// ============================================================================

/// Test NotifyQueueStreamIterator with short timeouts in single-threaded mode.
#[test]
fn test_notify_queue_stream_iterator_short_timeout() {
    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());

    // Create iterator with very short timeout (microseconds)
    let mut iterator = NotifyQueueStreamIterator::new(
        queue.clone(),
        3, // max_turns
        Duration::from_micros(10), // park_duration as timeout
    );

    // Empty queue should yield Ignore quickly
    let start = Instant::now();
    let result = iterator.next();
    let elapsed = start.elapsed();

    assert_eq!(result, Some(Stream::Ignore));
    // Should complete very quickly with short timeout
    assert!(elapsed < Duration::from_millis(2), "Iterator took too long: {:?}", elapsed);
}

/// Test NotifyQueueStreamIterator receives values via notification.
#[test]
fn test_notify_queue_stream_iterator_receives_values() {
    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());

    let mut iterator = NotifyQueueStreamIterator::new(
        queue.clone(),
        5,
        Duration::from_micros(50),
    );

    // Push values
    queue.push(Stream::Next(1)).unwrap();
    queue.push(Stream::Next(2)).unwrap();
    queue.push(Stream::Next(3)).unwrap();

    // Should receive all values
    assert_eq!(iterator.next(), Some(Stream::Next(1)));
    assert_eq!(iterator.next(), Some(Stream::Next(2)));
    assert_eq!(iterator.next(), Some(Stream::Next(3)));
}

// ============================================================================
// Executor Integration Tests
// ============================================================================

/// Single-threaded executor test with notification-based tasks.
/// Verifies that tasks using NotifyQueue-based receivers don't block progress.
#[test]
#[traced_test]
fn test_single_threaded_executor_with_notification_tasks() {
    let global: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
        Arc::new(ConcurrentQueue::bounded(10));

    let results: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(Vec::new()));

    let seed = rand::random();
    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        Duration::from_secs(10),
        None,
        None,
    );

    // Create a task that produces values with short wait timeout
    // The timeout is critical - must be short for single-threaded
    let task = WrapTask::new(vec![10, 20, 30].into_iter());

    let (mut inline_action, receiver) = InlineSendAction::boxed_mapper(
        InlineSendActionBehaviour::Lift,
        Vec::new(),
        task,
        // CRITICAL: Very short timeout for single-threaded mode
        Duration::from_micros(50),
    );

    // Wrap receiver in Arc<Mutex<>> to allow access from closure
    let receiver = Arc::new(Mutex::new(receiver));

    // Schedule the task
    inline_action
        .apply(None, executor.boxed_engine())
        .expect("should schedule");

    // Run until task completes
    executor.run_until({
        let receiver = Arc::clone(&receiver);
        let results = Arc::clone(&results);
        move |state| {
            // Collect available results with very short timeout
            while let Some(status) = receiver.lock().unwrap().next() {
                if let TaskStatus::Ready(val) = status {
                    results.lock().unwrap().push(val);
                }
            }
            // Continue until we have all 3 values
            results.lock().unwrap().len() >= 3 || state == ProgressIndicator::NoWork
        }
    });

    // Collect any remaining results
    while let Some(status) = receiver.lock().unwrap().next() {
        if let TaskStatus::Ready(val) = status {
            results.lock().unwrap().push(val);
        }
    }

    assert_eq!(*results.lock().unwrap(), vec![10, 20, 30]);
}

/// Test that single-threaded executor handles tasks that pend/sleep
/// without blocking indefinitely. Uses very short wait timeouts.
#[test]
#[traced_test]
fn test_single_threaded_executor_pends_without_blocking() {
    let global: Arc<ConcurrentQueue<BoxedSendExecutionIterator>> =
        Arc::new(ConcurrentQueue::bounded(10));

    let counter: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    let seed = rand::random();
    let executor = LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        Duration::from_secs(10),
        None,
        None,
    );

    // Task that counts and returns Delayed state
    struct CountingTask(Arc<Mutex<usize>>);
    impl TaskIterator for CountingTask {
        type Ready = ();
        type Spawner = NoSpawner;
        type Pending = Duration;

        fn next_status(
            &mut self,
        ) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
            let mut count = self.0.lock().unwrap();
            *count += 1;
            if *count >= 5 {
                return None; // Done
            }
            // Return Delayed to test pending state handling
            Some(TaskStatus::Delayed(Duration::from_micros(1)))
        }
    }

    let counter_clone = Arc::clone(&counter);
    let task = CountingTask(counter_clone);

    let (mut inline_action, receiver) = InlineSendAction::boxed_mapper(
        InlineSendActionBehaviour::Lift,
        Vec::new(),
        task,
        // Very short timeout for single-threaded
        Duration::from_micros(10),
    );

    // Wrap receiver in Arc<Mutex<>> for closure access
    let receiver = Arc::new(Mutex::new(receiver));

    inline_action
        .apply(None, executor.boxed_engine())
        .expect("should schedule");

    // Run until no more work
    let start = Instant::now();
    executor.run_until({
        let receiver = Arc::clone(&receiver);
        move |_| {
            // Drain receiver with short timeout
            while let Some(_status) = receiver.lock().unwrap().next() {}
            false // Keep running until timeout
        }
    });
    let elapsed = start.elapsed();

    // Should complete reasonably quickly, not hang
    assert!(
        elapsed < Duration::from_secs(5),
        "Test took too long, possible deadlock"
    );
    assert_eq!(*counter.lock().unwrap(), 5);
}

// ============================================================================
// Producer-Consumer Pattern Tests
// ============================================================================

/// Comprehensive single-threaded test: producer and consumer interleaved.
/// This simulates the real executor scenario where tasks produce and consume
/// values in the same thread.
#[test]
#[traced_test]
fn test_single_threaded_producer_consumer_interleaved() {
    let queue: Arc<NotifyQueue<usize>> = Arc::new(NotifyQueue::unbounded());
    let short_timeout = Duration::from_micros(25);

    let mut produced = 0;
    let mut consumed = 0;

    // Interleaved producer/consumer pattern
    for i in 0..10 {
        // Produce
        queue.push(i).unwrap();
        produced += 1;

        // Consume with short timeout
        if let Some(val) = queue.wait_for_item(short_timeout) {
            assert_eq!(val, i);
            consumed += 1;
        }
    }

    assert_eq!(produced, 10);
    assert_eq!(consumed, 10);
}

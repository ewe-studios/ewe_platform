//! Integration tests for Feature 04: Notification-Based Waiting
//!
//! These tests verify that the CondVar-based notification mechanism works correctly
//! in single-threaded mode. CRITICAL: In single-threaded mode, wait timeouts must be
//! very short (microseconds, not milliseconds) because if we block for too long
//! waiting for a notification, nothing else runs to produce the notification -
//! causing a deadlock.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::{
    SharedTaskQueue, ExecutionAction, NoSpawner, TaskIterator, TaskStatus, WrapTask,
};
use foundation_core::valtron::{
    InlineSendAction, InlineSendActionBehaviour, LocalThreadExecutor, NotifyQueue,
    NotifyQueueStreamIterator, NotifyRecvIter, NotifyRecvIterator, PriorityOrder,
    NotificationItem,
};
use foundation_core::{
    retries::ExponentialBackoffDecider,
    synca::{IdleMan, SleepyMan},
    valtron::{ProcessController, ProgressIndicator, Stream},
};
use tracing_test::traced_test;

/// No-op process controller for tests
#[derive(Default, Clone)]
struct NoYielder;

impl ProcessController for NoYielder {
    type YieldSignal = ();

    fn yield_for(&self, _duration: Duration) {}
}

// ============================================================================
// NotifyQueue Tests
// ============================================================================

/// Test that NotifyQueue blocks until item is available in single-threaded mode.
/// Uses a producer thread to push items while consumer waits.
#[test]
#[traced_test]
fn test_notify_queue_blocks_until_item_available() {
    let queue: Arc<NotifyQueue<i32>> = Arc::new(NotifyQueue::unbounded());
    let queue_clone = queue.clone();

    // Producer thread pushes item after short delay
    let producer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(10));
        queue_clone.push(42).expect("push should succeed");
    });

    // Consumer waits for item (blocks until available via retry loop)
    let start = Instant::now();
    let mut result = None;
    loop {
        match queue.wait_for_item(Duration::from_micros(100)) {
            Some(NotificationItem::Ready(val)) => { result = Some(val); break; }
            Some(NotificationItem::None) => continue, // Yield signal, retry
            None => break, // Queue closed
        }
    }
    let elapsed = start.elapsed();

    producer.join().expect("producer should complete");

    assert_eq!(result, Some(42));
    // Should have waited at least 10ms for the producer
    assert!(
        elapsed >= Duration::from_millis(5),
        "Should have blocked waiting for item"
    );
}

/// Test that NotifyQueue returns None when queue is closed.
#[test]
#[traced_test]
fn test_notify_queue_returns_none_when_closed() {
    let queue: NotifyQueue<i32> = NotifyQueue::unbounded();

    // Close the queue
    queue.close();

    // Should return None immediately since queue is closed
    let result = queue.wait_for_item(Duration::from_millis(100));
    assert!(result.is_none());
}

/// Test that NotifyQueue correctly receives items with notification.
#[test]
#[traced_test]
fn test_notify_queue_receives_item_with_notification() {
    let queue: NotifyQueue<i32> = NotifyQueue::unbounded();

    // Push an item
    queue.push(42).expect("push should succeed");

    // Should receive immediately with short timeout
    let result = queue.wait_for_item(Duration::from_micros(100));
    assert_eq!(result, Some(NotificationItem::Ready(42)));
}

/// Test NotifyQueue handles multiple items with blocking behavior.
/// This simulates a stream of values being processed.
#[test]
#[traced_test]
fn test_notify_queue_multiple_items_blocking() {
    let queue: Arc<NotifyQueue<i32>> = Arc::new(NotifyQueue::unbounded());
    let queue_clone = queue.clone();

    // Producer pushes items with small delays
    let producer = std::thread::spawn(move || {
        for i in 0..5 {
            std::thread::sleep(Duration::from_millis(5));
            queue_clone.push(i).expect("push should succeed");
        }
    });

    // Receive all items (retry loop for each)
    let mut received = Vec::new();
    for _ in 0..5 {
        loop {
            match queue.wait_for_item(Duration::from_micros(100)) {
                Some(NotificationItem::Ready(item)) => { received.push(item); break; }
                Some(NotificationItem::None) => continue,
                None => break,
            }
        }
    }

    producer.join().expect("producer should complete");

    assert_eq!(received, vec![0, 1, 2, 3, 4]);
}

/// Test that demonstrates the timeout race condition handling.
/// Item added between initial check and lock acquisition.
#[test]
#[traced_test]
fn test_notify_queue_race_condition_handling() {
    let queue: NotifyQueue<i32> = NotifyQueue::unbounded();

    // Push item before wait
    queue.push(100).unwrap();

    // Wait with timeout should return immediately with the item
    // This tests the race condition where item is added before lock
    let start = Instant::now();
    let result = queue.wait_for_item(Duration::from_millis(100));
    let elapsed = start.elapsed();

    assert_eq!(result, Some(NotificationItem::Ready(100)));
    // Should be very fast since item was already there
    assert!(
        elapsed < Duration::from_millis(10),
        "Race handling took too long"
    );
}

/// Test bounded NotifyQueue capacity handling in single-threaded mode.
#[test]
#[traced_test]
fn test_notify_queue_bounded_capacity() {
    let queue: NotifyQueue<i32> = NotifyQueue::bounded(2);

    // Fill to capacity
    queue.push(1).unwrap();
    queue.push(2).unwrap();

    // Third push should fail (bounded)
    let result = queue.push(3);
    assert!(
        result.is_err(),
        "Push should fail when bounded queue is full"
    );

    // Pop one to make space
    assert_eq!(queue.pop(), Ok(1));

    // Now push should succeed
    queue.push(3).unwrap();
}

/// Test that extremely short timeout parameter still results in blocking behavior.
/// The timeout is just for internal re-checking, not for returning None.
#[test]
#[traced_test]
fn test_notify_queue_timeout_is_recheck_interval() {
    let queue: Arc<NotifyQueue<i32>> = Arc::new(NotifyQueue::unbounded());
    let queue_clone = queue.clone();

    // Producer pushes item after delay
    let producer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        queue_clone.push(99).expect("push should succeed");
    });

    // Even with ultra-short timeout parameter, should block until item available (retry loop)
    let start = Instant::now();
    let mut result = None;
    loop {
        match queue.wait_for_item(Duration::from_micros(1)) {
            Some(NotificationItem::Ready(val)) => { result = Some(val); break; }
            Some(NotificationItem::None) => continue,
            None => break,
        }
    }
    let elapsed = start.elapsed();

    producer.join().expect("producer should complete");

    assert_eq!(result, Some(99));
    // Should have blocked at least 20ms
    assert!(
        elapsed >= Duration::from_millis(10),
        "Should have blocked despite ultra-short timeout param"
    );
}

// ============================================================================
// NotifyRecvIter Tests
// ============================================================================

/// Test that NotifyRecvIter blocks until item is available.
#[test]
#[traced_test]
fn test_notify_recv_iterator_blocks_until_item() {
    let queue: Arc<NotifyQueue<i32>> = Arc::new(NotifyQueue::unbounded());
    let recv_iter = NotifyRecvIter::new(queue.clone());
    let queue_clone = queue.clone();

    // Producer pushes item after delay
    let producer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        queue_clone.push(42).expect("push should succeed");
    });

    // Consumer blocks waiting for item (retry loop until Ready)
    let start = Instant::now();
    let mut result = None;
    loop {
        match recv_iter.block_recv(Duration::from_micros(100)) {
            Some(NotificationItem::Ready(val)) => { result = Some(val); break; }
            Some(NotificationItem::None) => continue,
            None => break,
        }
    }
    let elapsed = start.elapsed();

    producer.join().expect("producer should complete");

    assert_eq!(result, Some(42));
    // Should have blocked at least 20ms
    assert!(
        elapsed >= Duration::from_millis(10),
        "Should have blocked waiting for item"
    );
}

/// Test NotifyRecvIterator correctly iterates with notification.
#[test]
#[traced_test]
fn test_notify_recv_iterator_iterates_with_notification() {
    let queue: Arc<NotifyQueue<i32>> = Arc::new(NotifyQueue::unbounded());
    let iterator = NotifyRecvIterator::from_notify_queue(queue.clone(), Duration::from_micros(100));

    // Push items
    queue.push(1).unwrap();
    queue.push(2).unwrap();
    queue.push(3).unwrap();

    // Collect items through iterator
    let items: Vec<_> = iterator
        .take(3)
        .filter_map(|item| match item {
            NotificationItem::Ready(v) => Some(v),
            _ => None,
        })
        .collect();
    assert_eq!(items, vec![1, 2, 3]);
}

// ============================================================================
// NotifyQueueStreamIterator Tests
// ============================================================================

/// Test NotifyQueueStreamIterator blocks until values are available.
#[test]
#[traced_test]
fn test_notify_queue_stream_iterator_blocks_until_values() {
    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());
    let queue_clone = queue.clone();

    let mut iterator = NotifyQueueStreamIterator::new(queue.clone(), 5, Duration::from_micros(50));

    // Producer pushes values after delay
    let producer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        queue_clone.push(Stream::Next(1)).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        queue_clone.push(Stream::Next(2)).unwrap();
    });

    // Iterator blocks waiting for values (retry loop for each)
    let start = Instant::now();
    let mut results = Vec::new();
    while results.len() < 2 {
        match iterator.next() {
            Some(Stream::Next(v)) => results.push(v),
            Some(Stream::Wait) | Some(Stream::Ignore) | Some(Stream::Init) | Some(Stream::Pending(_)) | Some(Stream::Delayed(_)) => continue,
            None => break,
        }
    }
    let elapsed = start.elapsed();

    producer.join().expect("producer should complete");

    assert_eq!(results, vec![1, 2]);
    // Should have blocked at least 20ms
    assert!(
        elapsed >= Duration::from_millis(15),
        "Iterator should have blocked waiting for values"
    );
}

/// Test NotifyQueueStreamIterator returns None when queue is closed.
#[test]
#[traced_test]
fn test_notify_queue_stream_iterator_returns_none_when_closed() {
    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());

    let mut iterator = NotifyQueueStreamIterator::new(queue.clone(), 5, Duration::from_micros(50));

    // Push a value then close
    queue.push(Stream::Next(42)).unwrap();
    queue.close();

    // First next returns the value
    assert_eq!(iterator.next(), Some(Stream::Next(42)));
    // Second next returns None (queue closed)
    assert_eq!(iterator.next(), None);
}

/// Test NotifyQueueStreamIterator receives values via notification.
#[test]
fn test_notify_queue_stream_iterator_receives_values() {
    let queue: Arc<NotifyQueue<Stream<usize, ()>>> = Arc::new(NotifyQueue::unbounded());

    let mut iterator = NotifyQueueStreamIterator::new(queue.clone(), 5, Duration::from_micros(50));

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
    let global: SharedTaskQueue =
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

    // Run until task completes - predicate just checks state, doesn't hold locks
    executor.run_until(move |state| state == ProgressIndicator::NoWork);

    // Collect all results after executor completes
    let mut receiver = receiver.lock().unwrap();
    while let Some(item) = receiver.next() {
        if let NotificationItem::Ready(TaskStatus::Ready(val)) = item {
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
    let global: SharedTaskQueue =
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

        fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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

    // Run until no more work - predicate just checks state, doesn't hold locks
    let start = Instant::now();
    executor.run_until({
        move |_| false // Keep running until timeout
    });
    let elapsed = start.elapsed();

    // Drain receiver after executor completes
    let mut receiver = receiver.lock().unwrap();
    while let Some(_status) = receiver.next() {}

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
        if let Some(NotificationItem::Ready(val)) = queue.wait_for_item(short_timeout) {
            assert_eq!(val, i);
            consumed += 1;
        }
    }

    assert_eq!(produced, 10);
    assert_eq!(consumed, 10);
}

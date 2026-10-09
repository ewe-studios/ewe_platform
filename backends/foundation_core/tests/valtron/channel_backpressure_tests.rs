//! Integration tests for Feature 07: Channel Backpressure
//!
//! These tests verify that bounded channels correctly apply backpressure
//! without losing messages using the store-and-retry pattern.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use concurrent_queue::ConcurrentQueue;
use foundation_core::valtron::{SharedTaskQueue, TaskStatus, WrapTask};
use foundation_core::valtron::{
    ExecutionAction, InlineAction, InlineActionBehaviour, LocalThreadExecutor, NotifyQueue,
    PriorityOrder, NotificationItem,
};
use foundation_core::{
    retries::ExponentialBackoffDecider,
    synca::{IdleMan, SleepyMan},
    valtron::{ProcessController, ProgressIndicator},
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
// Bounded NotifyQueue Tests (Channel-level backpressure)
// ============================================================================

/// Test that bounded `NotifyQueue` correctly applies backpressure.
/// When the queue is full, push should fail with Full error.
#[test]
fn test_bounded_notify_queue_push_error_full() {
    let queue: NotifyQueue<i32> = NotifyQueue::bounded(2);

    // Fill to capacity
    assert!(queue.push(1).is_ok());
    assert!(queue.push(2).is_ok());

    // Third push should fail with Full error
    let result = queue.push(3);
    assert!(
        result.is_err(),
        "Push should fail when bounded queue is full"
    );

    // The error should contain the value that couldn't be pushed
    match result {
        Err(concurrent_queue::PushError::Full(val)) => {
            assert_eq!(val, 3, "PushError::Full should contain the rejected value");
        }
        _ => panic!("Expected PushError::Full"),
    }
}

/// Test that store-and-retry pattern works correctly:
/// 1. Push a value that gets rejected (Full)
/// 2. Pop to make space
/// 3. Retry the stored value - should succeed
#[test]
fn test_store_and_retry_pattern() {
    let queue: NotifyQueue<i32> = NotifyQueue::bounded(1);

    // Fill the queue
    queue.push(42).expect("first push should succeed");
    assert_eq!(queue.len(), 1);

    // Try to push another - should fail
    let result = queue.push(99);
    let stored_value = match result {
        Err(concurrent_queue::PushError::Full(val)) => val,
        Ok(()) => panic!("Expected PushError::Full"),
        Err(concurrent_queue::PushError::Closed(_)) => panic!("Unexpected PushError::Closed"),
    };

    // Make space by popping
    assert_eq!(queue.pop().unwrap(), 42);
    assert_eq!(queue.len(), 0);

    // Retry the stored value
    assert!(
        queue.push(stored_value).is_ok(),
        "Retry should succeed after making space"
    );

    // Verify the retried value is in the queue
    assert_eq!(queue.pop().unwrap(), 99);
}

/// Test that `NotifyQueue` with bounded capacity properly coordinates
/// producer and consumer through backpressure.
#[test]
#[traced_test]
// producer/produced/consumer/consumed are the domain vocabulary here — renaming
// them to satisfy similar_names would make the test harder to follow.
#[allow(clippy::similar_names)]
fn test_bounded_queue_producer_consumer_coordination() {
    use std::thread;

    let queue: Arc<NotifyQueue<i32>> = Arc::new(NotifyQueue::bounded(3));
    let queue_clone = queue.clone();

    // Producer thread that fills queue and handles backpressure
    let producer = thread::spawn(move || {
        let mut produced = Vec::new();
        let values = vec![1, 2, 3, 4, 5];
        let mut pending = None;

        for val in values {
            // First, try to send any pending value
            if let Some(p) = pending.take() {
                match queue_clone.push(p) {
                    Ok(()) => produced.push(p),
                    Err(concurrent_queue::PushError::Full(v)) => {
                        pending = Some(v);
                    }
                    Err(_) => break,
                }
            }

            // Now try to push the new value
            match queue_clone.push(val) {
                Ok(()) => produced.push(val),
                Err(concurrent_queue::PushError::Full(_)) => {
                    pending = Some(val);
                }
                Err(_) => break,
            }
        }

        // Store any remaining pending value for verification
        (produced, pending)
    });

    // Consumer thread that drains slowly
    let consumer = thread::spawn(move || {
        let mut consumed = Vec::new();

        // Consume with a timeout, simulating slow consumer
        for _ in 0..10 {
            if let Ok(val) = queue.pop() {
                consumed.push(val);
            }
            thread::sleep(Duration::from_millis(5));
        }

        consumed
    });

    let (produced, pending) = producer.join().expect("producer should complete");
    let consumed = consumer.join().expect("consumer should complete");

    // All consumed values should have been produced
    for c in &consumed {
        assert!(
            produced.contains(c) || pending == Some(*c),
            "Consumed value {c:} should be in produced or pending",
        );
    }
}

// ============================================================================
// Executor Integration Tests
// ============================================================================

/// Test that executor handles fast producer / slow consumer scenario
/// without losing messages. The store-and-retry pattern ensures all
/// values are eventually delivered.
#[test]
#[traced_test]
fn test_executor_fast_producer_slow_consumer_no_loss() {
    let global: SharedTaskQueue =
        Arc::new(ConcurrentQueue::bounded(10));

    let results: Arc<Mutex<Vec<i32>>> = Arc::new(Mutex::new(Vec::new()));

    let seed = fastrand::u64(..);
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

    // Task that produces many values quickly
    let task = WrapTask::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10].into_iter());

    let (mut inline_action, receiver) = InlineAction::new(
        InlineActionBehaviour::Lift,
        task,
        Duration::from_micros(50),
    );

    let receiver = Arc::new(Mutex::new(receiver));

    inline_action
        .apply(None, executor.boxed_engine())
        .expect("should schedule");

    // Run until all values are collected
    executor.run_until(|state| state == ProgressIndicator::NoWork);

    // Collect any remaining results
    while let Some(item) = receiver.lock().unwrap().next() {
        if let NotificationItem::Ready(TaskStatus::Ready(val)) = item {
            results.lock().unwrap().push(val);
        }
    }

    let final_results = results.lock().unwrap().clone();
    assert_eq!(final_results.len(), 10, "All 10 values should be received");

    // Verify all values are present
    for i in 1..=10 {
        assert!(final_results.contains(&i), "Should contain value {i}");
    }
}

/// Test that `ready_iter` with bounded channel produces all values without loss.
/// This verifies the `ReadyConsumingIter` store-and-retry pattern works correctly.
#[test]
#[traced_test]
fn test_executor_ready_iter_no_message_loss() {
    // Use a simple test with multiple values to stress the channel
    let global: SharedTaskQueue =
        Arc::new(ConcurrentQueue::bounded(10));

    let results: Arc<Mutex<Vec<i32>>> = Arc::new(Mutex::new(Vec::new()));

    let seed = fastrand::u64(..);
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

    // Task that produces values
    let task = WrapTask::new(vec![100, 200, 300, 400, 500].into_iter());

    let (mut inline_action, receiver) = InlineAction::new(
        InlineActionBehaviour::Lift,
        task,
        Duration::from_micros(50),
    );

    let receiver = Arc::new(Mutex::new(receiver));

    inline_action
        .apply(None, executor.boxed_engine())
        .expect("should schedule");

    // Run until no more work - predicate just checks state, doesn't hold locks
    executor.run_until(move |state| state == ProgressIndicator::NoWork);

    // Collect all results after executor completes
    let mut receiver = receiver.lock().unwrap();
    for item in receiver.by_ref() {
        if let NotificationItem::Ready(TaskStatus::Ready(val)) = item {
            results.lock().unwrap().push(val);
        }
    }

    let final_results = results.lock().unwrap().clone();
    assert_eq!(final_results.len(), 5, "All 5 values should be received");
    assert!(final_results.contains(&100));
    assert!(final_results.contains(&200));
    assert!(final_results.contains(&300));
    assert!(final_results.contains(&400));
    assert!(final_results.contains(&500));
}

// ============================================================================
// Fan-out (split) Backpressure Tests (Feature 45 Part C1)
// ============================================================================

/// Minimal in-order task iterator for the split backpressure tests.
struct SeqTask {
    items: std::vec::IntoIter<TaskStatus<u32, String, foundation_core::valtron::NoAction>>,
}

impl Iterator for SeqTask {
    type Item = TaskStatus<u32, String, foundation_core::valtron::NoAction>;

    fn next(&mut self) -> Option<Self::Item> {
        self.items.next()
    }
}

/// A split continuation parks on a FULL observer queue instead of dropping the
/// matched item (Feature 45 Part C1 replaces the old `force_push` drop-oldest).
///
/// This is the fan-out analogue of the Decision 00 §L1b delivery rule: while the
/// observer is full the source yields `TaskStatus::Depends` (park, turn-count
/// flat) rather than a value; draining a slot unparks it and the stashed item
/// lands — zero loss.
#[test]
#[traced_test]
fn test_split_continuation_parks_on_full_observer_no_loss() {
    use foundation_core::valtron::{Stream, TaskIteratorExt};

    // Three matched items through a depth-1 (queue_size = 1) observer queue.
    let task = SeqTask {
        items: vec![
            TaskStatus::Ready(10),
            TaskStatus::Ready(20),
            TaskStatus::Ready(30),
        ]
        .into_iter(),
    };
    let (mut observer, mut continuation) = task.split_collect_one(|_| true);

    // Step 1: pushes 10 into the depth-1 queue and forwards Ready(10).
    assert!(
        matches!(continuation.next(), Some(TaskStatus::Ready(10))),
        "first step forwards the source value and fills the observer queue"
    );

    // Steps 2 & 3: the queue is full, so the source PARKS on vacancy instead of
    // dropping 10 — turn-count stays flat (repeated Depends, no value consumed).
    assert!(
        matches!(continuation.next(), Some(TaskStatus::Depends(_))),
        "full observer queue parks the source rather than dropping"
    );
    assert!(
        matches!(continuation.next(), Some(TaskStatus::Depends(_))),
        "still parked while the observer stays full"
    );

    // Drain one slot: the stashed copy of 20 now lands and the source advances.
    assert!(matches!(observer.next(), Some(Stream::Next(10))));
    assert!(
        matches!(continuation.next(), Some(TaskStatus::Ready(20))),
        "draining a slot unparks the source and delivers the stashed item"
    );

    // Finish in lockstep and prove every matched item reached the observer.
    let mut received = vec![10u32];
    loop {
        while let Some(stream) = observer.next() {
            match stream {
                Stream::Next(v) => received.push(v),
                _ => break,
            }
        }
        match continuation.next() {
            Some(_) => {}
            None => break,
        }
    }
    for stream in &mut observer {
        if let Stream::Next(v) = stream {
            received.push(v);
        }
    }
    received.sort_unstable();
    assert_eq!(
        received,
        vec![10, 20, 30],
        "backpressured fan-out loses zero items"
    );
}

/// A DROPPED observer does not kill the stream: the continuation stops copying
/// (observer queue closed) but keeps forwarding source values (Feature 45 Part
/// C1 observer-dropped policy).
#[test]
#[traced_test]
fn test_split_continuation_survives_dropped_observer() {
    use foundation_core::valtron::TaskIteratorExt;

    let task = SeqTask {
        items: vec![
            TaskStatus::Ready(1),
            TaskStatus::Ready(2),
            TaskStatus::Ready(3),
        ]
        .into_iter(),
    };
    let (observer, mut continuation) = task.split_collect_one(|_| true);

    // Drop the observer immediately — its queue closes.
    drop(observer);

    // The continuation must keep forwarding every source value (no park, no panic).
    let mut forwarded = Vec::new();
    while let Some(status) = continuation.next() {
        if let TaskStatus::Ready(v) = status {
            forwarded.push(v);
        }
    }
    assert_eq!(
        forwarded,
        vec![1, 2, 3],
        "a dropped observer does not stall or kill the source stream"
    );
}

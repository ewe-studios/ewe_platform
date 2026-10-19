//! Tests for `mpp::TrackedBroadcaster`.

use foundation_core::synca::mpp::TrackedBroadcaster;

#[test]
fn fans_out_to_multiple_subscribers() {
    let b = TrackedBroadcaster::new(16);
    let r1 = b.subscribe();
    let r2 = b.subscribe();
    let r3 = b.subscribe();

    b.broadcast(42);
    b.broadcast(99);

    assert_eq!(r1.recv().unwrap(), 42);
    assert_eq!(r1.recv().unwrap(), 99);
    assert_eq!(r2.recv().unwrap(), 42);
    assert_eq!(r2.recv().unwrap(), 99);
    assert_eq!(r3.recv().unwrap(), 42);
    assert_eq!(r3.recv().unwrap(), 99);
}

#[test]
fn subscriber_added_after_broadcast_misses_earlier_events() {
    let b = TrackedBroadcaster::new(16);
    let r1 = b.subscribe();
    b.broadcast(1);

    let r2 = b.subscribe();
    b.broadcast(2);

    assert_eq!(r1.recv().unwrap(), 1);
    assert_eq!(r1.recv().unwrap(), 2);
    assert_eq!(r2.recv().unwrap(), 2);
    assert!(r2.recv().is_err());
}

#[test]
fn slow_subscriber_evicted_after_max_retries() {
    let b = TrackedBroadcaster::with_max_retries(2, 3);
    let slow = b.subscribe();
    let fast = b.subscribe();

    b.broadcast(1);
    b.broadcast(2);
    assert_eq!(b.subscriber_count(), 2);

    // Drain fast so it stays healthy; slow stays full
    assert_eq!(fast.recv().unwrap(), 1);
    assert_eq!(fast.recv().unwrap(), 2);

    // 3 more pushes: slow fails each time → evicted at 3
    b.broadcast(3);
    b.broadcast(4);
    b.broadcast(5);

    assert_eq!(b.subscriber_count(), 1);
    assert!(slow.is_closed());

    assert_eq!(fast.recv().unwrap(), 3);
}

#[test]
fn success_resets_failure_counter() {
    let b = TrackedBroadcaster::with_max_retries(2, 3);
    let r = b.subscribe();

    b.broadcast(1);
    b.broadcast(2);
    // 2 failures (queue full, capacity 2)
    b.broadcast(3);
    b.broadcast(4);
    assert_eq!(b.subscriber_count(), 1);

    // Drain to reset
    r.recv().unwrap();
    r.recv().unwrap();

    // Next broadcast succeeds → resets counter
    b.broadcast(5);
    assert_eq!(b.subscriber_count(), 1);
    assert_eq!(r.recv().unwrap(), 5);
}

#[test]
fn closed_receiver_removed_on_next_broadcast() {
    let b = TrackedBroadcaster::new(16);
    let r1 = b.subscribe();
    let r2 = b.subscribe();
    assert_eq!(b.subscriber_count(), 2);

    assert!(
        r2.close(),
        "closing an open receiver reports that it closed it"
    );
    b.broadcast(1);

    assert_eq!(b.subscriber_count(), 1);
    assert_eq!(r1.recv().unwrap(), 1);
}

#[test]
fn close_closes_all_subscribers() {
    let b = TrackedBroadcaster::<i32>::new(16);
    let r1 = b.subscribe();
    let r2 = b.subscribe();

    b.close();

    assert!(r1.is_closed());
    assert!(r2.is_closed());
    assert_eq!(b.subscriber_count(), 0);
}

#[test]
fn broadcast_with_no_subscribers_is_noop() {
    let b = TrackedBroadcaster::<i32>::new(16);
    b.broadcast(42);
    assert_eq!(b.subscriber_count(), 0);
}

#[test]
fn subscribe_and_broadcast_from_multiple_threads() {
    use std::sync::Arc;

    let b = Arc::new(TrackedBroadcaster::new(64));
    let r = b.subscribe();

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let b = b.clone();
            std::thread::spawn(move || {
                b.broadcast(i);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let mut received = Vec::new();
    while let Ok(v) = r.recv() {
        received.push(v);
    }
    received.sort();
    assert_eq!(received, vec![0, 1, 2, 3]);
}

#[test]
fn default_has_capacity_64() {
    let b = TrackedBroadcaster::<i32>::default();
    let r = b.subscribe();
    for i in 0..64 {
        b.broadcast(i);
    }
    for i in 0..64 {
        assert_eq!(r.recv().unwrap(), i);
    }
}

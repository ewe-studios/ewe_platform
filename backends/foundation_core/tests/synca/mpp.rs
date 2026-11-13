//! Tests for the `mpp` channels (`bounded`, `unbounded`, `RecvIterator`).

use std::{sync::Arc, thread, time::Duration};

use concurrent_queue::ConcurrentQueue;

use foundation_core::synca::mpp::RecvIterator;

use foundation_core::synca::mpp::{bounded, unbounded, ReceiverError};

/// A real clock: this test only measures the monotonic timeout.
fn clock() -> foundation_context::Ctx {
    foundation_context::Ctx::fixed(std::sync::Arc::new(
        foundation_context::MonotonicClock::new(),
    ))
}

#[test]
fn unbounded_channel() {
    let (sender, receiver) = unbounded();
    sender.send(42).unwrap();
    assert_eq!(receiver.recv().unwrap(), 42);
}

#[test]
fn bounded_channel() {
    let (sender, receiver) = bounded(1);
    sender.send(42).unwrap();
    assert_eq!(receiver.recv().unwrap(), 42);
}

#[test]
fn fail_to_receive_item_after_timeout() {
    let (sender, receiver) = bounded(1);

    let sender_clone = sender.clone();
    let join_handler = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        sender_clone.send(42).unwrap();
    });

    assert!(
        matches!(
            receiver.recv_timeout(Duration::from_millis(50), &clock()),
            Err(ReceiverError::Timeout)
        ),
        "should fail to receive item after timeout"
    );

    join_handler.join().expect("should finish");
}

#[test]
fn can_receive_item_after_timeout() {
    let (sender, receiver) = bounded(1);

    let sender_clone = sender.clone();
    let join_handler = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        sender_clone.send(42).unwrap();
    });

    // The timeout must be comfortably longer than the sender's 100 ms sleep:
    // a 2 ms margin flaked under scheduler load, where the sender thread can
    // be preempted past a ~100 ms timeout and the recv times out first.
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_millis(500), &clock())
            .unwrap(),
        42
    );
    join_handler.join().expect("should finish");
}

#[test]
fn can_receive_from_recv_iterator() {
    let chan: Arc<ConcurrentQueue<usize>> = Arc::new(ConcurrentQueue::bounded(10));

    chan.push(42).expect("Must receive value");
    chan.close();

    let chan_iter = RecvIterator::from_ten_nano(chan);

    let items: Vec<usize> = chan_iter.collect();
    dbg!("Received values: {:?}", &items);

    assert_eq!(items, vec![42]);
}

/// A spurious wake must not subtract the elapsed time twice.
///
/// WHY: `recv_timeout` took the time since the original start on every turn,
/// and subtracted it from the remaining time each time. After two wakes at 4 s
/// and 8 s of a 10 s timeout, the old code had 10 - 4 = 6 s left, saw 8 s
/// elapsed and timed out two seconds early.
#[cfg(feature = "test-clock")]
#[test]
fn recv_timeout_does_not_time_out_early_after_spurious_wakes() {
    use foundation_core::valtron::SimulatedClock;
    use std::sync::mpsc;

    let sim = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = foundation_context::Ctx::fixed(sim.clone());
    let (_sender, receiver) = unbounded::<u32>();
    let (tx, done) = mpsc::channel();
    let handle = thread::spawn(move || {
        let _ = tx.send(receiver.recv_timeout(Duration::from_secs(10), &ctx));
    });
    let waiter = handle.thread().clone();

    // Two spurious wakes, 4 s of simulated time apart each.
    for _ in 0..2 {
        thread::sleep(Duration::from_millis(100));
        sim.advance(Duration::from_secs(4));
        waiter.unpark();
    }
    thread::sleep(Duration::from_millis(200));
    assert!(
        done.try_recv().is_err(),
        "timed out after 8 s of a 10 s timeout"
    );

    // Past the 10 s mark the wait ends with a timeout.
    sim.advance(Duration::from_secs(3));
    waiter.unpark();
    let outcome = done
        .recv_timeout(Duration::from_secs(10))
        .expect("the wait must end after 11 s");
    assert!(matches!(outcome, Err(ReceiverError::Timeout)));
    handle.join().expect("join");
}

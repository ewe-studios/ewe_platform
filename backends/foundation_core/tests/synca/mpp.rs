//! Tests for the `mpp` channels (`bounded`, `unbounded`, `RecvIterator`).

use std::{sync::Arc, thread, time::Duration};

use concurrent_queue::ConcurrentQueue;

use foundation_core::synca::mpp::RecvIterator;

use foundation_core::synca::mpp::{bounded, unbounded, ReceiverError};

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
            receiver.recv_timeout(Duration::from_millis(50)),
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
        receiver.recv_timeout(Duration::from_millis(500)).unwrap(),
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

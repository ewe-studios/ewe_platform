//! The `mpp` channel readiness wakes a parked consumer (issue #201).
//!
//! WHY: a task that parks on a channel needs a readiness that fires when a
//! sender pushes. The `mpp` channel had none, so a consumer could not park.
//!
//! WHAT: a receiver's readiness is ready while the channel has an item or is
//! closed. Each send and each close wakes the registered waker.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Wake;

use foundation_core::synca::mpp;
use foundation_core::valtron::{EventReadiness, ReadinessWaker};

/// Counts the wakes a readiness delivers.
#[derive(Default)]
struct CountingWake {
    wakes: AtomicUsize,
}

impl Wake for CountingWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}

fn counting_waker(counter: &Arc<CountingWake>) -> ReadinessWaker {
    ReadinessWaker::from_waker(std::task::Waker::from(Arc::clone(counter)))
}

#[test]
fn readiness_is_ready_only_while_the_queue_has_an_item() {
    let (tx, rx) = mpp::unbounded::<u32>();
    let ready = rx.readiness();
    assert!(!ready.is_ready(None), "an empty channel is not ready");

    tx.send(7).expect("send");
    assert!(ready.is_ready(None), "a channel with an item is ready");

    assert_eq!(rx.recv().expect("recv"), 7);
    assert!(!ready.is_ready(None), "a drained channel is not ready");
}

#[test]
fn each_send_wakes_a_registered_consumer_once() {
    let (tx, rx) = mpp::unbounded::<u32>();
    let ready = rx.readiness();
    let counter = Arc::new(CountingWake::default());
    assert!(
        ready.register(&counting_waker(&counter)),
        "the readiness takes the waker"
    );

    tx.send(1).expect("send");
    assert_eq!(
        counter.wakes.load(Ordering::SeqCst),
        1,
        "one send wakes the consumer once"
    );

    tx.send(2).expect("send");
    assert_eq!(
        counter.wakes.load(Ordering::SeqCst),
        2,
        "a second send wakes the consumer once more"
    );
}

#[test]
fn a_closed_channel_reports_ready_and_wakes_the_consumer() {
    let (tx, rx) = mpp::unbounded::<u32>();
    let ready = rx.readiness();
    let counter = Arc::new(CountingWake::default());
    assert!(ready.register(&counting_waker(&counter)));

    let _ = tx.close();

    assert!(
        ready.is_ready(None),
        "a closed channel is ready, so the consumer observes the end"
    );
    assert_eq!(
        counter.wakes.load(Ordering::SeqCst),
        1,
        "close wakes the consumer once"
    );
}

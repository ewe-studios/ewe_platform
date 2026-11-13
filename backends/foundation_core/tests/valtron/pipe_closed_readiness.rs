//! `PipeSender::closed_readiness` reports only the close (spec 70 feature 126).
//!
//! WHY: the only producer-side readiness was `vacancy()`, which is ready on
//! "not full". A producer that must wait for "the consumer dropped the pipe"
//! therefore woke on every consumer pop. Feature 125 hit this in the HTTP/2
//! pump, which paid one extra poll per response chunk.
//!
//! WHAT: three tests drive the readiness directly, with a counting waker, so
//! the number of wakes is exact:
//! - it is not ready while the receiver lives, even on a full pipe;
//! - a pop does not wake a task parked on it;
//! - a close wakes it once, and a second close wakes nobody.

#![cfg(not(target_family = "wasm"))]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Wake, Waker};

use foundation_core::valtron::{EventReadiness, Pipe, ReadinessWaker};

/// Counts how many times a waker is woken.
struct CountingWake(AtomicUsize);

impl CountingWake {
    fn new() -> Arc<Self> {
        Arc::new(Self(AtomicUsize::new(0)))
    }

    fn count(self: &Arc<Self>) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

impl Wake for CountingWake {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// The readiness waker for `wake`. Two calls with the same `wake` produce
/// wakers that `will_wake` each other, as two parks of one task do.
fn readiness_waker(wake: &Arc<CountingWake>) -> ReadinessWaker {
    ReadinessWaker::from_waker(Waker::from(Arc::clone(wake)))
}

/// The signal is not the vacancy signal: it stays false on a full, open pipe
/// and becomes true only once the receiver drops.
#[test]
fn closed_readiness_is_ready_only_after_the_receiver_drops() {
    let (tx, rx) = Pipe::<u32>::with_depth(2);
    let closed = tx.closed_readiness();

    // A live, empty pipe has room, but that is the vacancy signal, not this one.
    assert!(tx.vacancy().is_ready(None), "the empty pipe has room");
    assert!(!closed.is_ready(None), "the receiver is still alive");

    // A full, live pipe still has not closed.
    tx.try_send(1).expect("send 1");
    tx.try_send(2).expect("send 2");
    assert!(tx.is_full(), "the pipe is full");
    assert!(!closed.is_ready(None), "full is not closed");

    drop(rx);
    assert!(closed.is_ready(None), "the last receiver dropped");
}

/// A consumer pop wakes a producer parked on vacancy, but not one parked on the
/// close-only signal. This is the regression the feature removes.
#[test]
fn a_pop_does_not_wake_the_closed_reader() {
    let (tx, rx) = Pipe::<u32>::with_depth(2);
    tx.try_send(1).expect("send 1");
    tx.try_send(2).expect("send 2");

    let closed = tx.closed_readiness();
    let wake = CountingWake::new();
    assert!(
        closed.register(&readiness_waker(&wake)),
        "the close-only signal wakes its waiters"
    );

    // The pop frees a slot and fires the vacancy notifier, not the closed one.
    assert_eq!(rx.try_recv(), Ok(1));
    assert_eq!(wake.count(), 0, "a pop must not wake the closed reader");

    drop(rx);
    assert_eq!(wake.count(), 1, "the close wakes the reader exactly once");
}

/// Two parks of one task and two closes produce one wake, not four.
#[test]
fn a_closed_pipe_wakes_the_closed_reader_only_once() {
    let (tx, rx) = Pipe::<u32>::with_depth(1);
    let closed = tx.closed_readiness();
    let wake = CountingWake::new();

    // Two parks of the same task: the notifier keeps one waker per task.
    assert!(closed.register(&readiness_waker(&wake)));
    assert!(closed.register(&readiness_waker(&wake)));

    // The receiver drops (the first close), then the sender tries to close an
    // already closed pipe. Only the first close may wake the parked reader.
    drop(rx);
    assert_eq!(wake.count(), 1, "the receiver drop closes and wakes once");
    assert!(!tx.close(), "the pipe is already closed");
    assert_eq!(wake.count(), 1, "a second close wakes nobody");
}

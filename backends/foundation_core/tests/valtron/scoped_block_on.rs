//! Tests for `block_on_future_scoped`, the scoped sync boundary.
//!
//! The function drives a borrowed, non-`Send` future on the caller's thread.
//! It parks the thread on `Pending` and returns when the future wakes it.

use foundation_core::valtron::block_on_future_scoped;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::Duration;

#[test]
fn drives_a_borrowed_non_send_future_on_the_calling_thread() {
    // `Rc` is not `Send` and the future borrows it, so the future is neither
    // `Send` nor `'static`. The scoped boundary still drives it.
    let local = Rc::new(String::from("scoped"));
    let len = block_on_future_scoped(async { local.len() });
    assert_eq!(len, 6);
    // The borrow ended when the future returned.
    assert_eq!(local.as_str(), "scoped");
}

/// State shared with a producer thread.
struct DeferredState {
    value: Option<u32>,
    polls: u32,
}

/// A future that hands its waker to a channel on the first poll, then returns
/// `Pending` until another thread wakes it.
struct Deferred {
    state: Arc<Mutex<DeferredState>>,
    waker_tx: Option<mpsc::Sender<Waker>>,
}

impl Future for Deferred {
    type Output = u32;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
        let this = self.get_mut();
        let mut state = this.state.lock().expect("state");
        state.polls += 1;
        if let Some(value) = state.value.take() {
            return Poll::Ready(value);
        }
        if let Some(tx) = this.waker_tx.take() {
            tx.send(cx.waker().clone()).ok();
        }
        Poll::Pending
    }
}

#[test]
fn a_pending_future_is_woken_from_another_thread() {
    let state = Arc::new(Mutex::new(DeferredState {
        value: None,
        polls: 0,
    }));
    let (waker_tx, waker_rx) = mpsc::channel::<Waker>();
    let producer_state = Arc::clone(&state);
    let producer = thread::spawn(move || {
        let waker = waker_rx.recv().expect("the future registers its waker");
        thread::sleep(Duration::from_millis(20));
        producer_state.lock().expect("state").value = Some(7);
        waker.wake();
    });

    let value = block_on_future_scoped(Deferred {
        state: Arc::clone(&state),
        waker_tx: Some(waker_tx),
    });
    producer.join().expect("producer");

    assert_eq!(value, 7);
    let polls = state.lock().expect("state").polls;
    // One `Pending` poll, one `Ready` poll. A busy loop would poll many times.
    assert_eq!(polls, 2);
}

/// A future that wakes itself once per poll until it is ready.
struct SelfWaking {
    remaining: u32,
}

impl Future for SelfWaking {
    type Output = u32;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
        if self.remaining == 0 {
            return Poll::Ready(42);
        }
        self.remaining -= 1;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

#[test]
fn a_self_woken_future_completes() {
    assert_eq!(block_on_future_scoped(SelfWaking { remaining: 3 }), 42);
}

#[test]
#[should_panic(expected = "scoped boom")]
fn a_panic_in_the_future_reaches_the_caller() {
    let _: () = block_on_future_scoped(async {
        panic!("scoped boom");
    });
}

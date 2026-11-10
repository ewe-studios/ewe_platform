//! `RpcCtx` / `RequestContext` / `CancelSignal` tests (spec-41 F16 / Decision 04):
//! downstream-only extension visibility, cancel shared across clones, opt-in
//! one-way linking, and the awaitable cancel path.

use std::sync::Arc;
use std::time::Duration;

use foundation_connectrpc::{CancelSignal, RpcCtx};

// A minimal std block_on so we can exercise `cancelled().await` without a runtime.
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

// ── Extensions: downstream-only visibility ────────────────────────────────────

#[test]
fn with_extension_is_downstream_only() {
    let upstream = RpcCtx::background();
    // A layer keeps its own clone, then hands a derived context downstream.
    let upstream_view = upstream.clone();
    let downstream = upstream.with_extension(99u32);

    assert_eq!(downstream.extension::<u32>(), Some(&99));
    assert_eq!(
        upstream_view.extension::<u32>(),
        None,
        "an earlier clone must not see a later insert"
    );
}

#[test]
fn with_extension_rebuilds_and_moves() {
    let base = RpcCtx::background();
    let derived = base
        .clone()
        .with_extension(1u32)
        .with_extension("hi".to_string());
    assert_eq!(derived.extension::<u32>(), Some(&1));
    assert_eq!(derived.extension::<String>(), Some(&"hi".to_string()));
    // Original untouched.
    assert_eq!(base.extension::<u32>(), None);
}

// ── Cancellation: shared across clones ────────────────────────────────────────

#[test]
fn cancel_fires_across_clones() {
    let ctx = RpcCtx::background();
    let clone = ctx.clone();
    assert!(!ctx.is_canceled());
    assert!(!clone.is_canceled());

    ctx.cancel_signal().cancel();

    assert!(ctx.is_canceled());
    assert!(clone.is_canceled(), "a clone shares the call's signal");
}

#[test]
fn with_cancellation_detaches() {
    let ctx = RpcCtx::background();
    let detached = ctx.clone().with_cancellation(CancelSignal::new());
    ctx.cancel_signal().cancel();
    assert!(ctx.is_canceled());
    assert!(!detached.is_canceled(), "a detached signal is independent");
}

// ── Linking: one-way down ─────────────────────────────────────────────────────

#[test]
fn linked_propagates_parent_to_child_only() {
    // Parent fires → child fires.
    let parent = CancelSignal::new();
    let child = CancelSignal::linked(&parent);
    assert!(!child.is_canceled());
    parent.cancel();
    assert!(child.is_canceled(), "parent cancel reaches the child");

    // Child fires → parent unaffected.
    let parent2 = CancelSignal::new();
    let child2 = CancelSignal::linked(&parent2);
    child2.cancel();
    assert!(child2.is_canceled());
    assert!(
        !parent2.is_canceled(),
        "child cancel never reaches the parent"
    );
}

// ── Awaitable cancel ──────────────────────────────────────────────────────────

#[test]
fn cancelled_await_wakes_on_cross_clone_cancel() {
    let signal = CancelSignal::new();
    let firer = signal.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        firer.cancel();
    });
    // Returns once the other thread fires the shared signal.
    block_on(signal.cancelled());
    assert!(signal.is_canceled());
}

#[test]
fn cancelled_await_wakes_via_parent_link() {
    let parent = CancelSignal::new();
    let child = CancelSignal::linked(&parent);
    let firer = parent.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        firer.cancel();
    });
    block_on(child.cancelled());
    assert!(child.is_canceled());
}

#[test]
fn cancelled_await_wakes_every_waiter() {
    let signal = CancelSignal::new();
    let first = signal.clone();
    let second = signal.clone();
    let firer = signal.clone();
    let one = std::thread::spawn(move || block_on(first.cancelled()));
    let two = std::thread::spawn(move || block_on(second.cancelled()));
    std::thread::sleep(Duration::from_millis(100));
    firer.cancel();
    one.join().expect("first waiter");
    two.join().expect("second waiter");
    assert!(signal.is_canceled());
}

#[test]
fn cancel_twice_stays_canceled() {
    let signal = CancelSignal::new();
    signal.cancel();
    signal.cancel();
    assert!(signal.is_canceled());
}

/// A finished `cancelled()` wait does not keep its waker (spec 70 feature 140).
///
/// Each facade wait builds a fresh `cancelled()` future, and valtron hands each
/// poll a different waker. Without a liveness token on the bridged waker, the
/// signal keeps every one of them until it fires.
#[test]
fn finished_cancel_waits_do_not_accumulate_wakers() {
    use std::future::Future;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Wake, Waker};

    struct CountingWake(AtomicUsize);

    impl Wake for CountingWake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let signal = CancelSignal::new();
    // The wakers stay alive, so each one is distinct and `will_wake` is false.
    // That is what a stream task gets when valtron re-parks it on each poll.
    let wakers: Vec<Waker> = (0..1000)
        .map(|_| Waker::from(Arc::new(CountingWake(AtomicUsize::new(0)))))
        .collect();
    for waker in &wakers {
        let mut cx = std::task::Context::from_waker(waker);
        let future = signal.cancelled();
        let mut future = std::pin::pin!(future);
        assert!(
            future.as_mut().poll(&mut cx).is_pending(),
            "a signal that did not fire must keep the wait pending"
        );
        // `future` drops here: its wait is gone.
    }

    let registered = signal.waiter_count();
    assert!(
        registered <= 2,
        "1000 finished waits left {registered} registered wakers; a finished \
         wait must be pruned on the next register"
    );
}

/// Two live cancel waits of one task must both keep a registration.
///
/// Both waits poll with the same task waker, so a de-duplication on `will_wake`
/// alone keeps the first wait's liveness token only. If that wait ends, the
/// signal prunes its only waker and the second wait never wakes.
#[test]
fn two_live_cancel_waits_of_one_task_stay_registered() {
    use std::future::Future;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Wake, Waker};

    struct CountingWake(AtomicUsize);

    impl Wake for CountingWake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let signal = CancelSignal::new();
    let counter = Arc::new(CountingWake(AtomicUsize::new(0)));
    let waker = Waker::from(Arc::clone(&counter));
    let mut cx = std::task::Context::from_waker(&waker);

    let mut first = Box::pin(signal.cancelled());
    assert!(first.as_mut().poll(&mut cx).is_pending());
    let mut second = Box::pin(signal.cancelled());
    assert!(second.as_mut().poll(&mut cx).is_pending());
    assert_eq!(
        signal.waiter_count(),
        2,
        "two live waits of one task are two registrations; a de-duplication on \
         will_wake leaves only the first"
    );

    // The first wait ends. The signal must still wake the pending second wait.
    drop(first);
    signal.cancel();
    assert!(second.as_mut().poll(&mut cx).is_ready());
    assert!(
        counter.0.load(Ordering::SeqCst) >= 1,
        "the pending second wait's task must be woken"
    );
}

// ── Deadline ──────────────────────────────────────────────────────────────────

#[test]
fn with_deadline_sets_remaining_timeout() {
    let ctx = RpcCtx::background();
    assert!(ctx.remaining_timeout().is_none());
    let timed = ctx.with_deadline(Duration::from_secs(30));
    let remaining = timed.remaining_timeout().expect("deadline set");
    assert!(remaining <= Duration::from_secs(30) && remaining > Duration::from_secs(29));
}

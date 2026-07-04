//! Ctx / RequestContext / CancelSignal tests (spec-41 F16 / Decision 04):
//! downstream-only extension visibility, cancel shared across clones, opt-in
//! one-way linking, and the awaitable cancel path.

use std::sync::Arc;
use std::time::Duration;

use foundation_connectrpc::{CancelSignal, Ctx};

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
    let upstream = Ctx::background();
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
    let base = Ctx::background();
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
    let ctx = Ctx::background();
    let clone = ctx.clone();
    assert!(!ctx.is_canceled());
    assert!(!clone.is_canceled());

    ctx.cancel_signal().cancel();

    assert!(ctx.is_canceled());
    assert!(clone.is_canceled(), "a clone shares the call's signal");
}

#[test]
fn with_cancellation_detaches() {
    let ctx = Ctx::background();
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
    assert!(!parent2.is_canceled(), "child cancel never reaches the parent");
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

// ── Deadline ──────────────────────────────────────────────────────────────────

#[test]
fn with_deadline_sets_remaining_timeout() {
    let ctx = Ctx::background();
    assert!(ctx.remaining_timeout().is_none());
    let timed = ctx.with_deadline(Duration::from_secs(30));
    let remaining = timed.remaining_timeout().expect("deadline set");
    assert!(remaining <= Duration::from_secs(30) && remaining > Duration::from_secs(29));
}

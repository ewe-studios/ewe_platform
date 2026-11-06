//! A bridged readiness waker reports when its wait is gone (spec 70 feature 140).
//!
//! WHY: `ReadinessWaker::from_waker` sets no liveness state, so `is_live()` is
//! always true. A source that keeps the wakers of finished bridge waits then
//! grows without bound until it fires. `CancelSignal` was the one found.
//!
//! WHAT: a bridged waker with a live token holds a registration. A bridged
//! waker whose token is false is dropped on the next register or notify.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Wake, Waker};

use foundation_core::valtron::{ReadinessNotifier, ReadinessWaker};

/// A wake target that counts its wakes. Two live instances are two distinct
/// wakers, so the notifier does not treat them as the same task.
struct CountingWake(AtomicUsize);

impl Wake for CountingWake {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn distinct_waker() -> Waker {
    Waker::from(Arc::new(CountingWake(AtomicUsize::new(0))))
}

/// A live bridged waker holds its waiter. A dead one is pruned, so a source
/// does not keep the waker of a wait that already finished.
#[test]
fn a_dead_bridged_waker_is_dropped() {
    let notifier = ReadinessNotifier::new();

    // A live token holds the waiter.
    let first_live = Arc::new(AtomicBool::new(true));
    let first = ReadinessWaker::from_waker_with_liveness(distinct_waker(), Arc::clone(&first_live));
    assert!(notifier.register(&first));
    assert_eq!(
        notifier.waiter_count(),
        1,
        "a bridged waker with a live token must hold its registration"
    );

    // The first wait is gone. The next register must drop it, not keep it.
    first_live.store(false, Ordering::SeqCst);
    let second_live = Arc::new(AtomicBool::new(true));
    let second =
        ReadinessWaker::from_waker_with_liveness(distinct_waker(), Arc::clone(&second_live));
    assert!(notifier.register(&second));
    assert_eq!(
        notifier.waiter_count(),
        1,
        "the dead bridged waker must be dropped on the next register; without \
         a liveness token the count is 2"
    );

    // A notify prunes a dead bridged waker as well.
    second_live.store(false, Ordering::SeqCst);
    notifier.notify();
    assert!(
        !notifier.has_waiters(),
        "a notify must drop a bridged waker whose wait is gone"
    );
}

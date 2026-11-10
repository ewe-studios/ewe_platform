//! Regression tests for the wake path of the split observers.
//!
//! WHY: the async stream bridge (`StreamNextStream`) asks its stream for a wake
//! callback when a pull finds an empty, open queue. The split observers used
//! the default `NotSupported`, so the bridge woke itself and polled again. One
//! executor worker spun on that loop until the producer pushed a value, and the
//! proxy stream tests starved under load (issue #329).
//!
//! HOW: an empty observer must accept a wake callback, must not fire it before
//! the producer pushes, and must fire it exactly once when the producer pushes.

#![cfg(feature = "multi")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_core::valtron::{
    CollectionState, Stream, TaskIterator, TaskIteratorExt, TaskStatus, WakeResult, WakeableStream,
};

use foundation_context::Ctx;

/// A task that yields its items once, in reverse order.
struct TestTask {
    items: Vec<TaskStatus<u32, String>>,
}

impl TaskIterator for TestTask {
    type Ready = u32;
    type Pending = String;

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.items.pop()
    }
}

#[test]
fn an_empty_split_observer_accepts_a_wake_callback_and_fires_it_on_push() {
    let ctx = Ctx::host();
    let task = TestTask {
        items: vec![TaskStatus::Ready(7)],
    };
    let (mut observer, mut continuation) =
        task.split_collect_until_map(|value| (CollectionState::Close(true), Some(*value)), 4);

    // An empty, open observer answers `Wait`: this is the pull that asks the
    // async bridge for a wake.
    assert_eq!(observer.next(), Some(Stream::Wait));

    let fired = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&fired);
    let result = observer.wake(Box::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    }));
    assert_eq!(
        result,
        WakeResult::Successful,
        "an empty split observer must accept a wake callback"
    );
    assert_eq!(
        fired.load(Ordering::SeqCst),
        0,
        "the callback must not fire before the producer pushes"
    );

    // Drive the producer: it copies the head into the observer queue.
    while continuation.next_status(ctx.clone()).is_some() {}

    assert_eq!(
        fired.load(Ordering::SeqCst),
        1,
        "a push must fire the registered wake exactly once"
    );

    // The wake does not consume the item: the pull still gets it.
    assert_eq!(observer.next(), Some(Stream::Next(7)));
}

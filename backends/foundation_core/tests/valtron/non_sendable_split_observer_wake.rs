//! Regression test for the wake path of the non-sendable split observers.
//!
//! WHY: the async stream bridge (`StreamNextStream`) asks its stream for a wake
//! callback when a pull finds an empty, open queue. The non-sendable split
//! observers used the default `NotSupported`, so the bridge woke itself and
//! polled again. On the single-threaded executor, which runs on wasm, that spin
//! loop blocked the only thread, so the producer never ran (issue #407).
//!
//! HOW: an empty observer must accept a non-`Send` wake callback, must not fire
//! it before the producer pushes, and must fire it exactly once when the
//! producer pushes.

#![cfg(not(feature = "multi"))]

use std::cell::Cell;
use std::rc::Rc;

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
fn a_non_sendable_split_observer_accepts_a_wake_callback_and_fires_it_on_push() {
    let ctx = Ctx::host();
    let task = TestTask {
        items: vec![TaskStatus::Ready(7)],
    };
    let (mut observer, mut continuation) =
        task.split_collect_until_map(|value| (CollectionState::Close(true), Some(*value)), 4);

    // An empty, open observer answers `Wait`: this is the pull that asks the
    // async bridge for a wake.
    assert_eq!(observer.next(), Some(Stream::Wait));

    // The callback captures an `Rc`, so it has no `Send` bound. The observer
    // must take it anyway: the single-threaded executor never crosses threads.
    let fired = Rc::new(Cell::new(0));
    let counter = Rc::clone(&fired);
    let result = observer.wake(Box::new(move || counter.set(counter.get() + 1)));
    assert_eq!(
        result,
        WakeResult::Successful,
        "a non-sendable split observer must accept a wake callback"
    );
    assert_eq!(
        fired.get(),
        0,
        "the callback must not fire before the producer pushes"
    );

    // Drive the producer: it copies the head into the observer queue.
    while continuation.next_status(ctx.clone()).is_some() {}

    assert_eq!(
        fired.get(),
        1,
        "a push must fire the registered wake exactly once"
    );

    // The wake does not consume the item: the pull still gets it.
    assert_eq!(observer.next(), Some(Stream::Next(7)));
}

//! Regression test for issue #423: a waker built from a non-`Send` callback
//! must not run or drop that callback on another thread.
//!
//! WHY: `std::task::Waker` is `Send` and `Sync` by type. In the non-`multi`
//! build the `WakeCallback` has no `Send` bound, so it may capture an `Rc`.
//! The raw waker data then held a value that must not cross a thread. Safe code
//! could reach that value from another thread: the split observer is `Send`
//! when its item types are `Send`, and its `QueueReadiness` can be pushed from
//! any thread.
//!
//! HOW: register a callback that captures an `Rc<Cell<_>>`, then move the
//! readiness to a second thread and push there. The wake must not run the
//! callback. A second test drops the last holder of the queue on a second
//! thread; the callback's captured value must not be dropped there.

#![cfg(not(feature = "multi"))]

use std::cell::Cell;
use std::rc::Rc;

use foundation_core::valtron::{
    CollectionState, Ctx, Stream, TaskIterator, TaskIteratorExt, TaskStatus, WakeResult,
    WakeableStream,
};

/// A task that yields its items once.
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

/// Counts its own drop. The counter is `Rc`, so this value is not `Send`.
struct DropCount(Rc<Cell<usize>>);

impl Drop for DropCount {
    fn drop(&mut self) {
        let count = &self.0;
        count.set(count.get() + 1);
    }
}

/// A wake from another thread must not run a callback that may capture
/// non-`Send` data.
#[test]
fn a_cross_thread_wake_does_not_run_a_non_send_callback() {
    let task = TestTask {
        items: vec![TaskStatus::Ready(7)],
    };
    let (mut observer, _continuation) =
        task.split_collect_until_map(|value| (CollectionState::Close(true), Some(*value)), 4);
    assert_eq!(observer.next(), Some(Stream::Wait));

    let fired = Rc::new(Cell::new(0));
    let counter = Rc::clone(&fired);
    assert_eq!(
        observer.wake(Box::new(move || counter.set(counter.get() + 1))),
        WakeResult::Successful,
        "the observer must accept the non-`Send` wake callback"
    );

    // The readiness over the observer's queue is `Send`, so safe code can push
    // from another thread. That push notifies the registered waker.
    let readiness = observer.readiness();
    std::thread::spawn(move || {
        readiness
            .push(Stream::Next(9))
            .expect("the observer queue accepts the item");
    })
    .join()
    .expect("the producer thread");

    assert_eq!(
        fired.get(),
        0,
        "a wake on another thread must not run a callback that may capture non-`Send` data (issue #423)"
    );

    // The push still delivered its item and the observer still works.
    assert_eq!(observer.next(), Some(Stream::Next(9)));
}

/// Dropping the queue on another thread must not drop a callback that may hold
/// non-`Send` data there.
#[test]
fn a_cross_thread_drop_keeps_a_non_send_callback_away_from_that_thread() {
    let task = TestTask {
        items: vec![TaskStatus::Ready(7)],
    };
    let (mut observer, continuation) =
        task.split_collect_until_map(|value| (CollectionState::Close(true), Some(*value)), 4);
    assert_eq!(observer.next(), Some(Stream::Wait));
    // The continuation closes the queue when it drops. Do that before the
    // callback exists, so the close is not mistaken for the drop under test.
    drop(continuation);

    let dropped = Rc::new(Cell::new(0));
    let flag = DropCount(Rc::clone(&dropped));
    assert_eq!(
        observer.wake(Box::new(move || {
            // The closure captures `flag`; it never reads it.
            let _ = &flag;
        })),
        WakeResult::Successful
    );

    // The observer is now the last holder of the queue. Drop it on another
    // thread: the notifier and its wakers go with it.
    std::thread::spawn(move || drop(observer))
        .join()
        .expect("the dropping thread");

    assert_eq!(
        dropped.get(),
        0,
        "another thread must not drop the value a non-`Send` callback captures; \
         the callback leaks instead (issue #423)"
    );
}

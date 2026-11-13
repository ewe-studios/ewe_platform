//! Regression test for issue #408: a task that returns `TaskStatus::Ignore`
//! with no progress must not be pushed back to the front of the worker queue.
//!
//! WHY: a self-waking task that answers `Ignore` was requeued to the front, so
//! the worker polled it again before any other runnable task. One such task
//! could starve every sibling on its worker, and on the single-threaded
//! executor it blocked the producer that had to push its value.
//!
//! HOW: schedule a task that never progresses next to a finite sibling. After a
//! bounded number of ticks the sibling must have run to completion. The old
//! code re-polled the first task on every tick, so the sibling never ran.

#![cfg(not(target_family = "wasm"))]
#![cfg_attr(
    not(feature = "multi"),
    expect(
        clippy::arc_with_non_send_sync,
        reason = "the local executor's task queue never crosses a thread in the single-threaded tree"
    )
)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use concurrent_queue::ConcurrentQueue;
use foundation_compact::rng::Rng;
use foundation_core::retries::ExponentialBackoffDecider;
use foundation_core::synca::*;
use foundation_core::valtron::*;

#[cfg(not(feature = "multi"))]
use foundation_core::valtron::any_task as any_task_for_mode;
#[cfg(feature = "multi")]
use foundation_core::valtron::send_any_task as any_task_for_mode;

/// A process controller that never sleeps, so the test controls every tick.
#[derive(Clone, Default)]
struct NoYielder;

impl ProcessController for NoYielder {
    type YieldSignal = ();

    fn yield_for(&self, _: Duration) {}
}

fn executor() -> LocalThreadExecutor<NoYielder> {
    let seed = foundation_compact::rng::rng().next_u64();
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));
    LocalThreadExecutor::from_seed(
        seed,
        "1".into(),
        global,
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Bottom,
        NoYielder,
        Duration::from_secs(10),
        Some(Arc::new(OnSignal::new())),
        None,
    )
}

/// A self-waking task: it counts its polls and never makes progress.
struct AlwaysIgnore {
    polls: Arc<AtomicUsize>,
}

impl TaskIterator for AlwaysIgnore {
    type Ready = usize;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        Some(TaskStatus::Ignore)
    }
}

/// A finite task that records each item it delivers.
struct RecordItems {
    items: std::vec::IntoIter<usize>,
    seen: Arc<Mutex<Vec<usize>>>,
}

impl TaskIterator for RecordItems {
    type Ready = usize;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let item = self.items.next()?;
        self.seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(item);
        Some(TaskStatus::Ready(item))
    }
}

#[test]
fn a_no_progress_task_does_not_starve_a_scheduled_sibling() {
    let executor = executor();
    let polls = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));

    // Both tasks go to the back of the local queue, parent first.
    any_task_for_mode(executor.boxed_engine())
        .with_task(AlwaysIgnore {
            polls: Arc::clone(&polls),
        })
        .as_scheduled()
        .spawn()
        .expect("the no-progress task must be scheduled");
    any_task_for_mode(executor.boxed_engine())
        .with_task(RecordItems {
            items: vec![1, 2, 3].into_iter(),
            seen: Arc::clone(&seen),
        })
        .as_scheduled()
        .spawn()
        .expect("the sibling task must be scheduled");

    // Enough ticks for the sibling to run to completion even when the
    // no-progress task takes one turn between each of its items.
    for _ in 0..10 {
        executor.run_once();
    }

    assert_eq!(
        *seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        vec![1, 2, 3],
        "the scheduled sibling must run while the no-progress task is still alive"
    );
    assert!(
        polls.load(Ordering::SeqCst) >= 1,
        "the executor must have polled the no-progress task"
    );
}

#![cfg(feature = "test-clock")]

//! The engine's task timers read the clock from the `Ctx` (spec 70 feature 132).
//!
//! WHY: `TimeoutTask` and `SleepingTask` used the real monotonic clock, so a
//! deterministic simulation test could not move a timeout or a sleep.
//!
//! WHAT: a task polled with a `Ctx` that carries a `SimulatedClock` times out or
//! wakes exactly when the simulated clock passes its deadline.
//!
//! HOW: `SimulatedClock::advance` moves the clock by hand; no test waits on the
//! real clock. Before this feature the two checks read the real clock, so the
//! simulated advance had no effect and the assertions failed.

use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{
    Ctx, SimulatedClock, SleepingTask, TaskIterator, TaskStatus, TimeoutTask,
};

/// A task that never completes, so only the wrapper's timeout can end it.
struct PendingForever;

impl TaskIterator for PendingForever {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<(), ()>> {
        Some(TaskStatus::Pending(()))
    }
}

/// The `Ctx` for a clock that starts at the UNIX epoch.
fn ctx_with(clock: &Arc<SimulatedClock>) -> Ctx {
    let clock: Arc<dyn foundation_core::valtron::Clock> = clock.clone();
    Ctx::fixed(clock)
}

#[test]
fn the_timeout_wrapper_fires_on_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = ctx_with(&clock);
    let mut task = TimeoutTask::new(PendingForever, Duration::from_secs(1));

    assert!(
        matches!(task.next_status(ctx.clone()), Some(TaskStatus::Pending(()))),
        "the first poll arms the timeout and passes the inner task through"
    );

    clock.advance(Duration::from_secs(2));

    assert_eq!(
        task.next_status(ctx),
        None,
        "advancing the injected clock past the timeout must end the task"
    );
}

#[test]
fn a_duration_sleep_wakes_on_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = ctx_with(&clock);
    let mut task = SleepingTask::sleep(Duration::from_secs(1));

    match task.next_status(ctx.clone()) {
        Some(TaskStatus::Delayed(remaining)) => assert_eq!(remaining, Duration::from_secs(1)),
        other => panic!("a fresh one-second sleep must report one second left, got {other:?}"),
    }

    clock.advance(Duration::from_secs(2));

    assert_eq!(
        task.next_status(ctx),
        None,
        "advancing the injected clock past the sleep must wake the task"
    );
}

#![cfg(feature = "test-clock")]

//! The engine wakes at the nearest sleeper deadline (GitHub issue #1115).
//!
//! WHY: the engine chose its wait from the duration of the task that had just
//! parked. With two sleepers, the wait was the longer duration. Both sleepers
//! were due at that late wake, and the sleeper map order decided which one ran
//! first. A 10 ms timer could finish after a 30 ms timer.
//!
//! WHAT: two sleeping tasks park with a 10 ms and a 30 ms deadline on a
//! simulated clock.
//!
//! HOW: after both park, the engine asks for a spin wait. The wait must not
//! exceed the nearest deadline (10 ms). The old code asked for 30 ms. The test
//! moves the clock by hand, so no real time passes and the run repeats.

use std::sync::Arc;
use std::time::Duration;

use concurrent_queue::ConcurrentQueue;
use foundation_context::Ctx;
use foundation_core::{
    panic_if_failed,
    retries::ExponentialBackoffDecider,
    synca::{IdleMan, SleepyMan},
    valtron::{
        LocalThreadExecutor, OnNext, PriorityOrder, ProcessController, ProgressIndicator,
        SharedTaskQueue, SimulatedClock, SleepingTask,
    },
};

/// Fixed seed for the engine, so each run replays exactly.
const SEED: u64 = 0x5EED_5EED_0000_1115;

/// A controller that never blocks: the test drives `run_once` by hand.
#[derive(Default, Clone)]
struct NoYielder;

impl ProcessController for NoYielder {
    type YieldSignal = ();

    fn yield_for(&self, _: Duration) {}
}

/// A root context on the simulated clock.
fn ctx_with(clock: &Arc<SimulatedClock>) -> Ctx {
    let clock: Arc<dyn foundation_context::Clock> = clock.clone();
    Ctx::fixed(clock)
}

#[test]
fn the_engine_waits_for_the_nearest_sleeper_deadline() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let global: SharedTaskQueue = Arc::new(ConcurrentQueue::bounded(10));

    let executor = LocalThreadExecutor::from_seed_with_root(
        SEED,
        "deadline-order".into(),
        global.clone(),
        IdleMan::new(
            3,
            None,
            SleepyMan::new(3, ExponentialBackoffDecider::default()),
        ),
        PriorityOrder::Top,
        NoYielder,
        Duration::from_secs(10),
        None,
        None,
        ctx_with(&clock),
    );

    for ms in [10_u64, 30] {
        panic_if_failed!(global.push(Box::new(OnNext::on_next(
            SleepingTask::sleep(Duration::from_millis(ms)),
            |_, _| {},
        ))));
    }

    // The first tick parks the 10 ms task.
    assert!(
        matches!(executor.run_once(), ProgressIndicator::CanProgress(_)),
        "the first tick must take one of the queued tasks"
    );

    // The second tick parks the 30 ms task. Both sleepers are registered now.
    // The wait must be the 10 ms deadline, not the 30 ms task's own duration.
    assert_eq!(
        executor.run_once(),
        ProgressIndicator::SpinWait(Duration::from_millis(10)),
        "the engine must wait for the nearest sleeper deadline"
    );
}

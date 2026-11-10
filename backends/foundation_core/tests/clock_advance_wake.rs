//! A move of the injected clock wakes the executors parked on it (issue #305).
//!
//! WHY: `SimulatedClock` moves only when a test moves it. A task that waits for
//! a simulated deadline parks its executor thread with an operating system wait.
//! Before this fix, `advance` changed the offset but woke nobody, so the parked
//! task learned about the new time only at the next timed re-check, and a
//! sleeper that the move put exactly on its deadline never matured at all. A
//! simulated clock therefore could not drive a deadline with the precision that
//! a deterministic simulation test needs.
//!
//! WHAT: a task parks for one simulated hour. Another thread advances the clock
//! by one hour. The task must run again at once, with the simulated time at
//! least one hour after it parked, and the executor of the task must receive a
//! wake.
//!
//! HOW: `SimulatedClock::advance` asks the executor host to run again, so the
//! wait the task parked on is interrupted. The task reports the executor's wake
//! count at the park and at the resume (see `executor_wake_count`). This file is
//! its own test target, so no other test can wake the executor in the window
//! that the count covers.

#![cfg(all(feature = "test-clock", not(target_family = "wasm")))]

use std::sync::{Arc, LazyLock};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_core::valtron::{
    execute, executor_wake_count, valtron_test, Clock, Ctx, SimulatedClock, StreamIteratorExt,
    TaskIterator, TaskStatus,
};

/// The simulated time the task waits for.
const ONE_HOUR: Duration = Duration::from_secs(3600);

/// The real time the mover gives the engine to park the task before it moves
/// the clock.
const PARK_DELAY: Duration = Duration::from_millis(300);

/// The most real time the whole run may take. A resumed task does not take this
/// long; a task that waits out the simulated hour on the real clock takes an
/// hour, and the macro timeout ends the test first.
const REAL_LIMIT: Duration = Duration::from_secs(5);

/// The pool clock. It starts at the real time of the process and moves only
/// through `advance`.
static CLOCK: LazyLock<Arc<SimulatedClock>> =
    LazyLock::new(|| Arc::new(SimulatedClock::new(Duration::ZERO)));

/// The pool root with the simulated clock.
fn ctx_with(clock: &Arc<SimulatedClock>) -> Ctx {
    let clock: Arc<dyn Clock> = clock.clone();
    Ctx::fixed(clock)
}

/// Parks for one simulated hour, then reports:
/// - the simulated time at the park;
/// - the simulated time at the resume;
/// - the executor's wake count at the park;
/// - the executor's wake count at the resume.
struct SleepsOneHour {
    step: u8,
    parked_at: Option<Instant>,
    wakes_at_park: u64,
}

impl TaskIterator for SleepsOneHour {
    type Ready = (Instant, Instant, u64, u64);
    type Pending = ();

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        match self.step {
            0 => {
                self.step = 1;
                self.parked_at = Some(ctx.clock().now());
                self.wakes_at_park = executor_wake_count();
                Some(TaskStatus::Delayed(ONE_HOUR))
            }
            1 => {
                self.step = 2;
                let parked_at = self.parked_at.expect("the task parked before it resumed");
                Some(TaskStatus::Ready((
                    parked_at,
                    ctx.clock().now(),
                    self.wakes_at_park,
                    executor_wake_count(),
                )))
            }
            _ => None,
        }
    }
}

#[valtron_test(timeout = 10_000, ctx = ctx_with(&CLOCK))]
async fn advancing_the_injected_clock_wakes_the_parked_executor() {
    // The task cannot signal from inside `next_status` without racing the
    // engine's own deadline setup, so the mover gives the engine a fixed head
    // start before it moves the clock, as the other park tests do.
    let clock = Arc::clone(&CLOCK);
    let mover = std::thread::spawn(move || {
        std::thread::sleep(PARK_DELAY);
        clock.advance(ONE_HOUR);
    });

    let started = std::time::Instant::now();
    let items = execute(
        SleepsOneHour {
            step: 0,
            parked_at: None,
            wakes_at_park: 0,
        },
        None,
    )
    .expect("execute")
    .into_collect_future()
    .await;
    let elapsed = started.elapsed();
    mover.join().expect("the clock mover thread");

    let (parked_at, resumed_at, wakes_at_park, wakes_at_resume) = items
        .into_iter()
        .next()
        .expect("the task yields one report");

    assert!(
        resumed_at.saturating_duration_since(parked_at) >= ONE_HOUR,
        "the task resumed at {resumed_at:?}, less than one simulated hour after it \
         parked at {parked_at:?}"
    );
    assert!(
        wakes_at_resume > wakes_at_park,
        "the clock move did not wake the executor that hosts the task: the wake \
         count stayed at {wakes_at_park}"
    );
    assert!(
        elapsed < REAL_LIMIT,
        "the task needed {elapsed:?} of real time; the bound is {REAL_LIMIT:?}"
    );
}

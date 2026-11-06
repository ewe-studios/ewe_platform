//! Tests for [`Clock::wall_now`].
//!
//! WHY: the `Clock` trait was monotonic only, so module code that needed a
//! calendar timestamp could not read it from the injected clock. Feature 117
//! adds `wall_now`, and a simulated clock that never reads the real wall clock.
//!
//! WHAT: [`MonotonicClock::wall_now`] reads the real time; [`SimulatedClock`]
//! starts at the UNIX epoch and moves only through `advance_wall`.
//!
//! HOW: one test compares the real clock with `SystemTime::now` (the single real
//! read in this file). The simulated tests advance the clock by hand and run a
//! task through the engine.

use foundation_compact::SystemTime;
use foundation_core::valtron::{Clock, MonotonicClock};

#[cfg(feature = "test-clock")]
use foundation_compact::Duration;

#[cfg(feature = "test-clock")]
use std::sync::{Arc, LazyLock};

#[cfg(feature = "test-clock")]
use foundation_core::valtron::{fork_ctx, valtron_test, Ctx, SimulatedClock};

/// The wall clock for the engine test: five seconds after the UNIX epoch.
#[cfg(feature = "test-clock")]
static WALL_CLOCK: LazyLock<Arc<SimulatedClock>> =
    LazyLock::new(|| Arc::new(SimulatedClock::new(Duration::ZERO)));

/// Build a pool root with `clock` as its engine clock.
#[cfg(feature = "test-clock")]
fn ctx_with(clock: &Arc<SimulatedClock>) -> Ctx {
    let clock: Arc<dyn Clock> = clock.clone();
    Ctx::new(clock)
}

#[test]
fn monotonic_clock_wall_now_is_near_real_time() {
    let clock = MonotonicClock::new();
    let start = SystemTime::now();
    let wall = clock.wall_now();
    let end = SystemTime::now();

    assert!(
        wall >= start && wall <= end,
        "the real clock's wall time must fall between the two reads around it"
    );
}

#[cfg(feature = "test-clock")]
#[test]
fn simulated_wall_time_moves_only_when_advanced() {
    let clock = SimulatedClock::new(Duration::from_secs(3_600));
    let start = clock.wall_now();

    assert_eq!(
        clock.wall_now(),
        start,
        "the wall clock must not move without advance_wall"
    );

    clock.advance(Duration::from_secs(60));
    assert_eq!(
        clock.wall_now(),
        start,
        "advancing the monotonic clock must not move the wall clock"
    );

    clock.advance_wall(Duration::from_millis(1_234));
    assert_eq!(clock.wall_now(), start + Duration::from_millis(1_234));
}

#[cfg(feature = "test-clock")]
#[valtron_test(ctx = ctx_with(&WALL_CLOCK))]
fn the_task_ctx_reports_the_simulated_wall_time() {
    WALL_CLOCK.advance_wall(Duration::from_secs(5));

    let ctx = fork_ctx();
    assert_eq!(
        ctx.clock().wall_now(),
        WALL_CLOCK.wall_now(),
        "a task-side fork must read the simulated wall clock"
    );
}

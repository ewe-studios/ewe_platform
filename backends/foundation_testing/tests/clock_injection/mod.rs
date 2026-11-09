//! A stress run measures its duration on the injected clock (spec 70
//! feature 134).
//!
//! WHY: `StressHarness::run` measured with the real monotonic clock, so a
//! deterministic test could not state its duration.
//!
//! WHAT: with a simulated clock that never moves, the reported duration is
//! exactly zero, although the workers run.
//!
//! HOW: before this feature the run read the real clock, so the reported
//! duration was the real run time and the zero assertion failed.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::SimulatedClock;
use foundation_testing::stress::{StressConfig, StressHarness};

#[test]
fn the_stress_duration_comes_from_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let counter = Arc::new(AtomicUsize::new(0));
    let config = StressConfig::new().threads(2).iterations(10);
    let harness = StressHarness::new(config, clock);

    let shared_counter = Arc::clone(&counter);
    let result = harness.run(move |_thread_id, _iteration| {
        shared_counter.fetch_add(1, Ordering::Relaxed);
        true
    });

    assert_eq!(result.successes, 20, "the workers must still run");
    assert_eq!(
        result.duration,
        Duration::ZERO,
        "the simulated clock never moved, so the measured duration must be zero"
    );
}

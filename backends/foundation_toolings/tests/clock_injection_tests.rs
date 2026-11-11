//! Spec 70 feature 134 — the runner reload wait reads the injected clock.
//!
//! The old `DurationSleeper` compared the real wall clock in epoch
//! milliseconds. It now stores a deadline from `Clock::now`, so a simulated
//! clock ends the wait with no real time passing.

use std::sync::Arc;
use std::time::Duration;

use foundation_context::{Clock, Ctx};
use foundation_core::valtron::SimulatedClock;
use foundation_toolings::runner::DurationSleeper;

#[test]
fn duration_sleeper_follows_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let shared: Arc<dyn Clock> = Arc::clone(&clock) as Arc<dyn Clock>;
    let mut sleeper = DurationSleeper::new(Ctx::fixed(shared));

    assert!(!sleeper.is_done(), "an unstarted sleeper is not done");

    sleeper.start(Duration::from_secs(5));
    assert!(
        !sleeper.is_done(),
        "the deadline is five simulated seconds away"
    );

    clock.advance(Duration::from_secs(4));
    assert!(!sleeper.is_done(), "four simulated seconds are not enough");

    clock.advance(Duration::from_secs(1));
    assert!(sleeper.is_done(), "the injected clock passed the deadline");
}

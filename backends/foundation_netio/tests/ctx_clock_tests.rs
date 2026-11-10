//! A deadline built from a `Ctx` over a simulated clock expires when the test
//! moves that clock. The deadline reads the time only through the `Ctx`.

use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::SimulatedClock;

use foundation_context::Ctx;
use foundation_netio::native::readiness::DeadlineReadiness;

#[test]
fn a_deadline_on_a_ctx_expires_when_the_simulated_clock_advances() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(clock.clone());

    let deadline = DeadlineReadiness::after(Duration::from_secs(5), ctx);
    assert!(!deadline.expired(), "the deadline is in the future");

    clock.advance(Duration::from_secs(4));
    assert!(!deadline.expired(), "four seconds of five have passed");

    clock.advance(Duration::from_secs(1));
    assert!(
        deadline.expired(),
        "the simulated clock reached the deadline"
    );
}

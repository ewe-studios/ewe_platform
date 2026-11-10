//! The harness deadlines follow the injected `Clock` (spec 70 feature 132).
//!
//! WHY: the Docker test environment, the browser runner and the wrangler runner
//! read the real monotonic clock directly, so a deterministic simulation test
//! could not control when they give up waiting.
//!
//! WHAT: the builder computes its readiness deadline from the clock it is given.
//! A simulated clock moves the deadline by exactly the time the test advances.
//!
//! HOW: `SimulatedClock` (the `test-clock` feature) advances by hand; no test
//! waits on the real clock.

#![cfg(feature = "docker-tests")]

use std::sync::Arc;
use std::time::Duration;

use foundation_context::{Clock, Ctx};
use foundation_core::valtron::SimulatedClock;
use foundation_testbed::docker::TestEnvironmentBuilder;

#[test]
fn the_ready_deadline_follows_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::from_secs(10)));
    let injected: Arc<dyn Clock> = clock.clone();
    let builder = TestEnvironmentBuilder::new("container", Ctx::fixed(injected.clone()))
        .with_ready_timeout(Duration::from_secs(30));

    let first = builder.deadline();
    assert_eq!(
        first,
        clock.now() + Duration::from_secs(30),
        "the deadline must start from the injected clock"
    );

    clock.advance(Duration::from_secs(5));

    assert_eq!(
        builder.deadline(),
        first + Duration::from_secs(5),
        "advancing the injected clock must move the deadline by the same amount"
    );
    assert!(
        Arc::ptr_eq(&builder.clock(), &injected),
        "the builder must keep the clock it was given"
    );
}

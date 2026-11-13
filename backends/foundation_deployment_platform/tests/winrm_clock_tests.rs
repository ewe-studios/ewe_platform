//! Clock injection test for the `WinRM` client.
//!
//! WHY: spec 70 feature 134 removes every direct clock read from the platform.
//! A `WinRM` client reads its time only through the `Ctx` it was built with, so
//! a deterministic simulation test (DST) controls every time it sees.
//!
//! WHAT: a `WinRM` client is built from `Ctx::fixed` around a `SimulatedClock`.
//! The test asserts that the client's clock reads the simulated time, and that
//! the reading moves only when the test advances the simulated clock.
//!
//! HOW: no network connection and no real clock read. The client is only
//! constructed, never used to send a request.

#![cfg(feature = "vms")]

use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;
use foundation_deployment_platform::winrm::WinRM;

#[test]
fn winrm_client_clock_follows_the_injected_ctx() {
    let simulated = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(simulated.clone());
    let client = WinRM::new("192.0.2.10", 5985, "user", "pass", ctx);

    let before = client.clock().now();
    simulated.advance(Duration::from_secs(90));
    let after = client.clock().now();

    assert_eq!(
        after.duration_since(before),
        Duration::from_secs(90),
        "the client clock must move with the simulated clock, not the process clock"
    );
    assert_eq!(
        client.clock().elapsed(),
        Duration::from_secs(90),
        "the client clock elapsed time must be the simulated elapsed time"
    );
}

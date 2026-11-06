//! The SSH connection pool measures idle age on the injected clock
//! (spec 70 feature 134).
//!
//! WHY: the pool stamped a session and evicted it with the real monotonic
//! clock, so a deterministic test could not control the idle window.
//!
//! WHAT: the pool carries the `SimulatedClock` the caller built, and its idle
//! check reads that clock.
//!
//! HOW: before this feature `ConnectionPool::new` had no clock and
//! `evict_idle` used `Instant::elapsed`, so this test could not build the pool
//! with a simulated clock.

use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{Clock, SimulatedClock};
use foundation_sshkit::ConnectionPool;

#[test]
fn the_pool_measures_idle_age_on_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let pool = ConnectionPool::new(Duration::from_secs(30), clock.clone());

    assert_eq!(
        pool.clock().now(),
        clock.now(),
        "the pool must read the clock its caller injected"
    );

    clock.advance(Duration::from_secs(31));
    assert_eq!(
        pool.clock().now(),
        clock.now(),
        "advancing the injected clock must move the pool's idle measurement"
    );

    // No sessions are open, so this must not panic; it is the path a real
    // session would take with the advanced clock.
    pool.evict_idle();
}

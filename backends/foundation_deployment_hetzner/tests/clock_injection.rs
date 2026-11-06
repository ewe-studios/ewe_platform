//! A Hetzner client measures rate-limit resets and poll deadlines on the
//! injected clock (spec 70 feature 134).
//!
//! WHY: `HetznerClient` read the real wall clock for `await_running` and for a
//! `RateLimit-Reset` header, so a deterministic test could not control either.
//!
//! WHAT: the client carries the `SimulatedClock` the caller built.
//!
//! HOW: before this feature the client had no clock, so this test could not
//! build a client with one.

use std::sync::Arc;
use std::time::Duration;

use foundation_compact::time::unix_seconds;
use foundation_core::valtron::SimulatedClock;
use foundation_deployment_hetzner::HetznerClient;

/// 2026-01-01T00:00:00Z in unix seconds.
const SIMULATED_SECS: u64 = 1_767_225_600;

#[test]
fn the_client_keeps_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_secs(SIMULATED_SECS));

    let client = HetznerClient::new("token", clock.clone());

    assert_eq!(
        unix_seconds(client.clock().wall_now()),
        i64::try_from(SIMULATED_SECS).unwrap(),
        "the client must keep the clock that measures its waits"
    );
}

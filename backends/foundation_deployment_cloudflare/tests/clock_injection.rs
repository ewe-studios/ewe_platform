//! A Cloudflare client hands its injected clock to the records it stamps
//! (spec 70 feature 134).
//!
//! WHY: `bootstrap_domain` stamped the DNS record with `Utc::now()`, so a
//! deterministic test could not control the record time.
//!
//! WHAT: the client carries the `SimulatedClock` the caller built.
//!
//! HOW: before this feature the client had no clock, so this test could not
//! build a client with one.

#![cfg(not(target_family = "wasm"))]

use std::sync::Arc;
use std::time::Duration;

use foundation_compact::time::unix_millis;
use foundation_core::valtron::{Clock, SimulatedClock};
use foundation_deployment_cloudflare::client::CloudflareClient;

/// 2026-01-01T00:00:00Z in unix seconds.
const SIMULATED_SECS: u64 = 1_767_225_600;

#[test]
fn the_client_keeps_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_secs(SIMULATED_SECS));
    let expected = unix_millis(clock.wall_now());

    let client = CloudflareClient::with_client(
        foundation_netio::HttpClientBuilder::new().build(),
        "token".to_string(),
        "zone".to_string(),
        clock,
    );

    assert_eq!(
        unix_millis(client.clock().wall_now()),
        expected,
        "the client must keep the clock that stamps its DNS records"
    );
}

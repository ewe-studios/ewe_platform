//! Deployment record times come from the injected clock (spec 70 feature 134).
//!
//! WHY: the deployment output constructors read the real wall clock, so a
//! deterministic simulation test could not control a recorded `deployed_at`.
//!
//! WHAT: a fixed simulated wall time stamps every deployment record and the
//! `ProviderClient` carries the same clock to the state it records.
//!
//! HOW: `SimulatedClock::advance_wall` moves the clock by hand. Before this
//! feature the constructors called `Utc::now()`, so the stored value was the
//! real time and the equality below failed.

use std::sync::Arc;
use std::time::Duration;

use foundation_compact::time::unix_millis;
use foundation_core::valtron::{Clock, SimulatedClock};
use foundation_db::core::state::FileStateStore;
use foundation_deployment::core::types::DeploymentResult;
use foundation_deployment::provider_client::ProviderClient;
use foundation_deployment::types::{DeploymentOutput, WorkerDeployment};

/// 2026-01-01T00:00:00Z in unix seconds.
const SIMULATED_SECS: u64 = 1_767_225_600;

#[test]
fn deployment_record_times_follow_the_injected_wall_clock() {
    let clock = SimulatedClock::new(Duration::ZERO);
    clock.advance_wall(Duration::from_secs(SIMULATED_SECS));
    let expected = unix_millis(clock.wall_now());
    assert_eq!(expected, i64::try_from(SIMULATED_SECS * 1_000).unwrap());

    let output = DeploymentOutput::new("id", "name", &clock);
    assert_eq!(output.deployed_at.timestamp_millis(), expected);

    let worker = WorkerDeployment::new("account", "script", "deployment", "url", &clock);
    assert_eq!(worker.deployed_at.timestamp_millis(), expected);

    let dry_run = DeploymentResult::dry_run("provider", "resource", &clock);
    assert_eq!(dry_run.deployed_at.timestamp_millis(), expected);

    let skipped = DeploymentResult::skipped("unchanged", &clock);
    assert_eq!(skipped.deployed_at.timestamp_millis(), expected);
}

#[test]
fn provider_client_carries_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_secs(SIMULATED_SECS));
    let store = FileStateStore::new(&std::env::temp_dir(), "clock-test", "dev");

    let client = ProviderClient::new("clock-test", "dev", store, clock.clone());

    assert_eq!(
        unix_millis(client.clock().wall_now()),
        unix_millis(clock.wall_now()),
        "the provider client must hand its injected clock to recorded state"
    );
}

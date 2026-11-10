//! Clock injection tests for the VM lifecycle and deployment record times.
//!
//! WHY: spec 70 feature 134 removes every direct clock read from the platform.
//! A VM state file and an export manifest each record a wall time; both must
//! come from the injected `Clock`, so a deterministic simulation test (DST)
//! controls them.
//!
//! WHAT: `state::from_runtime` and `ExportManifest::new` are given a
//! `SimulatedClock`. The test advances its wall clock and asserts the stored
//! timestamps move with it, not with the process clock.
//!
//! HOW: no real wait and no real clock read. The simulated wall clock starts at
//! the UNIX epoch and moves only through `SimulatedClock::advance_wall`.

#![cfg(feature = "vms")]

use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;
use foundation_deployment_platform::config::get_profile;
use foundation_deployment_platform::export::ExportManifest;
use foundation_deployment_platform::providers::ProviderId;
use foundation_deployment_platform::state;

/// 2026-01-01T00:00:00Z in whole seconds.
const START_SECONDS: u64 = 1_767_225_600;

#[test]
fn vm_state_created_at_follows_the_injected_wall_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(clock.clone());
    clock.advance_wall(Duration::from_secs(START_SECONDS));

    let state = state::from_runtime(
        "clock-test",
        std::path::Path::new("/tmp/clock-test.qcow2"),
        ProviderId::Qemu,
        "1234",
        2222,
        None,
        None,
        5900,
        false,
        &ctx,
    );
    assert!(
        state.created_at.starts_with("2026-01-01T00:00:00"),
        "stored time must be the simulated wall time, got {:?}",
        state.created_at
    );

    clock.advance_wall(Duration::from_secs(86_400));
    let later = state::from_runtime(
        "clock-test",
        std::path::Path::new("/tmp/clock-test.qcow2"),
        ProviderId::Qemu,
        "1234",
        2222,
        None,
        None,
        5900,
        false,
        &ctx,
    );
    assert!(
        later.created_at.starts_with("2026-01-02T00:00:00"),
        "stored time must follow the advanced clock, got {:?}",
        later.created_at
    );
}

#[test]
fn export_manifest_created_at_follows_the_injected_wall_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(clock.clone());
    clock.advance_wall(Duration::from_secs(START_SECONDS));

    let profile = get_profile("linux-test").expect("linux-test profile must exist");
    let image_path =
        std::env::temp_dir().join(format!("clock-injection-{}.qcow2", std::process::id()));
    std::fs::write(&image_path, b"clock injection image").expect("write temporary image");

    let manifest = ExportManifest::new(
        profile,
        &image_path,
        "test",
        std::collections::HashMap::new(),
        0,
        "clock injection",
        &ctx,
    )
    .expect("manifest must build from a readable image");
    assert_eq!(
        manifest.created_at, "2026-01-01T00:00:00Z",
        "manifest time must be the simulated wall time"
    );

    clock.advance_wall(Duration::from_secs(86_400));
    let later = ExportManifest::new(
        profile,
        &image_path,
        "test",
        std::collections::HashMap::new(),
        0,
        "clock injection",
        &ctx,
    )
    .expect("manifest must build from a readable image");
    assert_eq!(
        later.created_at, "2026-01-02T00:00:00Z",
        "manifest time must follow the advanced clock"
    );

    let _ = std::fs::remove_file(&image_path);
}

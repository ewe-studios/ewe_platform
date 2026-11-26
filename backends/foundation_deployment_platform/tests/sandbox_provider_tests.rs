//! The `Provider` facade of spec 77 feature 06.
//!
//! The facade lives in `foundation_sandbox::providers`; the trait stays in
//! `foundation_deployment_platform`. These tests use the simulation driver and
//! the scripted slave of feature 02, so no microVM runs.

#![cfg(feature = "sandbox")]

use std::sync::{Arc, OnceLock};

use foundation_context::Clock;
use foundation_context::Ctx;
use foundation_core::env;
use foundation_core::valtron::SimulatedClock;
use foundation_deployment_platform::providers::{Provider, ProviderId};
use foundation_sandbox::providers::SandboxDeploymentProvider;
use foundation_sandbox::shared::control::testing::{
    ScriptedSlave, SimDriver, full_capabilities, in_memory_connector, spec,
};
use foundation_sandbox::shared::control::{
    ManagerConfig, MemoryRecordStore, SandboxManager, SandboxState,
};
use foundation_sandbox::shared::driver::{Capabilities, HostCheck, HostReport, SandboxDriver};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::ids::DriverId;

/// Starts the valtron pool one time for the test binary.
fn pool() {
    static POOL: OnceLock<()> = OnceLock::new();
    POOL.get_or_init(|| {
        std::mem::forget(foundation_core::valtron::initialize_pool(11, Some(8)));
    });
}

/// A context with a simulated clock and environment.
fn ctx(seed: u64) -> Ctx {
    pool();
    let clock = Arc::new(SimulatedClock::new(std::time::Duration::ZERO));
    let ctx = Ctx::with_seed(Arc::clone(&clock) as Arc<dyn Clock>, seed);
    let env = env::simulated(&ctx, seed).expect("the simulated Env builds");
    ctx.store(env);
    ctx
}

/// A manager over one simulation driver and a scripted slave.
fn manager(ctx: &Ctx, driver: SimDriver, slave: Arc<ScriptedSlave>) -> Arc<SandboxManager> {
    Arc::new(SandboxManager::new(
        ctx.clone(),
        vec![Arc::new(driver) as Arc<dyn SandboxDriver>],
        in_memory_connector(slave),
        Arc::new(MemoryRecordStore::default()),
        ManagerConfig::default(),
    ))
}

#[test]
fn provider_launch_stop_sim() {
    let ctx = ctx(120);
    let driver = SimDriver::new(DriverId::Firecracker);
    let slave = Arc::new(ScriptedSlave::new());
    // The stop reaches the shutdown grace, and the simulated clock does not
    // move. The driver's substrate exits at once when the slave gets `Shutdown`.
    slave.with_shutdown_exit(driver.substrate());
    let control = manager(&ctx, driver, slave);
    let provider = SandboxDeploymentProvider::firecracker(control);
    let handle = provider.launch(&spec()).expect("the launch succeeds");
    assert_eq!(handle.provider_id, ProviderId::Firecracker);
    assert!(
        provider.is_running(&handle),
        "invariant I8: a ready sandbox runs"
    );
    assert_eq!(handle.sandbox.info().state, SandboxState::Ready);
    provider.stop(&handle).expect("the stop succeeds");
    assert!(!provider.is_running(&handle));
}

#[test]
fn provider_error_is_typed() {
    let ctx = ctx(121);
    // The driver is usable, but it cannot use a directory root.
    let driver = SimDriver::with_report(HostReport {
        driver: DriverId::Firecracker,
        usable: true,
        checks: Vec::new(),
        capabilities: Capabilities::default(),
    });
    let control = manager(&ctx, driver, Arc::new(ScriptedSlave::new()));
    let provider = SandboxDeploymentProvider::firecracker(control);
    let Err(error) = provider.launch(&spec()) else {
        panic!("the launch must fail");
    };
    assert!(matches!(
        error.current_context(),
        SandboxError::UnsupportedRootFs
    ));
}

#[test]
fn provider_host_health() {
    let ctx = ctx(122);
    let driver = SimDriver::with_report(HostReport {
        driver: DriverId::Firecracker,
        usable: true,
        checks: vec![
            HostCheck {
                name: "kvm".to_string(),
                ok: true,
                message: "ok".to_string(),
            },
            HostCheck {
                name: "slave".to_string(),
                ok: false,
                message: "missing".to_string(),
            },
        ],
        capabilities: full_capabilities(),
    });
    let control = manager(&ctx, driver, Arc::new(ScriptedSlave::new()));
    let provider = SandboxDeploymentProvider::firecracker(control);
    assert_eq!(
        provider.host_health(),
        vec![
            ("kvm".to_string(), true, "ok".to_string()),
            ("slave".to_string(), false, "missing".to_string()),
        ]
    );
}

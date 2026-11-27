//! The nested driver (feature 11, section 7).
//!
//! The parent is a `SimDriver` sandbox whose slave also serves the control
//! plane. The control service of the parent is a real `ControlService` over a
//! real `SandboxManager`; the child runs in that inner manager. No socket and no
//! virtual machine is used.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::SimulatedClock;
use foundation_sandbox::shared::control::testing::{
    in_memory_connector, in_memory_dual_connector, CountingStore, ScriptedSlave, SimDriver,
    SimSubstrate,
};
use foundation_sandbox::shared::control::{
    ControlService, ManagerConfig, SandboxManager, SandboxProvider, SandboxState,
};
use foundation_sandbox::shared::driver::SubstrateExit;
use foundation_sandbox::shared::drivers::nested::{watch_child, NestedWatchConfig, WatchOutcome};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::ids::{DriverId, SandboxId};
use foundation_sandbox::shared::limits::SandboxLimits;
use foundation_sandbox::shared::spec::{RootFs, SandboxSpec};

use super::support;

/// The two control planes and the scripts behind them.
struct Setup {
    clock: Arc<SimulatedClock>,
    outer: SandboxManager,
    inner: Arc<SandboxManager>,
    parent_slave: Arc<ScriptedSlave>,
    parent_substrate: Arc<SimSubstrate>,
    inner_slave: Arc<ScriptedSlave>,
}

/// Builds a parent control plane and its inner control plane.
fn setup(seed: u64, can_control: bool, info_pending: bool) -> Setup {
    let (ctx, clock) = support::ctx(seed);
    setup_with(ctx, clock, can_control, info_pending)
}

/// Builds the two control planes over a caller's context and clock.
fn setup_with(
    ctx: Ctx,
    clock: Arc<SimulatedClock>,
    can_control: bool,
    info_pending: bool,
) -> Setup {
    // The inner control plane: a manager with the Firecracker driver.
    let inner_slave = Arc::new(ScriptedSlave::new());
    let inner_driver = Arc::new(SimDriver::new(DriverId::Firecracker));
    let inner_substrate = inner_driver.substrate();
    inner_slave.with_shutdown_exit(Arc::clone(&inner_substrate));
    let inner = Arc::new(SandboxManager::new(
        ctx.clone(),
        vec![inner_driver],
        in_memory_connector(Arc::clone(&inner_slave)),
        Arc::new(CountingStore::new()),
        ManagerConfig::default(),
    ));
    let control = Arc::new(ControlService::new(
        Arc::clone(&inner) as Arc<dyn SandboxProvider>,
        SandboxLimits::default(),
    ));

    // The parent slave serves both services on one transport.
    let parent_slave = Arc::new(ScriptedSlave::new());
    parent_slave.with_can_control(can_control);
    if info_pending {
        parent_slave.with_info_pending();
    }
    let connector = in_memory_dual_connector(ctx.clone(), Arc::clone(&parent_slave), control);

    // The outer control plane: the parent driver and the nested driver.
    let parent_driver = Arc::new(SimDriver::new(DriverId::Docker));
    let parent_substrate = parent_driver.substrate();
    parent_slave.with_shutdown_exit(Arc::clone(&parent_substrate));
    let outer = SandboxManager::with_nested_driver(
        ctx,
        vec![parent_driver],
        connector,
        Arc::new(CountingStore::new()),
        ManagerConfig::default(),
    );
    Setup {
        clock,
        outer,
        inner,
        parent_slave,
        parent_substrate,
        inner_slave,
    }
}

/// A parent sandbox that is a nested host.
fn parent_spec() -> SandboxSpec {
    let mut spec = SandboxSpec::new(RootFs::ContainerImage("ewe/nested:latest".to_string()));
    spec.driver = Some(DriverId::Docker);
    spec.nested_host = true;
    spec
}

/// A child of `parent`.
fn child_spec(parent: &SandboxId) -> SandboxSpec {
    let mut spec = SandboxSpec::new(RootFs::Ext4Image(PathBuf::from(
        "/var/cache/ewe-sandbox/rootfs.ext4",
    )));
    spec.driver = Some(DriverId::Nested);
    spec.parent = Some(parent.clone());
    spec
}

/// Creates one parent and gives its identifier.
fn create_parent(setup: &Setup) -> SandboxId {
    let handle = support::block_on(setup.outer.create(parent_spec())).expect("the parent starts");
    handle.id().clone()
}

/// Waits until the outer manager has no record.
fn wait_empty(outer: &SandboxManager) {
    for _ in 0..200 {
        if support::block_on(outer.list()).expect("list").is_empty() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the records did not leave the manager");
}

/// The automatic driver choice never picks the nested driver: a child names it.
#[test]
fn nested_driver_is_not_chosen_automatically() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(35);
    let manager = SandboxManager::with_nested_driver(
        ctx.clone(),
        Vec::new(),
        in_memory_connector(Arc::new(ScriptedSlave::new())),
        Arc::new(CountingStore::new()),
        ManagerConfig::default(),
    );
    let mut spec = SandboxSpec::new(RootFs::Ext4Image(PathBuf::from("/rootfs.ext4")));
    spec.driver = None;
    let error = support::expect_error(
        support::block_on(foundation_sandbox::shared::control::manager::choose_driver(
            &manager.inner(),
            &spec,
        )),
        "the automatic choice",
    );
    assert!(matches!(error, SandboxError::NoDriver { .. }));
}

/// I2: a parent whose slave cannot control is refused.
#[test]
fn nested_parent_cannot_control() {
    let _serial = support::serial();
    let setup = setup(30, false, false);
    let parent = create_parent(&setup);
    let error = support::expect_error(
        support::block_on(setup.outer.create(child_spec(&parent))),
        "a parent that cannot control",
    );
    assert!(matches!(
        error,
        SandboxError::DriverUnavailable {
            driver: DriverId::Nested,
            ..
        }
    ));
    assert_eq!(
        setup.inner_slave.call_count("info"),
        0,
        "the parent got no Create call"
    );
}

/// The `NotFound` of a missing parent.
#[test]
fn nested_parent_missing() {
    let _serial = support::serial();
    let setup = setup(31, true, false);
    let missing: SandboxId = "0000000000ff".parse().expect("a valid id");
    let error = support::expect_error(
        support::block_on(setup.outer.create(child_spec(&missing))),
        "a missing parent",
    );
    match error {
        SandboxError::NotFound { what } => assert_eq!(what, "parent"),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

/// A parent that is not `Ready` gives `SandboxStopped`.
#[test]
fn nested_parent_not_ready() {
    let _serial = support::serial();
    let setup = setup(32, true, true);
    let mut stream = setup.outer.create_with_progress(parent_spec());
    // The first step adds the record in `Creating`.
    let mut found_id = None;
    for _ in 0..100 {
        let _ = stream.next();
        let listed = support::block_on(setup.outer.list()).expect("list");
        if let Some(found) = listed.first() {
            found_id = Some(found.id.parse().expect("a valid id"));
            break;
        }
        std::thread::yield_now();
    }
    let id: SandboxId = found_id.expect("the parent record");
    let error = support::expect_error(
        support::block_on(setup.outer.create(child_spec(&id))),
        "a parent that is not ready",
    );
    assert!(matches!(error, SandboxError::SandboxStopped));
    setup.clock.advance(Duration::from_secs(11));
    drop(stream);
    std::thread::yield_now();
}

/// I3: stopping the parent stops its children first.
#[test]
fn stop_parent_stops_children() {
    let _serial = support::serial();
    let setup = setup(33, true, false);
    let parent = create_parent(&setup);
    let first = support::block_on(setup.outer.create(child_spec(&parent))).expect("first child");
    let second = support::block_on(setup.outer.create(child_spec(&parent))).expect("second child");
    for child in [&first, &second] {
        assert_eq!(
            child.info().state,
            foundation_sandbox::shared::control::SandboxState::Ready
        );
    }
    let report = support::block_on(setup.outer.stop(parent)).expect("the parent stops");
    assert!(!report.killed);
    assert!(support::block_on(setup.outer.list())
        .expect("list")
        .is_empty());
    assert_eq!(
        setup.inner_slave.call_count("shutdown"),
        2,
        "both inner sandboxes got Shutdown"
    );
    assert_eq!(
        setup.parent_slave.call_count("shutdown"),
        1,
        "the parent got Shutdown"
    );
}

/// I3: when the parent dies, its children become `Stopped`.
#[test]
fn parent_exit_fails_children() {
    let _serial = support::serial();
    let setup = setup(34, true, false);
    let parent = create_parent(&setup);
    let child = support::block_on(setup.outer.create(child_spec(&parent))).expect("the child");
    assert_eq!(
        child.info().state,
        foundation_sandbox::shared::control::SandboxState::Ready
    );
    setup.parent_substrate.exit_now(SubstrateExit {
        code: Some(0),
        signal: None,
        forced: false,
    });
    wait_empty(&setup.outer);
    assert!(support::block_on(setup.inner.list())
        .expect("list")
        .is_empty());
}

/// A transport error is not a stopped child: the watcher keeps polling.
#[test]
fn watcher_retries_a_transport_error() {
    let _serial = support::serial();
    let (ctx, _clock, _timer) = support::recording_ctx_resolve_long(36);
    let limits = SandboxLimits::default();
    let config = NestedWatchConfig::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let outcome = support::block_on(watch_child(&ctx, &limits, &config, || {
        let calls = Arc::clone(&calls);
        async move {
            match calls.fetch_add(1, Ordering::SeqCst) {
                0 => Err(SandboxError::Rpc {
                    code: "unavailable".to_string(),
                    message: "a transport blip".to_string(),
                }
                .into()),
                1 => Ok(SandboxState::Ready),
                _ => Ok(SandboxState::Stopped),
            }
        }
    }));
    assert_eq!(outcome, WatchOutcome::Gone);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        3,
        "the error, the ready read and the stopped read"
    );
}

/// Transport errors that outlast the boot timeout end the watch.
#[test]
fn watcher_gives_up_after_the_boot_timeout() {
    let _serial = support::serial();
    let (ctx, _clock, _timer) = support::recording_ctx_resolve_long(37);
    let limits = SandboxLimits::default().with_boot_timeout(Duration::from_secs(3));
    let config = NestedWatchConfig::default();
    let calls = Arc::new(AtomicUsize::new(0));
    let outcome = support::block_on(watch_child(&ctx, &limits, &config, || {
        let calls = Arc::clone(&calls);
        async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(SandboxError::Rpc {
                code: "unavailable".to_string(),
                message: "a transport blip".to_string(),
            }
            .into())
        }
    }));
    assert_eq!(outcome, WatchOutcome::Timeout);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        4,
        "three waited seconds and the read that passed the limit"
    );
}

/// Only `NotFound` and `SandboxStopped` mean the child is gone.
#[test]
fn watcher_treats_not_found_and_stopped_as_gone() {
    let _serial = support::serial();
    let (ctx, _clock, _timer) = support::recording_ctx(38);
    let limits = SandboxLimits::default();
    let config = NestedWatchConfig::default();
    for error in [
        SandboxError::NotFound {
            what: "sandbox".to_string(),
        },
        SandboxError::SandboxStopped,
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let outcome = support::block_on(watch_child(&ctx, &limits, &config, || {
            let calls = Arc::clone(&calls);
            let error = error.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(error.into())
            }
        }));
        assert_eq!(outcome, WatchOutcome::Gone, "{error:?} is gone");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "one read is enough");
    }
}


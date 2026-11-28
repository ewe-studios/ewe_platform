//! The create story: progress, driver choice and the fault table.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use foundation_sandbox::shared::control::testing::{
    full_capabilities, spec, FailingStore, ScriptedSlave, SimDriver, SlaveCall, SubstrateOp,
};
use foundation_sandbox::shared::control::SandboxProvider;
use foundation_sandbox::shared::control::{
    DropPolicy, ManagerConfig, RecordStore, SandboxIdPolicy, SandboxState,
};
use foundation_sandbox::shared::driver::{Capabilities, CreateProgress, HostReport, SandboxDriver};
use foundation_sandbox::shared::error::{SandboxError, SlaveErrorCode};
use foundation_sandbox::shared::ids::DriverId;
use foundation_sandbox::shared::spec::{RootMode, RootSetup, RootSource};

use super::drain_create;
use super::support;

fn err<T>(result: foundation_sandbox::shared::error::SandboxResult<T>, what: &str) -> SandboxError {
    support::expect_error(result, what)
}

fn driver(id: DriverId) -> Arc<SimDriver> {
    Arc::new(SimDriver::new(id))
}

#[test]
fn create_ok() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(1);
    let slave = Arc::new(ScriptedSlave::new());
    let sim = driver(support::driver_id());
    let (manager, _store) = support::simple_manager(ctx, Arc::clone(&sim), Arc::clone(&slave));
    let mut stream = manager.create_with_progress(spec());
    let (progress, result) = drain_create(&mut stream);
    let handle = result.expect("the create succeeds");
    assert_eq!(
        progress,
        vec![
            CreateProgress::Preparing,
            CreateProgress::SubstrateStarted,
            CreateProgress::SlaveConnected,
            CreateProgress::SlaveReady,
        ]
    );
    assert_eq!(handle.info().state, SandboxState::Ready);
    let calls = slave.calls();
    assert_eq!(calls.first(), Some(&SlaveCall::Info));
    match calls.get(1) {
        Some(SlaveCall::Prepare { root, .. }) => {
            assert_eq!(root.source, RootSource::Plain);
            assert_eq!(root.mode, RootMode::Overlay);
        }
        other => panic!("the second call is not Prepare: {other:?}"),
    }
    let listed = support::block_on(manager.list()).expect("list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].state, SandboxState::Ready);
}

#[test]
fn driver_choice() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(2);
    let slave = Arc::new(ScriptedSlave::new());
    let first = Arc::new(SimDriver::unusable(DriverId::Firecracker, "no kvm"));
    let second = driver(DriverId::Docker);
    let store = Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::new());
    let manager = support::manager(
        ctx,
        vec![
            Arc::clone(&first) as Arc<dyn SandboxDriver>,
            Arc::clone(&second) as Arc<dyn SandboxDriver>,
        ],
        slave,
        ManagerConfig::default(),
        store,
    );
    let handle = support::block_on(manager.create(spec())).expect("create");
    assert_eq!(handle.info().driver, DriverId::Docker);
    assert_eq!(second.launch_calls(), 1);
    assert_eq!(first.launch_calls(), 0);
}

#[test]
fn no_driver() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(3);
    let slave = Arc::new(ScriptedSlave::new());
    let first = Arc::new(SimDriver::unusable(DriverId::Firecracker, "no kvm"));
    let second = Arc::new(SimDriver::unusable(DriverId::Docker, "no socket"));
    let store = Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::new());
    let manager = support::manager(
        ctx,
        vec![
            Arc::clone(&first) as Arc<dyn SandboxDriver>,
            Arc::clone(&second) as Arc<dyn SandboxDriver>,
        ],
        slave,
        ManagerConfig::default(),
        store,
    );
    let error = err(support::block_on(manager.create(spec())), "no driver");
    match error {
        SandboxError::NoDriver { reports } => assert_eq!(reports.len(), 2),
        other => panic!("the error is not NoDriver: {other:?}"),
    }
    assert_eq!(first.launch_calls(), 0);
    assert_eq!(second.launch_calls(), 0);
}

#[test]
fn driver_unavailable() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(4);
    let slave = Arc::new(ScriptedSlave::new());
    let sim = Arc::new(SimDriver::unusable(DriverId::Firecracker, "no kvm"));
    let store = Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::new());
    let manager = support::manager(
        ctx,
        vec![Arc::clone(&sim) as Arc<dyn SandboxDriver>],
        slave,
        ManagerConfig::default(),
        store,
    );
    let mut request = spec();
    request.driver = Some(DriverId::Firecracker);
    let error = err(support::block_on(manager.create(request)), "unavailable");
    match error {
        SandboxError::DriverUnavailable { driver, reason } => {
            assert_eq!(driver, DriverId::Firecracker);
            assert_eq!(reason, "no kvm");
        }
        other => panic!("the error is not DriverUnavailable: {other:?}"),
    }
    assert_eq!(sim.launch_calls(), 0);
}

#[test]
fn validate_before_launch() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(5);
    let slave = Arc::new(ScriptedSlave::new());
    let mut capabilities: Capabilities = full_capabilities();
    capabilities.rootfs_directory = false;
    let sim = Arc::new(SimDriver::with_report(HostReport {
        driver: DriverId::Firecracker,
        usable: true,
        checks: Vec::new(),
        capabilities,
    }));
    let store = Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::new());
    let manager = support::manager(
        ctx,
        vec![Arc::clone(&sim) as Arc<dyn SandboxDriver>],
        slave,
        ManagerConfig::default(),
        store,
    );
    let error = err(
        support::block_on(manager.create(spec())),
        "unsupported root",
    );
    assert!(matches!(error, SandboxError::UnsupportedRootFs));
    assert_eq!(sim.launch_calls(), 0);
}

#[test]
fn launch_error_leaves_nothing() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(6);
    let slave = Arc::new(ScriptedSlave::new());
    let sim = Arc::new(
        SimDriver::new(support::driver_id()).with_launch_error(SandboxError::Io {
            op: "launch".to_string(),
            message: "the driver failed".to_string(),
        }),
    );
    let (manager, _store) = support::simple_manager(ctx, Arc::clone(&sim), slave);
    let error = err(support::block_on(manager.create(spec())), "launch error");
    assert!(matches!(error, SandboxError::Io { .. }));
    assert!(support::block_on(manager.list()).expect("list").is_empty());
}

#[test]
fn substrate_exit_during_start() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(7);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_info_pending();
    let sim = driver(support::driver_id());
    let substrate = sim.substrate();
    substrate.exit_now(foundation_sandbox::shared::driver::SubstrateExit {
        code: Some(1),
        signal: None,
        forced: false,
    });
    let (manager, _store) = support::simple_manager(ctx, sim, slave);
    let error = err(support::block_on(manager.create(spec())), "substrate exit");
    match error {
        SandboxError::SubstrateExited { code, .. } => assert_eq!(code, Some(1)),
        other => panic!("the error is not SubstrateExited: {other:?}"),
    }
    assert_eq!(substrate.op_count(SubstrateOp::Kill), 0);
    assert!(substrate.op_count(SubstrateOp::Cleanup) >= 1);
    assert!(support::block_on(manager.list()).expect("list").is_empty());
}

#[test]
fn prepare_error_stops_create() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(8);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_prepare_error(SandboxError::SlaveError {
        code: SlaveErrorCode::RootSetupFailed,
        errno: None,
        message: "the root failed".to_string(),
    });
    let sim = driver(support::driver_id());
    let substrate = sim.substrate();
    let (manager, _store) = support::simple_manager(ctx, sim, slave);
    let error = err(support::block_on(manager.create(spec())), "prepare error");
    match error {
        SandboxError::SlaveError { code, .. } => {
            assert_eq!(code, SlaveErrorCode::RootSetupFailed);
        }
        other => panic!("the error is not SlaveError: {other:?}"),
    }
    assert!(substrate.op_count(SubstrateOp::Kill) >= 1);
    assert!(substrate.op_count(SubstrateOp::Cleanup) >= 1);
    assert!(support::block_on(manager.list()).expect("list").is_empty());
}

#[test]
fn boot_timeout_leaves_nothing() {
    let _serial = support::serial();
    let (ctx, _clock, _timer) = support::recording_ctx_resolve_long(9);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_info_pending();
    let sim = driver(support::driver_id());
    let substrate = sim.substrate();
    let (manager, _store) = support::simple_manager(ctx, sim, slave);
    let error = err(support::block_on(manager.create(spec())), "boot timeout");
    match error {
        SandboxError::BootTimeout { stage, .. } => assert_eq!(stage, "slave"),
        other => panic!("the error is not BootTimeout: {other:?}"),
    }
    assert!(substrate.op_count(SubstrateOp::Kill) >= 1);
    assert!(substrate.op_count(SubstrateOp::Cleanup) >= 1);
    assert!(support::block_on(manager.list()).expect("list").is_empty());
}

#[test]
fn await_slave_backoff() {
    let _serial = support::serial();
    let (ctx, _clock, timer) = support::recording_ctx(10);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_info_refusals(4);
    let sim = driver(support::driver_id());
    let (manager, _store) = support::simple_manager(ctx, sim, slave);
    let handle = support::block_on(manager.create(spec())).expect("create");
    assert_eq!(handle.info().state, SandboxState::Ready);
    let short: Vec<Duration> = timer
        .waits()
        .into_iter()
        .filter(|wait| *wait <= Duration::from_millis(200))
        .collect();
    assert_eq!(
        short,
        vec![
            Duration::from_millis(5),
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(40),
        ]
    );
}

/// A container that is still starting can miss one `Info` call deadline. The
/// readiness wait retries until the boot deadline, so the create succeeds.
///
/// Fails on the old code: a `deadline_exceeded` answer ended the create at once.
#[test]
fn await_slave_retries_an_info_timeout() {
    let _serial = support::serial();
    let (ctx, _clock, _timer) = support::recording_ctx(11);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_info_timeouts(1);
    let sim = driver(support::driver_id());
    let (manager, _store) = support::simple_manager(ctx, sim, slave);
    let handle = support::block_on(manager.create(spec())).expect("create");
    assert_eq!(handle.info().state, SandboxState::Ready);
}

#[test]
fn store_error_in_create_fails() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(11);
    let slave = Arc::new(ScriptedSlave::new());
    let sim = driver(support::driver_id());
    let store: Arc<dyn RecordStore> = Arc::new(FailingStore::new(1));
    let manager = support::manager(
        ctx,
        vec![sim as Arc<dyn SandboxDriver>],
        slave,
        ManagerConfig::default(),
        store,
    );
    let error = err(support::block_on(manager.create(spec())), "store error");
    assert!(matches!(error, SandboxError::Io { .. }));
    assert!(support::block_on(manager.list()).expect("list").is_empty());
}

#[test]
fn store_error_after_ready_is_logged() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(19);
    let slave = Arc::new(ScriptedSlave::new());
    let sim = Arc::new(SimDriver::new(support::driver_id()));
    slave.with_shutdown_exit(sim.substrate());
    let store = Arc::new(support::SwitchStore::new());
    let manager = support::manager(
        ctx,
        vec![Arc::clone(&sim) as Arc<dyn SandboxDriver>],
        slave,
        ManagerConfig::default(),
        Arc::clone(&store) as Arc<dyn RecordStore>,
    );
    let handle = support::block_on(manager.create(spec())).expect("create");
    assert_eq!(store.records().len(), 1);
    store.fail();
    let report = support::block_on(handle.stop())
        .expect("a store error after Ready does not change the stop result");
    assert!(!report.killed);
    assert_eq!(handle.info().state, SandboxState::Stopped);
    assert!(support::block_on(manager.list()).expect("list").is_empty());
    // The failed remove is logged and leaves the record in the store.
    assert_eq!(store.records().len(), 1);
    // The manager stays usable after the logged error.
    store.recover();
    sim.substrate().reset();
    let again = support::block_on(manager.create(spec())).expect("a later create");
    assert_eq!(again.info().state, SandboxState::Ready);
    support::block_on(again.kill()).expect("kill");
}

#[test]
fn max_sandboxes() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(12);
    let slave = Arc::new(ScriptedSlave::new());
    let sim = driver(support::driver_id());
    let store = Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::new());
    let config = ManagerConfig::default().with_max_sandboxes(1);
    let manager = support::manager(
        ctx,
        vec![Arc::clone(&sim) as Arc<dyn SandboxDriver>],
        slave,
        config,
        store,
    );
    let _first = support::block_on(manager.create(spec())).expect("first");
    let error = err(support::block_on(manager.create(spec())), "limit");
    assert!(matches!(error, SandboxError::TooManySessions));
}

#[test]
fn fixed_id() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(13);
    let slave = Arc::new(ScriptedSlave::new());
    let sim = driver(support::driver_id());
    slave.with_shutdown_exit(sim.substrate());
    let store = Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::new());
    let id: foundation_sandbox::shared::ids::SandboxId = "aaaaaaaaaaaa".parse().expect("id");
    let config = ManagerConfig::default().with_id_policy(SandboxIdPolicy::Fixed(id.clone()));
    let manager = support::manager(
        ctx,
        vec![Arc::clone(&sim) as Arc<dyn SandboxDriver>],
        slave,
        config,
        store,
    );
    let handle = support::block_on(manager.create(spec())).expect("first");
    assert_eq!(handle.id(), &id);
    let error = err(support::block_on(manager.create(spec())), "second");
    assert!(matches!(error, SandboxError::TooManySessions));
    support::block_on(handle.stop()).expect("stop");
    sim.substrate().reset();
    let again = support::block_on(manager.create(spec())).expect("after stop");
    assert_eq!(again.id(), &id);
}

#[test]
fn record_created_time_is_set() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(14);
    let slave = Arc::new(ScriptedSlave::new());
    let sim = driver(support::driver_id());
    let (manager, _store) = support::simple_manager(ctx, sim, slave);
    let handle = support::block_on(manager.create(spec())).expect("create");
    assert_eq!(handle.info().created_unix_ms, 0);
}

#[test]
fn drop_policy_detach_leaves_the_substrate() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(15);
    let slave = Arc::new(ScriptedSlave::new());
    let sim = driver(support::driver_id());
    let substrate = sim.substrate();
    let store = Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::new());
    let config = ManagerConfig::default().with_on_drop(DropPolicy::Detach);
    let manager = support::manager(
        ctx,
        vec![Arc::clone(&sim) as Arc<dyn SandboxDriver>],
        slave,
        config,
        store,
    );
    let _handle = support::block_on(manager.create(spec())).expect("create");
    drop(manager);
    std::thread::yield_now();
    assert_eq!(substrate.op_count(SubstrateOp::Kill), 0);
    // End the watcher too: it holds the detached sandbox until it exits.
    substrate.exit_now(foundation_sandbox::shared::driver::SubstrateExit {
        code: Some(0),
        signal: None,
        forced: false,
    });
    let mut attempts = 0;
    while substrate.op_count(SubstrateOp::Cleanup) == 0 && attempts < 10_000 {
        std::thread::yield_now();
        attempts += 1;
    }
}

#[test]
fn drop_policy_kill_all_kills_the_substrate() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(16);
    let slave = Arc::new(ScriptedSlave::new());
    let sim = driver(support::driver_id());
    let substrate = sim.substrate();
    let store = Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::new());
    let manager = support::manager(
        ctx,
        vec![Arc::clone(&sim) as Arc<dyn SandboxDriver>],
        slave,
        ManagerConfig::default(),
        store,
    );
    let _handle = support::block_on(manager.create(spec())).expect("create");
    drop(manager);
    let mut attempts = 0;
    while (substrate.op_count(SubstrateOp::Kill) == 0
        || substrate.op_count(SubstrateOp::Cleanup) == 0)
        && attempts < 10_000
    {
        std::thread::yield_now();
        attempts += 1;
    }
    assert_eq!(substrate.op_count(SubstrateOp::Kill), 1);
    assert!(substrate.op_count(SubstrateOp::Cleanup) >= 1);
}

#[test]
fn root_and_mounts_come_from_the_driver() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(17);
    let slave = Arc::new(ScriptedSlave::new());
    let root = RootSetup {
        source: RootSource::VirtioFs {
            tag: "root".to_string(),
        },
        mode: RootMode::ReadOnly,
    };
    let sim = Arc::new(
        SimDriver::new(support::driver_id())
            .with_root(root.clone())
            .with_mounts(vec![foundation_sandbox::shared::spec::GuestMount {
                tag: "work".to_string(),
                guest_path: "/work".to_string(),
                read_only: false,
            }]),
    );
    let (manager, _store) = support::simple_manager(ctx, sim, slave.clone());
    support::block_on(manager.create(spec())).expect("create");
    match &slave.calls()[1] {
        SlaveCall::Prepare { root: prepared, .. } => assert_eq!(prepared, &root),
        other => panic!("the second call is not Prepare: {other:?}"),
    }
}

#[test]
fn prepare_workdir_is_the_spec_workdir() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(18);
    let slave = Arc::new(ScriptedSlave::new());
    let sim = driver(support::driver_id());
    let (manager, _store) = support::simple_manager(ctx, sim, slave.clone());
    let mut request = spec();
    request.workdir = "/custom".to_string();
    support::block_on(manager.create(request)).expect("create");
    match &slave.calls()[1] {
        SlaveCall::Prepare { workdir, .. } => assert_eq!(workdir, "/custom"),
        other => panic!("the second call is not Prepare: {other:?}"),
    }
}

/// Issue #1031: an empty workdir creates and `Prepare` carries the default.
#[test]
fn empty_workdir_uses_the_default() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(19);
    let slave = Arc::new(ScriptedSlave::new());
    let sim = driver(support::driver_id());
    let (manager, _store) = support::simple_manager(ctx, sim, slave.clone());
    let mut request = spec();
    request.workdir = String::new();
    support::block_on(manager.create(request)).expect("the create succeeds");
    match &slave.calls()[1] {
        SlaveCall::Prepare { workdir, .. } => assert_eq!(workdir, "/workspace"),
        other => panic!("the second call is not Prepare: {other:?}"),
    }
}

#[test]
fn default_spec_uses_directory_root() {
    let _serial = support::serial();
    let request = spec();
    assert!(matches!(
        request.rootfs,
        foundation_sandbox::shared::spec::RootFs::Directory(ref path)
            if path == &PathBuf::from("/tmp/root")
    ));
}

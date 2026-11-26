//! `restore`: finding the sandboxes of a control plane that restarted.

use std::sync::Arc;

use foundation_sandbox::shared::control::testing::{
    spec, AttachMode, CountingStore, ScriptedSlave, SimDriver, SubstrateOp,
};
use foundation_sandbox::shared::control::{RecordStore, SandboxProvider, SandboxState};
use foundation_sandbox::shared::driver::{SandboxDriver, StoredRecord};
use foundation_sandbox::shared::ids::{DriverId, SandboxId};

use super::support;

fn stored(id: &SandboxId, state: SandboxState) -> StoredRecord {
    let spec = spec();
    StoredRecord {
        id: id.clone(),
        driver: DriverId::Firecracker,
        endpoint: Some(foundation_sandbox::shared::driver::SlaveEndpoint::Unix {
            path: std::path::PathBuf::from("/tmp/sim-slave.sock"),
        }),
        spec,
        state,
        created_unix_ms: 0,
        expires_at_unix_ms: None,
    }
}

fn id() -> SandboxId {
    "aaaaaaaaaaaa".parse().expect("id")
}

#[test]
fn restore_ready_record() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(50);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_info_prepared(true);
    let driver = Arc::new(SimDriver::new(DriverId::Firecracker).with_attach(AttachMode::Found));
    let store = Arc::new(CountingStore::new());
    support::block_on(store.put(stored(&id(), SandboxState::Ready))).expect("store");
    let manager = support::manager(
        ctx,
        vec![Arc::clone(&driver) as Arc<dyn SandboxDriver>],
        Arc::clone(&slave),
        foundation_sandbox::shared::control::ManagerConfig::default(),
        Arc::clone(&store) as Arc<dyn RecordStore>,
    );
    support::block_on(manager.restore()).expect("restore");
    let handle = support::block_on(manager.get(id())).expect("get");
    assert_eq!(handle.info().state, SandboxState::Ready);
    assert_eq!(slave.call_count("info"), 1);
    assert_eq!(slave.call_count("prepare"), 0);
    assert_eq!(driver.launch_calls(), 0);
    assert_eq!(driver.attach_calls(), 1);
}

#[test]
fn restore_dead_substrate() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(51);
    let slave = Arc::new(ScriptedSlave::new());
    let driver = Arc::new(SimDriver::new(DriverId::Firecracker));
    let store = Arc::new(CountingStore::new());
    support::block_on(store.put(stored(&id(), SandboxState::Ready))).expect("store");
    let manager = support::manager(
        ctx,
        vec![driver as Arc<dyn SandboxDriver>],
        slave,
        foundation_sandbox::shared::control::ManagerConfig::default(),
        Arc::clone(&store) as Arc<dyn RecordStore>,
    );
    support::block_on(manager.restore()).expect("restore");
    let error = support::expect_error(support::block_on(manager.get(id())), "gone");
    assert!(matches!(
        error,
        foundation_sandbox::shared::error::SandboxError::NotFound { .. }
    ));
    assert!(store.records().is_empty());
}

#[test]
fn restore_unprepared_slave() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(52);
    let slave = Arc::new(ScriptedSlave::new());
    let driver = Arc::new(SimDriver::new(DriverId::Firecracker).with_attach(AttachMode::Found));
    let substrate = driver.substrate();
    let store = Arc::new(CountingStore::new());
    support::block_on(store.put(stored(&id(), SandboxState::Ready))).expect("store");
    let manager = support::manager(
        ctx,
        vec![driver as Arc<dyn SandboxDriver>],
        slave,
        foundation_sandbox::shared::control::ManagerConfig::default(),
        Arc::clone(&store) as Arc<dyn RecordStore>,
    );
    support::block_on(manager.restore()).expect("restore");
    assert!(support::block_on(manager.get(id())).is_err());
    assert!(store.records().is_empty());
    assert!(substrate.op_count(SubstrateOp::Cleanup) >= 1);
}

#[test]
fn restore_unfinished_record() {
    let _serial = support::serial();
    for state in [SandboxState::Creating, SandboxState::Stopping] {
        let (ctx, _clock) = support::ctx(53);
        let slave = Arc::new(ScriptedSlave::new());
        let driver = Arc::new(SimDriver::new(DriverId::Firecracker).with_attach(AttachMode::Found));
        let substrate = driver.substrate();
        let store = Arc::new(CountingStore::new());
        support::block_on(store.put(stored(&id(), state))).expect("store");
        let manager = support::manager(
            ctx,
            vec![driver as Arc<dyn SandboxDriver>],
            slave,
            foundation_sandbox::shared::control::ManagerConfig::default(),
            Arc::clone(&store) as Arc<dyn RecordStore>,
        );
        support::block_on(manager.restore()).expect("restore");
        assert!(support::block_on(manager.get(id())).is_err());
        assert!(store.records().is_empty());
        assert!(substrate.op_count(SubstrateOp::Kill) >= 1);
        assert!(substrate.op_count(SubstrateOp::Cleanup) >= 1);
    }
}

#[test]
fn restore_twice_does_nothing_the_second_time() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(54);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_info_prepared(true);
    let driver = Arc::new(SimDriver::new(DriverId::Firecracker).with_attach(AttachMode::Found));
    let store = Arc::new(CountingStore::new());
    support::block_on(store.put(stored(&id(), SandboxState::Ready))).expect("store");
    let manager = support::manager(
        ctx,
        vec![driver as Arc<dyn SandboxDriver>],
        Arc::clone(&slave),
        foundation_sandbox::shared::control::ManagerConfig::default(),
        store,
    );
    support::block_on(manager.restore()).expect("restore");
    support::block_on(manager.restore()).expect("restore again");
    assert_eq!(slave.call_count("info"), 1);
}

#[test]
fn restore_removes_a_stopped_record() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(55);
    let slave = Arc::new(ScriptedSlave::new());
    let store = Arc::new(CountingStore::new());
    support::block_on(store.put(stored(&id(), SandboxState::Stopped))).expect("store");
    let manager = support::manager(
        ctx,
        Vec::new(),
        slave,
        foundation_sandbox::shared::control::ManagerConfig::default(),
        Arc::clone(&store) as Arc<dyn RecordStore>,
    );
    support::block_on(manager.restore()).expect("restore");
    assert!(store.records().is_empty());
}

//! The small contracts: the bearer check, the container control and the state
//! machine. The source scan proves the module has no operating-system call.

use std::path::Path;
use std::sync::Arc;

use foundation_sandbox::shared::container::ContainerControl;
use foundation_sandbox::shared::control::record::allowed_edge;
use foundation_sandbox::shared::control::testing::{
    in_memory_connector, FakeContainerControl, ScriptedSlave, SimDriver,
};
use foundation_sandbox::shared::control::{authorize, ManagerConfig, SandboxState};
use foundation_sandbox::shared::driver::SandboxDriver;
use foundation_sandbox::shared::host::SlaveConnector;
use foundation_sandbox::shared::{extract_bearer_token, Secret};

use super::support;

#[test]
fn check_bearer_cases() {
    let _serial = support::serial();
    let token = Secret::new("right-token");
    assert!(authorize(None, &token).is_err());
    assert!(authorize(Some("Basic right-token"), &token).is_err());
    assert!(authorize(Some("Bearer wrong-token"), &token).is_err());
    assert!(authorize(Some("Bearer right-token"), &token).is_ok());
    assert!(authorize(Some("bearer right-token"), &token).is_ok());
    assert_eq!(
        extract_bearer_token(Some("Bearer right-token")),
        Some("right-token".to_string())
    );
}

#[test]
fn manager_containers() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(60);
    let slave = Arc::new(ScriptedSlave::new());
    let connector: Arc<dyn SlaveConnector> = in_memory_connector(slave);
    let plain = Arc::new(SimDriver::new(
        foundation_sandbox::shared::ids::DriverId::Firecracker,
    ));
    let first = Arc::new(FakeContainerControl);
    let second = Arc::new(FakeContainerControl);
    let with_first = Arc::new(
        SimDriver::new(foundation_sandbox::shared::ids::DriverId::Docker)
            .with_container(Arc::clone(&first) as Arc<dyn ContainerControl>),
    );
    let with_second = Arc::new(
        SimDriver::new(foundation_sandbox::shared::ids::DriverId::Libkrun)
            .with_container(Arc::clone(&second) as Arc<dyn ContainerControl>),
    );
    let manager = foundation_sandbox::shared::control::SandboxManager::new(
        ctx,
        vec![
            plain as Arc<dyn SandboxDriver>,
            with_first as Arc<dyn SandboxDriver>,
            with_second as Arc<dyn SandboxDriver>,
        ],
        connector,
        Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::new()),
        ManagerConfig::default(),
    );
    let found = manager
        .containers()
        .expect("the first driver with a control");
    let expected: Arc<dyn ContainerControl> = Arc::clone(&first) as Arc<dyn ContainerControl>;
    assert!(Arc::ptr_eq(&found, &expected));
}

#[test]
fn manager_without_container_control() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(61);
    let slave = Arc::new(ScriptedSlave::new());
    let connector: Arc<dyn SlaveConnector> = in_memory_connector(slave);
    let plain = Arc::new(SimDriver::new(
        foundation_sandbox::shared::ids::DriverId::Firecracker,
    ));
    let manager = foundation_sandbox::shared::control::SandboxManager::new(
        ctx,
        vec![plain as Arc<dyn SandboxDriver>],
        connector,
        Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::new()),
        ManagerConfig::default(),
    );
    assert!(manager.containers().is_none());
}

#[test]
fn state_machine_edges() {
    let _serial = support::serial();
    use SandboxState::{Creating, Failed, Ready, Stopped, Stopping};
    assert!(allowed_edge(Creating, Ready));
    assert!(allowed_edge(Creating, Failed));
    assert!(allowed_edge(Ready, Stopping));
    assert!(allowed_edge(Ready, Failed));
    assert!(allowed_edge(Stopping, Stopped));
    assert!(allowed_edge(Failed, Stopped));
    assert!(!allowed_edge(Stopped, Ready));
    assert!(!allowed_edge(Stopping, Ready));
    assert!(!allowed_edge(Creating, Stopping));
}

#[test]
fn no_os_calls() {
    let _serial = support::serial();
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shared/control");
    let mut files = Vec::new();
    collect(&directory, &mut files);
    let forbidden = [
        "std::process",
        "std::net",
        "std::fs",
        "SystemTime::now",
        "Instant::now",
    ];
    for file in files {
        let text = std::fs::read_to_string(&file).expect("read a source file");
        for token in forbidden {
            assert!(
                !text.contains(token),
                "{file:?} holds the forbidden call {token}"
            );
        }
    }
}

fn collect(directory: &Path, files: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(directory).expect("read the control directory") {
        let entry = entry.expect("a directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

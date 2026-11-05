//! Docker test-environment tests (moved from `src/docker.rs`).
#![cfg(feature = "docker-tests")]

use foundation_testbed::docker::{TestEnvError, TestEnvironmentBuilder, port_is_open};

#[test]
fn builder_container_not_found_when_not_running() {
    let builder = TestEnvironmentBuilder::new("nonexistent_container_12345");
    assert!(!builder.is_running());
    match builder.connect() {
        Err(TestEnvError::ContainerNotFound(c)) => assert!(c.contains("nonexistent")),
        other => panic!("expected ContainerNotFound, got {other:?}"),
    }
}

#[test]
fn builder_with_vnc_configures_correctly() {
    // `docker-compose.yaml` names the macOS container `ewe_macos`. This test
    // needs that container to run; the Platform Docker Tests workflow starts
    // it in the `macos-tests` job before it runs this file.
    let builder = TestEnvironmentBuilder::new("ewe_macos").with_vnc(5900);
    assert!(builder.is_running()); // macOS container should be up
}

#[test]
fn builder_with_rdp_configures_correctly() {
    let builder = TestEnvironmentBuilder::new("ewe_windows").with_rdp(3389, "Docker", "admin");
    assert!(builder.is_running()); // Windows container should be up
}

#[test]
fn port_is_open_returns_bool() {
    // 192.0.2.0/24 is TEST-NET-1, reserved and unreachable — always false.
    let result = port_is_open("192.0.2.1", 80);
    assert!(!result);
}

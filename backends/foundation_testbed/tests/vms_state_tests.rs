//! Tests extracted from vms/state/mod.rs

use foundation_testbed::vms::state::{from_qemu, is_process_alive, state_dir, VmState};

#[test]
fn test_is_process_alive_check() {
    // Our own PID should always be alive
    #[cfg(unix)]
    {
        let pid = std::process::id();
        assert!(is_process_alive(pid), "own process should be alive");
        // Non-existent PID should be dead
        assert!(!is_process_alive(999_999_999));
    }
}

#[test]
fn test_state_dir_exists() {
    let dir = state_dir();
    assert!(dir.to_str().unwrap().contains("foundation_testbed"));
}

#[test]
fn test_from_qemu_creates_valid_state() {
    let state = from_qemu(
        "test-vm",
        std::path::Path::new("/tmp/test.qcow2"),
        12345,
        std::path::Path::new("/tmp/test.monitor"),
        2222,
        Some(5985),
        Some(3389),
        5900,
        false,
    );
    assert_eq!(state.profile_name, "test-vm");
    assert_eq!(state.pid, Some(12345));
    assert_eq!(state.ssh_port, 2222);
    assert!(!state.bootstrapped);
    assert!(!state.created_at.is_empty());
}

#[test]
fn test_state_serialization_roundtrip() {
    let state = from_qemu(
        "roundtrip",
        std::path::Path::new("/tmp/rt.qcow2"),
        1,
        std::path::Path::new("/tmp/rt.monitor"),
        2222,
        None,
        None,
        5900,
        true,
    );
    let json = serde_json::to_string(&state).unwrap();
    let restored: VmState = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.profile_name, state.profile_name);
    assert_eq!(restored.pid, state.pid);
}

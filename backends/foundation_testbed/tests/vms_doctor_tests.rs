//! Tests extracted from vms/doctor/mod.rs

use foundation_testbed::vms::doctor::{check_host, find_ssh_key, kvm_available};

#[test]
fn test_host_health_returns_checks() {
    let health = check_host();
    assert!(!health.checks.is_empty());
    // Should have at least kvm, qemu, qemu-img, ssh_key, disk_space
    assert!(health.checks.len() >= 5);
}

#[test]
fn test_kvm_available() {
    // Just verify it doesn't panic
    let _ = kvm_available();
}

#[test]
fn test_find_ssh_key() {
    // Just verify it doesn't panic
    let _ = find_ssh_key();
}

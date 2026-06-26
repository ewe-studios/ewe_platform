//! Tests extracted from vms/host_bootstrap/ modules

use foundation_testbed::vms::host_bootstrap::{host_mise_config_path, ToolStatus};
use foundation_testbed::vms::host_bootstrap::install::find_mise_bin;

// From host_bootstrap/mod.rs
#[test]
fn test_tool_status_is_available() {
    let present = ToolStatus::AlreadyPresent { version: "1.0".into() };
    assert!(present.is_available());
    let missing = ToolStatus::Missing { error: "not found".into() };
    assert!(!missing.is_available());
}

#[test]
fn test_tool_status_version() {
    let present = ToolStatus::AlreadyPresent { version: "2.0.0".into() };
    assert_eq!(present.version(), Some("2.0.0"));
    let missing = ToolStatus::Missing { error: "nope".into() };
    assert_eq!(missing.version(), None);
}

#[test]
fn test_host_mise_config_path_is_absolute() {
    let path = host_mise_config_path();
    assert!(path.is_absolute());
    assert!(path.ends_with("config.toml"));
}

// From host_bootstrap/install.rs
#[test]
fn test_find_mise_bin_not_found() {
    // This will fail gracefully -- just verify it doesn't panic
    let result = find_mise_bin();
    // May or may not be present depending on the host
    let _ = result.is_ok() || result.is_err();
}

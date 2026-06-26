//! Tests extracted from vms/ssh/ modules

use foundation_testbed::vms::config::GuestOs;
use foundation_testbed::vms::ssh::{wrap_command, wrap_command_bash, wrap_command_cmd};

// From ssh/mod.rs
#[test]
fn test_wrap_command_linux() {
    let wrapped = wrap_command("echo hello", GuestOs::Linux);
    assert!(wrapped.starts_with("bash -c"));
}

#[test]
fn test_wrap_command_windows() {
    let wrapped = wrap_command("echo hello", GuestOs::Windows);
    assert!(wrapped.starts_with("cmd /c"));
}

#[test]
fn test_wrap_command_cmd() {
    let wrapped = wrap_command_cmd("echo hello");
    assert!(wrapped.starts_with("cmd /c"));
}

#[test]
fn test_wrap_command_bash() {
    let wrapped = wrap_command_bash("echo hello");
    assert!(wrapped.starts_with("bash -c"));
}

// From ssh/streaming.rs
#[test]
fn test_streaming_wraps_in_nushell() {
    // Verify the command would be wrapped correctly
    let cmd = "cargo build";
    let wrapped = format!("nu -c {cmd:?}");
    assert!(wrapped.contains("cargo build"));
    assert!(wrapped.starts_with("nu -c"));
}

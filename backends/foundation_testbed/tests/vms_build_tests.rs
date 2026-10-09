//! Tests extracted from vms/build/ modules

use foundation_testbed::vms::build::{guest_project_path, target_triple};
use foundation_testbed::vms::build::logs::{extract_errors, ERROR_PATTERNS};
use foundation_testbed::vms::build::multiarch::{cross_compile_env, CROSS_DEPS};
use foundation_testbed::vms::config::GuestOs;

// From build/mod.rs
#[test]
fn test_target_triple() {
    assert_eq!(target_triple(GuestOs::Windows), "x86_64-pc-windows-msvc");
    assert_eq!(target_triple(GuestOs::Linux), "aarch64-unknown-linux-gnu");
}

#[test]
fn test_guest_project_path() {
    assert_eq!(guest_project_path(GuestOs::Linux), "/mnt/project");
    assert_eq!(guest_project_path(GuestOs::Windows), "C:\\Users\\vagrant\\project");
    assert_eq!(guest_project_path(GuestOs::MacOS), "/Volumes/project");
}

// From build/multiarch.rs
#[test]
fn test_cross_compile_env_contains_expected_vars() {
    let env = cross_compile_env();
    assert!(env.contains("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"));
    assert!(env.contains("PKG_CONFIG_ALLOW_CROSS"));
    assert!(env.contains("PKG_CONFIG_PATH"));
}

#[test]
fn test_cross_deps_not_empty() {
    assert!(!CROSS_DEPS.is_empty());
}

// From build/logs.rs
#[test]
fn test_extract_errors_finds_rust_error() {
    let log = r#"Compiling my-app v0.1.0
   Compiling dependency v1.0.0
error[E0308]: mismatched types
   --> src/main.rs:10:5
    |
10  |     let x: String = 42;
    |         ^   ------ expected due to this
    |         |
    |         expected `String`, found integer
"#;
    let result = extract_errors(log).unwrap();
    assert!(result.contains("E0308"));
    assert!(result.contains("mismatched types"));
}

#[test]
fn test_extract_errors_empty_log() {
    let result = extract_errors("Build completed successfully\n").unwrap();
    assert!(result.contains("No error patterns"));
}

#[test]
fn test_error_patterns_contains_expected_values() {
    assert!(ERROR_PATTERNS.iter().any(|p| p.contains("error")));
    assert!(ERROR_PATTERNS.iter().any(|p| p.contains("FAILED")));
    assert!(ERROR_PATTERNS.iter().any(|p| p.contains("linker")));
}

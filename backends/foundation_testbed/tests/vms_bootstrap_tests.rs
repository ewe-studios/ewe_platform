//! Tests extracted from vms/bootstrap/ modules

use foundation_testbed::vms::bootstrap::BOOTSTRAP_MISE_TOML;

// From bootstrap/mod.rs
#[test]
fn test_bootstrap_mise_toml_is_valid() {
    let toml = BOOTSTRAP_MISE_TOML;
    assert!(toml.contains("[tools]"));
    assert!(toml.contains("rust"));
    assert!(toml.contains("nushell"));
    assert!(toml.contains("tauri-cli"));
}

// From bootstrap/linux.rs
#[test]
fn test_linux_bootstrap_mise_toml_has_required_tools() {
    let toml = BOOTSTRAP_MISE_TOML;
    assert!(toml.contains("rust"));
    assert!(toml.contains("nushell"));
    assert!(toml.contains("tauri-cli"));
}

// From bootstrap/macos.rs
#[test]
fn test_macos_bootstrap_mise_toml_has_required_tools() {
    let toml = BOOTSTRAP_MISE_TOML;
    assert!(toml.contains("rust"));
    assert!(toml.contains("nushell"));
    assert!(toml.contains("tauri-cli"));
}

// From bootstrap/windows.rs
#[test]
fn test_windows_bootstrap_mise_toml_has_required_tools() {
    let toml = BOOTSTRAP_MISE_TOML;
    assert!(toml.contains("rust"));
    assert!(toml.contains("nushell"));
    assert!(toml.contains("tauri-cli"));
}

// From bootstrap/logger.rs
#[test]
fn test_logger_creates_directory() {
    use foundation_testbed::vms::bootstrap::logger::BootstrapLogger;
    use std::path::PathBuf;

    let logger = BootstrapLogger::new("test-logger-tmp").unwrap();
    assert!(PathBuf::from(".testbed/test-logger-tmp").exists());
    assert!(logger.log_path().exists());
    let _ = std::fs::remove_dir_all(".testbed/test-logger-tmp");
}

// From bootstrap/boot_wait.rs
#[test]
fn test_wait_for_ssh_times_out_on_unreachable_port() {
    use foundation_testbed::vms::bootstrap::boot_wait::wait_for_ssh;
    use foundation_testbed::vms::config::get_profile;

    let profile = get_profile("linux-test").unwrap();
    // Use a different port for this test
    let mut test_profile = profile.clone();
    test_profile.ssh_port = 64000;
    let result = wait_for_ssh(&test_profile, 1);
    assert!(result.is_err());
}

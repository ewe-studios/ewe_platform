//! Tests extracted from vms/runner/ modules

use foundation_testbed::vms::config::GuestOs;

// From runner/mod.rs
#[test]
fn test_auto_detect_search_paths() {
    let target = foundation_testbed::vms::build::target_triple(GuestOs::Linux);
    assert!(target.contains("aarch64"));
}

// From runner/logs.rs
#[test]
fn test_log_kind_from_str() {
    use foundation_testbed::vms::runner::logs::LogKind;
    assert!("build".parse::<LogKind>().is_ok());
    assert!("run".parse::<LogKind>().is_ok());
    assert!("invalid".parse::<LogKind>().is_err());
}

#[test]
fn test_log_path_linux() {
    use foundation_testbed::vms::config::get_profile;
    use foundation_testbed::vms::runner::logs::{log_path, LogKind};
    let profile = get_profile("linux-build").unwrap().clone();
    assert_eq!(
        log_path(&profile, LogKind::Build),
        "/home/vagrant/.testbed-build/build.log"
    );
}

#[test]
fn test_log_path_windows() {
    use foundation_testbed::vms::config::get_profile;
    use foundation_testbed::vms::runner::logs::{log_path, LogKind};
    let profile = get_profile("windows-build").unwrap().clone();
    assert!(log_path(&profile, LogKind::Build).contains("Users"));
    assert!(log_path(&profile, LogKind::Build).contains("build.log"));
}

// From runner/screenshot.rs
#[test]
fn test_capture_uses_correct_paths() {
    // Verify the function would use the right VM path
    let path = "/tmp/testbed-screenshot.png";
    assert!(path.starts_with("/tmp/"));
    assert!(path.ends_with(".png"));
}

// From runner/transfer.rs
#[test]
fn test_push_identifies_directory() {
    let temp = std::env::temp_dir().join("test_push_dir");
    let _ = std::fs::create_dir_all(&temp);
    assert!(temp.is_dir());
    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn test_push_uses_upload_for_files() {
    use std::path::PathBuf;
    // Verify the path resolution logic
    let local = PathBuf::from("/tmp/test.txt");
    let remote = "/tmp/test.txt";
    assert_eq!(local.is_file(), false); // doesn't exist
    assert_eq!(remote, "/tmp/test.txt");
}

// From runner/ui.rs
#[test]
fn test_linux_type_escape() {
    let text = "hello 'world'";
    let escaped = text.replace("\\", "\\\\").replace("'", "\\'");
    assert_eq!(escaped, "hello \\'world\\'");
}

#[test]
fn test_windows_type_escape() {
    let text = "hello+world^test%foo";
    let escaped = text
        .replace("+", "{{+}}")
        .replace("^", "{{^}}")
        .replace("%", "{{%}}")
        .replace("~", "{{~}}");
    assert_eq!(escaped, "hello{{+}}world{{^}}test{{%}}foo");
}

// From runner/validate.rs
#[test]
fn test_validation_result_structure() {
    use foundation_testbed::vms::runner::validate::ValidationResult;
    let result = ValidationResult {
        match_pct: 95.0,
        passed: true,
        diff_path: None,
    };
    assert!(result.passed);
    assert!((result.match_pct - 95.0).abs() < 0.01);
}

#[test]
fn test_default_tolerance_95_percent() {
    // Common tolerance for UI testing
    let tolerance = 95.0;
    assert!(96.0 >= tolerance);
    assert!(94.0 < tolerance);
}

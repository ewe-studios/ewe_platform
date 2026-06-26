//! Tests extracted from vms/artifacts.rs

use foundation_testbed::vms::artifacts::scan_artifacts;
use foundation_testbed::vms::config::GuestOs;

#[test]
fn test_scan_artifacts_filters_extensions() {
    // Create a temp build dir with mixed files
    let dir = std::env::temp_dir().join("testbed_test_artifacts");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // Linux artifacts
    std::fs::write(dir.join("my-app"), "").unwrap();       // executable
    std::fs::write(dir.join("libfoo.rlib"), "").unwrap();  // rlib (skipped)
    std::fs::write(dir.join("libbar.so"), "").unwrap();    // shared lib

    let artifacts = scan_artifacts(&dir, GuestOs::Linux).unwrap();
    // Should find my-app (no ext) and libbar.so
    let names: Vec<_> = artifacts.iter().map(|p| p.file_name().unwrap().to_string_lossy()).collect();
    assert!(names.iter().any(|n| n == "my-app"));
    assert!(names.iter().any(|n| n == "libbar.so"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_scan_artifacts_windows() {
    let dir = std::env::temp_dir().join("testbed_test_artifacts_win");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    std::fs::write(dir.join("my-app.exe"), "").unwrap();
    std::fs::write(dir.join("my-lib.dll"), "").unwrap();
    std::fs::write(dir.join("README.txt"), "").unwrap();

    let artifacts = scan_artifacts(&dir, GuestOs::Windows).unwrap();
    let names: Vec<_> = artifacts.iter().map(|p| p.file_name().unwrap().to_string_lossy()).collect();
    assert!(names.iter().any(|n| n == "my-app.exe"));
    assert!(names.iter().any(|n| n == "my-lib.dll"));
    assert!(!names.iter().any(|n| n == "README.txt"));

    let _ = std::fs::remove_dir_all(&dir);
}

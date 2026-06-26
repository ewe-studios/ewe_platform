//! Tests extracted from vms/import/ modules

use foundation_testbed::vms::import::{validate_image, MIN_IMAGE_SIZE};

// From import/mod.rs
#[test]
fn test_min_image_size_constant() {
    assert_eq!(MIN_IMAGE_SIZE, 100 * 1_048_576);
}

#[test]
fn test_validate_image_rejects_small_file() {
    let path = std::path::PathBuf::from("/tmp/test_small_image.qcow2");
    std::fs::write(&path, b"tiny").unwrap();
    let err = validate_image(&path).unwrap_err();
    assert!(err.to_string().contains("failed download"));
    let _ = std::fs::remove_file(&path);
}

// From import/macos.rs
#[test]
fn test_config_plist_is_valid_xml() {
    let config = foundation_testbed::vms::import::macos::generate_config_plist();
    assert!(config.contains("<?xml"));
    assert!(config.contains("iMacPro1,1"));
    assert!(config.contains("boot-args"));
}

#[test]
fn test_config_plist_has_required_sections() {
    let config = foundation_testbed::vms::import::macos::generate_config_plist();
    assert!(config.contains("<key>PlatformInfo</key>"));
    assert!(config.contains("<key>SystemProductName</key>"));
    assert!(config.contains("<key>NVRAM</key>"));
    assert!(config.contains("<key>UEFI</key>"));
    assert!(config.contains("<key>Misc</key>"));
    assert!(config.contains("<key>Kernel</key>"));
}

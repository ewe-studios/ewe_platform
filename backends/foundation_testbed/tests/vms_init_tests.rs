//! Tests extracted from vms/init/mod.rs

use foundation_testbed::vms::init::{
    generate_gitignore, generate_testbed_toml, BUILTIN_DEFENDER_EXCLUSIONS, BUILTIN_MOUNT_CHECK,
};

#[test]
fn test_generate_testbed_toml_has_required_sections() {
    let toml = generate_testbed_toml(&["linux-build".to_string()]);
    assert!(toml.contains("[[vms]]"));
    assert!(toml.contains("mount"));
    assert!(toml.contains("guest_path"));
    assert!(toml.contains("[artifacts]"));
}

#[test]
fn test_generate_gitignore_ignores_state() {
    let gitignore = generate_gitignore();
    assert!(gitignore.contains("/state/"));
    assert!(gitignore.contains("/logs/"));
    assert!(gitignore.contains("/artifacts/"));
    assert!(gitignore.contains("/mounts/"));
}

#[test]
fn test_builtin_scripts_have_content() {
    assert!(!BUILTIN_DEFENDER_EXCLUSIONS.is_empty());
    assert!(!BUILTIN_MOUNT_CHECK.is_empty());
    assert!(BUILTIN_DEFENDER_EXCLUSIONS.contains("Add-MpPreference"));
    assert!(BUILTIN_MOUNT_CHECK.contains("/mnt/project"));
}

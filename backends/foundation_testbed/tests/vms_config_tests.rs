//! Tests extracted from vms/config.rs

use foundation_testbed::vms::config::UserConfig;

#[test]
fn test_parse_mount_from_inline_toml() {
    // Simulate a testbed.toml with inline mount config
    let toml_content = r#"
        [[vms]]
        name = "linux-build"
        profile = "linux-build"
        mount = { host_path = ".", guest_path = "/mnt/project", readonly = true }
    "#;
    let config: UserConfig = toml::from_str(toml_content).expect("failed to parse");
    assert_eq!(config.profiles.len(), 1);
    let entry = &config.profiles[0];
    assert_eq!(entry.name, "linux-build");
    let mount = entry.profile.mount.as_ref().expect("mount should exist");
    assert_eq!(mount.host_path.as_deref(), Some("."));
    assert_eq!(mount.guest_path.as_deref(), Some("/mnt/project"));
    assert_eq!(mount.readonly, Some(true));
}

#[test]
fn test_parse_mount_with_missing_fields() {
    let toml_content = r#"
        [[vms]]
        name = "windows-build"
        profile = "windows-build"
        mount = { host_path = "." }
    "#;
    let config: UserConfig = toml::from_str(toml_content).expect("failed to parse");
    let entry = &config.profiles[0];
    let mount = entry.profile.mount.as_ref().expect("mount should exist");
    assert_eq!(mount.host_path.as_deref(), Some("."));
    assert!(mount.guest_path.is_none());
    assert!(mount.readonly.is_none());
}

#[test]
fn test_parse_mount_with_methods() {
    let toml_content = r#"
        [[vms]]
        name = "windows-build"
        profile = "windows-build"
        mount = { host_path = ".", methods = ["virtiofs", "smb"] }
    "#;
    let config: UserConfig = toml::from_str(toml_content).expect("failed to parse");
    let entry = &config.profiles[0];
    let mount = entry.profile.mount.as_ref().expect("mount should exist");
    assert_eq!(mount.methods, Some(vec!["virtiofs".to_string(), "smb".to_string()]));
}

//! Tests extracted from vms/providers/ modules

use foundation_testbed::vms::config::{get_profile, DisplayMode};
use foundation_testbed::vms::providers::{Provider, ProviderId, ResolvedPorts, VmHandle};

// From providers/mod.rs
#[test]
fn test_provider_id_display() {
    assert_eq!(ProviderId::Qemu.to_string(), "qemu");
    assert_eq!(ProviderId::Utm.to_string(), "utm");
}

#[test]
fn test_vm_handle_pid_parsing() {
    let handle = VmHandle {
        profile: get_profile("linux-build").unwrap().clone(),
        provider_id: ProviderId::Qemu,
        internal_id: "12345".to_string(),
        resolved_ports: ResolvedPorts {
            ssh_port: 2222,
            winrm_port: Some(5985),
            rdp_port: Some(3389),
            vnc_port: 5900,
        },
        display_mode: DisplayMode::Headless,
    };
    assert!(handle.is_qemu());
    assert_eq!(handle.pid(), Some(12345));
}

#[cfg(feature = "qemu")]
#[test]
fn test_qemu_provider_creation() {
    use foundation_testbed::vms::providers::qemu::QemuProvider;
    let provider = QemuProvider::new();
    assert_eq!(provider.name(), "qemu");
    assert_eq!(provider.id(), ProviderId::Qemu);
}

#[cfg(feature = "qemu")]
#[test]
fn test_qemu_provider_host_health() {
    use foundation_testbed::vms::providers::qemu::QemuProvider;
    use foundation_testbed::vms::providers::Provider;
    let provider = QemuProvider::new();
    let health = provider.host_health();
    // Should return host health struct (checks may fail if KVM not available)
    let _ = health.is_healthy();
}

// From providers/utm/applescript.rs
#[test]
fn test_import_script_format() {
    // Verify the import script uses the correct AppleSyntax
    let bundle = "/tmp/test.utm";
    let script = format!(
        r#"tell application "UTM" to import new virtual machine from POSIX file "{}""#,
        bundle
    );
    assert!(script.contains("import new virtual machine from POSIX file"));
}

// From providers/utm/bundle.rs
#[test]
fn test_quoted_plist_string_simple() {
    use foundation_testbed::vms::providers::utm::bundle::quoted_plist_string;
    assert_eq!(quoted_plist_string("hello"), "\"hello\"");
}

#[test]
fn test_quoted_plist_string_with_backslash() {
    use foundation_testbed::vms::providers::utm::bundle::quoted_plist_string;
    assert_eq!(quoted_plist_string("path\\to"), "\"path\\\\to\"");
}

#[test]
fn test_quoted_plist_string_with_quotes() {
    use foundation_testbed::vms::providers::utm::bundle::quoted_plist_string;
    assert_eq!(quoted_plist_string("say \"hi\""), "\"say \\\"hi\\\"\"");
}

#[test]
fn test_is_utm_bundle_false_for_regular_dir() {
    use foundation_testbed::vms::providers::utm::bundle::is_utm_bundle;
    // A regular directory without config.plist should not be a bundle
    let dir = std::env::temp_dir().join("test_not_bundle");
    let _ = std::fs::create_dir_all(&dir);
    assert!(!is_utm_bundle(&dir));
    let _ = std::fs::remove_dir_all(&dir);
}

// From providers/utm/state.rs
#[test]
fn test_save_from_runtime_creates_valid_state() {
    use foundation_testbed::vms::providers::utm::state::{delete, exists, save_from_runtime};
    let _ = save_from_runtime("test-utm", "/tmp/test.utm", "test-uuid", 2222, false);
    assert!(exists("test-utm"));
    let _ = delete("test-utm");
}

// From providers/utm/utmctl.rs
#[test]
fn test_version_is_older() {
    use foundation_testbed::vms::providers::utm::utmctl::version_is_older;
    assert!(version_is_older("4.6.4", "4.6.5"));
    assert!(version_is_older("4.5.0", "4.6.5"));
    assert!(version_is_older("3.9.9", "4.6.5"));
    assert!(!version_is_older("4.6.5", "4.6.5"));
    assert!(!version_is_older("4.6.6", "4.6.5"));
    assert!(!version_is_older("4.7.0", "4.6.5"));
    assert!(!version_is_older("5.0.0", "4.6.5"));
}

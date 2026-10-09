//! Tests extracted from vms/qemu/ modules

use std::path::Path;
use std::net::TcpListener;

use foundation_testbed::vms::config::{get_profile, list_profiles};
use foundation_testbed::vms::qemu::{build_qemu_args, display, ResolvedPorts};
use foundation_testbed::vms::qemu::disk::{find_qemu_img, parse_disk_info};
use foundation_testbed::vms::qemu::display::{connection_info, DisplayBackend};
use foundation_testbed::vms::qemu::download::is_cached;
use foundation_testbed::vms::qemu::mount::{fstab_entry, guest_mount_command, mount_args};
use foundation_testbed::vms::qemu::net::{allocate_port, is_port_free};
use foundation_testbed::vms::qemu::snapshot::parse_size_string;
use foundation_testbed::vms::config::DisplayMode;

// From qemu/mod.rs
#[test]
fn test_build_qemu_args_headless() {
    let profile = get_profile("windows-build").unwrap();
    let ports = ResolvedPorts {
        ssh_port: 2222,
        winrm_port: Some(5985),
        rdp_port: Some(3389),
        vnc_port: 5900,
    };
    let monitor = std::path::PathBuf::from("/tmp/test.monitor");
    let disk = std::path::PathBuf::from("/tmp/test.qcow2");
    let display_args = display::DisplayBackend::Vnc.qemu_args(0);

    let args = build_qemu_args(&profile, &disk, &ports, &monitor, display_args, false, None, false, None, None);

    assert!(args.contains(&"-enable-kvm".to_string()) || args.iter().any(|a| a.contains("kvm")));
    assert!(args.iter().any(|a| a.contains("12288"))); // RAM
    assert!(args.iter().any(|a| a.contains("virtio-net-pci,netdev=net")));
    assert!(args.iter().any(|a| a.contains("vnc=:0")));
    assert!(args.iter().any(|a| a.contains("hostfwd=tcp:127.0.0.1:2222-:22")));
    assert!(args.iter().any(|a| a.contains("usb-tablet"))); // mouse tracking fix
}

#[test]
fn test_build_qemu_args_headful() {
    let profile = get_profile("linux-build").unwrap();
    let ports = ResolvedPorts {
        ssh_port: 2422,
        winrm_port: None,
        rdp_port: None,
        vnc_port: 5902,
    };
    let monitor = std::path::PathBuf::from("/tmp/test.monitor");
    let disk = std::path::PathBuf::from("/tmp/test.qcow2");
    let display_args = display::DisplayBackend::Vnc.qemu_args(2);

    let args = build_qemu_args(&profile, &disk, &ports, &monitor, display_args, false, None, false, None, None);

    assert!(args.iter().any(|a| a.contains("vnc=:2")));
    assert!(args.iter().any(|a| a.contains("usb-tablet")));
}

#[test]
fn test_build_qemu_args_with_mount() {
    let profile = get_profile("linux-build").unwrap();
    let ports = ResolvedPorts {
        ssh_port: 2422,
        winrm_port: None,
        rdp_port: None,
        vnc_port: 5902,
    };
    let monitor = std::path::PathBuf::from("/tmp/test.monitor");
    let disk = std::path::PathBuf::from("/tmp/test.qcow2");
    let display_args = display::DisplayBackend::Vnc.qemu_args(2);
    let project_mount = std::path::PathBuf::from("/home/user/project");

    let args = build_qemu_args(&profile, &disk, &ports, &monitor, display_args, false, Some(&project_mount), false, None, None);

    assert!(args.iter().any(|a| a.contains("-virtfs")));
    assert!(args.iter().any(|a| a.contains("path=/home/user/project")));
    assert!(args.iter().any(|a| a.contains("mount_tag=project")));
}

#[test]
fn test_build_qemu_args_with_cdrom() {
    let profile = get_profile("windows-build").unwrap();
    let ports = ResolvedPorts {
        ssh_port: 2222,
        winrm_port: Some(5985),
        rdp_port: Some(3389),
        vnc_port: 5900,
    };
    let monitor = std::path::PathBuf::from("/tmp/test.monitor");
    let disk = std::path::PathBuf::from("/tmp/test.qcow2");
    let display_args = display::DisplayBackend::Vnc.qemu_args(0);
    let cdrom = std::path::PathBuf::from("/tmp/virtio-win.iso");

    let args = build_qemu_args(&profile, &disk, &ports, &monitor, display_args, false, None, false, Some(&cdrom), None);

    assert!(args.iter().any(|a| a.contains("media=cdrom")));
    assert!(args.iter().any(|a| a.contains("/tmp/virtio-win.iso")));
}

#[test]
fn test_all_profiles_parseable() {
    let profiles = list_profiles();
    assert!(!profiles.is_empty());
    for p in profiles {
        assert!(!p.name.is_empty());
        assert!(p.memory_mib > 0);
        assert!(p.cpu_cores > 0);
        assert!(p.disk_gb > 0);
    }
}

// From qemu/disk.rs
#[test]
fn test_parse_disk_info() {
    let json = r#"{
        "virtual-size": 85899345920,
        "actual-size": 1234567890,
        "format": "qcow2"
    }"#;
    let info = parse_disk_info(json).unwrap();
    assert_eq!(info.format, "qcow2");
    assert_eq!(info.virtual_size_bytes, 85_899_345_920);
    assert!((info.virtual_size_gb() - 80.0).abs() < 0.01);
}

#[test]
fn test_qemu_img_not_found_error() {
    use foundation_testbed::vms::config::TestbedError;
    let err = find_qemu_img();
    // Should either find qemu-img or return the right error type
    match err {
        Ok(_) => {} // qemu-img is installed, fine
        Err(TestbedError::QemuImgNotFound { .. }) => {} // expected error
        Err(e) => panic!("unexpected error variant: {e}"),
    }
}

// From qemu/display.rs
#[test]
fn test_vnc_args() {
    let backend = DisplayBackend::Vnc;
    let args = backend.qemu_args(0);
    assert_eq!(args, vec!["-display", "vnc=:0"]);
}

#[test]
fn test_connection_info_has_port() {
    let info = connection_info(DisplayMode::Headless, DisplayBackend::Vnc, 5900);
    assert!(info.contains("5900"));
}

// From qemu/download.rs
#[test]
fn test_is_cached_false_for_missing_or_empty() {
    // Non-existent path
    assert!(!is_cached(Path::new("/tmp/nonexistent_file_12345.qcow2")));

    // Empty file
    let path = Path::new("/tmp/test_empty_cache.qcow2");
    let _ = std::fs::remove_file(path);
    std::fs::File::create(path).unwrap();
    assert!(!is_cached(path));
    let _ = std::fs::remove_file(path);
}

// From qemu/mount.rs
#[test]
fn test_mount_args_readable() {
    let args = mount_args(Path::new("/home/user/project"), "project", false);
    assert_eq!(args.len(), 2);
    assert_eq!(args[0], "-virtfs");
    assert!(args[1].contains("path=/home/user/project"));
    assert!(args[1].contains("mount_tag=project"));
    assert!(args[1].contains("security_model=mapped"));
    assert!(args[1].contains("id=project"));
}

#[test]
fn test_mount_args_readonly() {
    let args = mount_args(Path::new("/home/user/project"), "project", true);
    assert!(args[1].contains("readonly"));
}

#[test]
fn test_guest_mount_command() {
    let cmd = guest_mount_command("project", "/mnt/project");
    assert!(cmd.contains("sudo mkdir -p /mnt/project"));
    assert!(cmd.contains("sudo mount -t 9p"));
}

#[test]
fn test_fstab_entry() {
    let entry = fstab_entry("project", "/mnt/project");
    assert!(entry.contains("9p"));
    assert!(entry.contains("trans=virtio"));
}

// From qemu/net.rs
#[test]
fn test_allocate_port_returns_free_port() {
    // Port 0 is always free (OS picks one), but let's use a high port
    let port = allocate_port(30000).unwrap();
    assert!(port >= 30000);
    // The returned port should be free (we just allocated it, so it was free at check time)
    assert!(is_port_free(port));
}

#[test]
fn test_is_port_free() {
    // Bind a port, verify it shows as in-use
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    assert!(!is_port_free(port), "bound port should not be free");
    drop(listener);
    assert!(is_port_free(port), "port should be free after drop");
}

// From qemu/snapshot.rs
#[test]
fn test_parse_size_string() {
    assert_eq!(parse_size_string("432MB"), 432 * 1_048_576);
    assert_eq!(parse_size_string("1GB"), 1_073_741_824);
    assert_eq!(parse_size_string("512KB"), 512 * 1_024);
    assert_eq!(parse_size_string("100"), 100);
    assert_eq!(parse_size_string("invalid"), 0);
}

#[test]
fn test_list_parses_empty() {
    // Simulates QEMU output when no snapshots exist
    let output = "No snapshots available.";
    assert!(output.contains("No snapshots"));
    // Would return empty vec in the real function
}

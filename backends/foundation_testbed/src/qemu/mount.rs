//! QEMU 9p/virtio filesystem mount configuration.
//!
//! Mounts host directories into guest VMs via virtio-9p protocol.

use std::path::Path;

/// Build QEMU `-virtfs` arguments for mounting a host directory.
///
/// Uses the Plan 9 filesystem protocol over virtio (`virtio-9p`).
/// The `guest_tag` is the name used inside the VM to identify the mount.
///
/// Inside the VM: `mount -t 9p -o trans=virtio,version=9p2000.L <tag> /mnt/project`
pub fn mount_args(host_path: &Path, guest_tag: &str, readonly: bool) -> Vec<String> {
    let security = if readonly {
        "mapped,readonly"
    } else {
        "mapped"
    };
    vec![
        "-virtfs".to_string(),
        format!(
            "local,path={},mount_tag={},security_model={},id={}",
            host_path.display(),
            guest_tag,
            security,
            guest_tag,
        ),
    ]
}

/// Default guest mount path for project directories.
pub const DEFAULT_GUEST_PATH: &str = "/mnt/project";

/// Default mount tag name.
pub const DEFAULT_TAG: &str = "project";

/// Generate the mount command to run inside the guest.
pub fn guest_mount_command(tag: &str, guest_path: &str) -> String {
    format!(
        "sudo mkdir -p {guest_path} && \
         sudo mount -t 9p -o trans=virtio,version=9p2000.L {tag} {guest_path}"
    )
}

/// Generate an fstab entry for persistent mounts across VM reboots.
pub fn fstab_entry(tag: &str, guest_path: &str) -> String {
    format!("{tag} {guest_path} 9p trans=virtio,version=9p2000.L 0 0")
}

/// Verify mount is active inside the guest (returns a command to run).
pub fn verify_mount_command(guest_path: &str) -> String {
    format!("mount | grep -q {guest_path} && echo OK || echo MISSING")
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

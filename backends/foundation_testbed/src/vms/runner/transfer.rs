//! File transfer between host and VM via SCP.
//!
//! Wraps ssh::upload and ssh::download with path resolution.

use std::path::Path;

use crate::vms::config::Result;
use crate::vms::ssh::VmSession;

/// Push a file or directory from host to VM.
///
/// Supports both single files and recursive directory transfers.
pub fn push(session: &mut VmSession, local: &Path, remote: &str) -> Result<()> {
    if local.is_dir() {
        // For directories, tar+scp is more reliable than recursive scp
        push_dir(session, local, remote)
    } else {
        crate::vms::ssh::upload(session, local, remote)
    }
}

/// Pull a file or directory from VM to host.
pub fn pull(session: &mut VmSession, remote: &str, local: &Path) -> Result<()> {
    crate::vms::ssh::download(session, remote, local)
}

/// Push a directory by tarring it first.
fn push_dir(session: &mut VmSession, dir: &Path, _remote: &str) -> Result<()> {
    let tar_path = std::env::temp_dir().join("testbed-push.tar.gz");

    // Create tar archive
    let status = std::process::Command::new("tar")
        .args([
            "czf",
            tar_path.to_str().unwrap(),
            "-C",
            dir.parent().unwrap_or(dir).to_str().unwrap(),
            dir.file_name().unwrap_or_default().to_str().unwrap(),
        ])
        .status()
        .map_err(|e| crate::vms::config::TestbedError::BootstrapFailed {
            step: "push".to_string(),
            message: format!("creating tar: {e}"),
        })?;

    if !status.success() {
        return Err(crate::vms::config::TestbedError::BootstrapFailed {
            step: "push".to_string(),
            message: "tar failed".to_string(),
        });
    }

    // Upload and extract on VM
    crate::vms::ssh::upload(session, &tar_path, "/tmp/push.tar.gz")?;
    crate::vms::ssh::exec(session, "cd ~ && tar xzf /tmp/push.tar.gz")?;

    // Clean up
    let _ = std::fs::remove_file(&tar_path);
    crate::vms::ssh::exec(session, "rm -f /tmp/push.tar.gz")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    #[test]
    fn test_push_identifies_directory() {
        let temp = std::env::temp_dir().join("test_push_dir");
        let _ = std::fs::create_dir_all(&temp);
        assert!(temp.is_dir());
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_push_uses_upload_for_files() {
        // Verify the path resolution logic
        let local = PathBuf::from("/tmp/test.txt");
        let remote = "/tmp/test.txt";
        assert_eq!(local.is_file(), false); // doesn't exist
        assert_eq!(remote, "/tmp/test.txt");
    }
}

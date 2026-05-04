//! Artifact mirroring — mirrors build outputs from the mounted directory
//! to the `.testbed/artifacts/` directory on the host.
//!
//! Since builds run inside VMs with the host project directory mounted,
//! artifacts are already on the host filesystem. This module organizes
//! them into a platform-scoped layout and optionally creates symlinks
//! for instant access.

use std::path::{Path, PathBuf};

use crate::config::{GuestOs, Result, TestbedError};

/// Mirror build outputs from the host project's target directory into
/// the `.testbed/artifacts/<platform>/` directory.
///
/// Since the project is mounted, artifacts are already on the host.
/// This function creates a structured view under `.testbed/artifacts/`
/// using symlinks (instant, no copy).
pub fn mirror_artifacts(
    project_dir: &Path,
    target_triple: &str,
    os: GuestOs,
) -> Result<Vec<PathBuf>> {
    let artifacts_dir = project_dir.join(".testbed").join("artifacts");
    std::fs::create_dir_all(&artifacts_dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating artifacts dir: {e}"),
    })?;

    let platform_dir = match os {
        GuestOs::Windows => artifacts_dir.join("windows"),
        GuestOs::Linux => artifacts_dir.join("linux"),
        GuestOs::MacOS => artifacts_dir.join("macos"),
    };

    let _ = std::fs::remove_dir_all(&platform_dir); // remove old symlink
    std::fs::create_dir_all(&platform_dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating platform dir: {e}"),
    })?;

    // Scan for actual build outputs
    let build_dir = project_dir.join("target").join(target_triple).join("release");
    if !build_dir.exists() {
        return Ok(Vec::new());
    }

    let artifacts = scan_artifacts(&build_dir, os)?;
    for artifact in &artifacts {
        if let Some(file_name) = artifact.file_name() {
            let link_path = platform_dir.join(file_name);
            // Create symlink to the actual artifact
            #[cfg(unix)]
            std::os::unix::fs::symlink(artifact, &link_path).map_err(|e| TestbedError::Qcow2Error {
                message: format!("creating symlink: {e}"),
            })?;
        }
    }

    Ok(artifacts)
}

/// Scan a build directory for artifact files (executables, shared libs).
fn scan_artifacts(dir: &Path, os: GuestOs) -> Result<Vec<PathBuf>> {
    let mut artifacts = Vec::new();

    let entries = std::fs::read_dir(dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("reading build dir: {e}"),
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| TestbedError::Qcow2Error {
            message: format!("reading dir entry: {e}"),
        })?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        match os {
            GuestOs::Windows => {
                if name_str.ends_with(".exe") || name_str.ends_with(".dll") {
                    artifacts.push(path);
                }
            }
            GuestOs::Linux | GuestOs::MacOS => {
                let has_no_ext = path.extension().is_none();
                let is_so = name_str.ends_with(".so");
                if has_no_ext || is_so {
                    artifacts.push(path);
                }
            }
        }
    }

    Ok(artifacts)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

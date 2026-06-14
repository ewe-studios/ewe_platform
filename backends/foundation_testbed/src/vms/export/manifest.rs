//! Export manifest generation — metadata about exported VM images.
//!
//! Each export produces a `manifest.json` alongside the qcow2 file containing
//! OS info, image size, SHA256, installed tools, and creation metadata.

use std::path::Path;

use serde::Serialize;

use crate::vms::config::{GuestOs, Result, TestbedError, VmProfile};

/// Manifest written alongside an exported VM image.
#[derive(Debug, Serialize)]
pub struct ExportManifest {
    pub name: String,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub image_file: String,
    pub image_size_bytes: u64,
    pub image_sha256: String,
    pub bootstrap_version: u32,
    pub installed_tools: std::collections::HashMap<String, String>,
    pub created_at: String,
    pub created_by: String,
    pub git_commit: String,
    pub notes: String,
}

impl ExportManifest {
    /// Build a manifest from a profile, exported image path, and optional tool versions.
    pub fn new(
        profile: &VmProfile,
        image_path: &Path,
        version: &str,
        installed_tools: std::collections::HashMap<String, String>,
        bootstrap_version: u32,
        notes: &str,
    ) -> Result<Self> {
        let meta = std::fs::metadata(image_path).map_err(|e| TestbedError::Qcow2Error {
            message: format!("reading exported image metadata: {e}"),
        })?;

        let sha256 = compute_sha256(image_path)?;
        let image_file = image_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "export.qcow2".to_string());

        let arch = std::env::consts::ARCH.to_string();
        let os_str = match profile.os {
            GuestOs::Windows => "windows".to_string(),
            GuestOs::Linux => "linux".to_string(),
            GuestOs::MacOS => "macos".to_string(),
        };

        let created_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let created_by = whoami::hostname().unwrap_or("unknown".to_string());
        let git_commit = get_git_commit();

        Ok(Self {
            name: profile.name.to_string(),
            version: version.to_string(),
            os: os_str,
            arch,
            image_file,
            image_size_bytes: meta.len(),
            image_sha256: sha256,
            bootstrap_version,
            installed_tools,
            created_at,
            created_by,
            git_commit,
            notes: notes.to_string(),
        })
    }

    /// Write the manifest to `<image_path>.manifest.json` (sibling file).
    pub fn write_to(&self, image_path: &Path) -> Result<()> {
        let manifest_path = image_path.with_extension("qcow2.manifest.json");
        let json = serde_json::to_string_pretty(self).map_err(|e| TestbedError::Qcow2Error {
            message: format!("serializing manifest: {e}"),
        })?;
        std::fs::write(&manifest_path, json).map_err(|e| TestbedError::Qcow2Error {
            message: format!("writing manifest: {e}"),
        })?;
        Ok(())
    }

    /// Display a summary of installed tools for import feedback.
    pub fn tools_summary(&self) -> String {
        if self.installed_tools.is_empty() {
            return "no tools detected".to_string();
        }
        let mut tools: Vec<_> = self.installed_tools.iter().collect();
        tools.sort_by_key(|(k, _)| (*k).clone());
        tools
            .iter()
            .map(|(name, version)| format!("{name} {version}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn compute_sha256(path: &Path) -> Result<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| TestbedError::Qcow2Error {
        message: format!("opening image for SHA256: {e}"),
    })?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| TestbedError::Qcow2Error {
            message: format!("reading image for SHA256: {e}"),
        })?;
        if n == 0 { break; }
        use sha2::Digest;
        hasher.update(&buf[..n]);
    }
    use sha2::Digest;
    Ok(format!("{:x}", hasher.finalize()))
}

fn get_git_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_tools_summary() {
        let mut tools = std::collections::HashMap::new();
        tools.insert("rust".to_string(), "1.78.0".to_string());
        tools.insert("node".to_string(), "22.1.0".to_string());

        let manifest = ExportManifest {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            image_file: "test.qcow2".to_string(),
            image_size_bytes: 1_000_000,
            image_sha256: "abc123".to_string(),
            bootstrap_version: 3,
            installed_tools: tools,
            created_at: "2026-05-03T00:00:00Z".to_string(),
            created_by: "testhost".to_string(),
            git_commit: "abc1234".to_string(),
            notes: "".to_string(),
        };

        let summary = manifest.tools_summary();
        assert!(summary.contains("node 22.1.0"));
        assert!(summary.contains("rust 1.78.0"));
    }
}

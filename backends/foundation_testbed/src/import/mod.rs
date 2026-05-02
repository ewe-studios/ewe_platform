//! Image import orchestrator — manages VM disk images in the cache.
//!
//! Coordinates downloading, validation, and caching of pre-built qcow2
//! images. Supports direct URLs and Vagrant Cloud as sources.
//! Automatically extracts qcow2 from vagrant box archives (tar or gzip).

use std::path::Path;

use crate::config::{Result, TestbedError, VmProfile};

use crate::qemu::download;

/// Minimum image size to consider a download valid (100 MB).
const MIN_IMAGE_SIZE: u64 = 100 * 1_048_576;

/// Ensure the image for a profile is available in the cache.
///
/// If the image already exists, returns the cached path. Otherwise
/// downloads it from the profile's `prebaked_url` or a default URL.
/// Handles both direct qcow2 downloads and vagrant box extraction.
pub fn ensure_image(profile: &VmProfile) -> Result<std::path::PathBuf> {
    let dest = profile.image_cache_path();

    // Already cached
    if download::is_cached(&dest) {
        return Ok(dest);
    }

    // Determine source URL
    let url = resolve_image_url(profile)?;

    // Download to a temp location first (in case it's a vagrant box)
    let temp_dest = dest.with_extension("downloading");
    download::download(&url, &temp_dest)?;

    // Check if it's a vagrant box and extract qcow2
    if is_vagrant_box(&temp_dest) {
        extract_qcow2_from_box(&temp_dest, &dest)?;
        let _ = std::fs::remove_file(&temp_dest);
    } else {
        std::fs::rename(&temp_dest, &dest).map_err(|e| TestbedError::Qcow2Error {
            message: format!("moving downloaded image to {dest:?}: {e}"),
        })?;
    }

    // Validate size
    validate_image(&dest)?;

    Ok(dest)
}

/// Download an image from a specific URL.
pub fn download_from_url(url: &str, dest: &Path) -> Result<()> {
    download::download(url, dest)?;
    validate_image(dest)?;
    Ok(())
}

/// Check if a profile's image is already cached.
pub fn is_cached(profile: &VmProfile) -> bool {
    let dest = profile.image_cache_path();
    download::is_cached(&dest)
}

/// Remove a cached image for a profile.
pub fn evict(profile: &VmProfile) -> Result<()> {
    let dest = profile.image_cache_path();
    if dest.exists() {
        std::fs::remove_file(&dest).map_err(|e| TestbedError::Qcow2Error {
            message: format!("removing {dest:?}: {e}"),
        })?;
    }
    Ok(())
}

/// Get the size of a cached image.
pub fn cached_size(profile: &VmProfile) -> Result<u64> {
    let dest = profile.image_cache_path();
    let metadata = std::fs::metadata(&dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("stat {dest:?}: {e}"),
    })?;
    Ok(metadata.len())
}

// ── Vagrant box detection and extraction ─────────────────────────────────────

/// Check if a downloaded file looks like a vagrant box (tar or gzip-compressed tar).
fn is_vagrant_box(path: &Path) -> bool {
    // Check for gzip magic: 0x1f 0x8b
    if let Ok(mut f) = std::fs::File::open(path) {
        let mut buf = [0u8; 2];
        if std::io::Read::read_exact(&mut f, &mut buf).is_ok() {
            if buf[0] == 0x1f && buf[1] == 0x8b {
                return true;
            }
        }
    }
    // Fallback: try tar -tf and see if it works
    std::process::Command::new("tar")
        .args(["-tf", path.to_str().unwrap()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Extract the qcow2 disk image from a vagrant box archive.
///
/// Vagrant boxes can be plain tar or gzip-compressed tar. They typically
/// contain `box.img`, `box_0.img`, or `*.qcow2` as the disk image.
fn extract_qcow2_from_box(box_path: &Path, dest: &Path) -> Result<()> {
    // List entries to find the disk image
    let output = std::process::Command::new("tar")
        .arg("-tf")
        .arg(box_path)
        .output()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("listing vagrant box contents: {e}"),
        })?;

    if !output.status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!("failed to list vagrant box: {}", String::from_utf8_lossy(&output.stderr)),
        });
    }

    // Find the disk image entry
    let entries: String = String::from_utf8_lossy(&output.stdout).to_string();
    let disk_entry = entries.lines().find(|name| {
        let n = name.trim();
        n == "box.img" || n == "box_0.img" || n.ends_with(".qcow2") || n.ends_with(".img")
    }).ok_or_else(|| TestbedError::Qcow2Error {
        message: "no disk image found in vagrant box (expected box.img, box_0.img, *.qcow2, or *.img)".to_string(),
    })?;

    // Extract the specific file to a temp location
    let temp_extract = dest.with_extension("extracting");
    let out_file = std::fs::File::create(&temp_extract).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating {temp_extract:?}: {e}"),
    })?;

    let status = std::process::Command::new("tar")
        .args(["-xf", box_path.to_str().unwrap(), "-O", disk_entry])
        .stdout(out_file)
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("extracting disk image: {e}"),
        })?;

    if !status.success() {
        let _ = std::fs::remove_file(&temp_extract);
        return Err(TestbedError::Qcow2Error {
            message: "failed to extract disk image from vagrant box".to_string(),
        });
    }

    // Move to final destination
    std::fs::rename(&temp_extract, dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("moving extracted disk to {dest:?}: {e}"),
    })?;

    Ok(())
}

// ── URL resolution ──────────────────────────────────────────────────────────

/// Resolve the image URL for a profile.
///
/// Priority: profile.prebaked_url → Vagrant Cloud lookup → error.
fn resolve_image_url(profile: &VmProfile) -> Result<String> {
    if let Some(url) = profile.prebaked_url {
        return Ok(url.to_string());
    }

    // Check Vagrant Cloud for this image
    if let Some(url) = query_vagrant_cloud(profile) {
        return Ok(url);
    }

    Err(TestbedError::DownloadFailed {
        status: 0,
        url: format!("no image source for profile '{}'", profile.name),
    })
}

/// Query Vagrant Cloud for a libvirt download URL.
///
/// Vagrant Cloud API: GET /api/v2/box/{username}/{box_name}
fn query_vagrant_cloud(profile: &VmProfile) -> Option<String> {
    let vagrant_box = match profile.image_name {
        "windows-11-x86_64.qcow2" => Some("gusztavvargadr/windows-11"),
        "ubuntu-24.04-x86_64.qcow2" => Some("alvistack/ubuntu-24.04"),
        _ => None,
    };

    let box_name = vagrant_box?;
    let api_url = format!("https://app.vagrantup.com/api/v2/box/{box_name}");

    // Use curl subprocess to avoid tokio runtime conflicts
    let output = std::process::Command::new("curl")
        .args(["-s", "-L", "--max-redirs", "5", "-m", "30", &api_url])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;

    // Find libvirt provider
    if let Some(providers) = json.get("current_version")?.get("providers")?.as_array() {
        for provider in providers {
            if provider.get("name")?.as_str()? == "libvirt" {
                if let Some(dl) = provider.get("download_url")?.as_str() {
                    return Some(dl.to_string());
                }
                if let Some(url) = provider.get("url")?.as_str() {
                    return Some(url.to_string());
                }
            }
        }
    }

    None
}

// ── Validation ──────────────────────────────────────────────────────────────

/// Validate that a downloaded image looks legitimate.
fn validate_image(path: &Path) -> Result<()> {
    let metadata = std::fs::metadata(path).map_err(|e| TestbedError::Qcow2Error {
        message: format!("stat {path:?}: {e}"),
    })?;

    let size = metadata.len();
    if size < MIN_IMAGE_SIZE {
        return Err(TestbedError::Qcow2Error {
            message: format!(
                "image at {path:?} is only {} bytes (< {} MB minimum) — likely a failed download",
                size,
                MIN_IMAGE_SIZE / 1_048_576
            ),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

//! Image import orchestrator — manages VM disk images in the cache.
//!
//! Coordinates downloading, validation, and caching of pre-built qcow2
//! images. Supports direct URLs and Vagrant Cloud as sources.

use std::path::Path;

use crate::config::{Result, TestbedError, VmProfile};

use crate::qemu::download;

/// Minimum image size to consider a download valid (100 MB).
const MIN_IMAGE_SIZE: u64 = 100 * 1_048_576;

/// Ensure the image for a profile is available in the cache.
///
/// If the image already exists, returns the cached path. Otherwise
/// downloads it from the profile's `prebaked_url` or a default URL.
pub fn ensure_image(profile: &VmProfile) -> Result<std::path::PathBuf> {
    let dest = profile.image_cache_path();

    // Already cached
    if download::is_cached(&dest) {
        return Ok(dest);
    }

    // Determine source URL
    let url = resolve_image_url(profile)?;

    // Download
    download::download(&url, &dest)?;

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

// ── URL resolution ──────────────────────────────────────────────────────────

/// Resolve the image URL for a profile.
///
/// Priority: profile.prebaked_url → hardcoded defaults → error.
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

/// Query Vagrant Cloud for a qcow2 download URL.
///
/// Vagrant Cloud API: GET /vagrant/2022-09-30/registry/{provider}/box/{box_name}/versions
/// We check known Vagrant boxes for qcow2 images.
fn query_vagrant_cloud(profile: &VmProfile) -> Option<String> {
    // Map profile image names to Vagrant Cloud boxes
    let vagrant_box = match profile.image_name {
        "windows-11-x86_64.qcow2" => None, // Windows boxes aren't on Vagrant Cloud
        "ubuntu-24.04-x86_64.qcow2" => Some("generic/ubuntu2404"),
        _ => None,
    };

    let box_name = vagrant_box?;

    let url = format!(
        "https://app.vagrantup.com/api/v2/box/{box_name}"
    );

    // Quick HTTP fetch (blocking)
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .ok()?;

    let response = client.get(&url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }

    let json: serde_json::Value = response.json().ok()?;

    // Find a provider with qcow2 format
    if let Some(providers) = json.get("current_version")?.get("providers")?.as_array() {
        for provider in providers {
            if provider.get("name")?.as_str()? == "libvirt" {
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
        // Create a small file
        let path = std::path::PathBuf::from("/tmp/test_small_image.qcow2");
        std::fs::write(&path, b"tiny").unwrap();
        let err = validate_image(&path).unwrap_err();
        assert!(err.to_string().contains("failed download"));
        let _ = std::fs::remove_file(&path);
    }
}

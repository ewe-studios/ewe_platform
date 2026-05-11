//! Image import orchestrator — manages VM disk images in the cache.
//!
//! Coordinates downloading, validation, and caching of pre-built qcow2
//! images. Supports direct URLs and Vagrant Cloud as sources.
//! Automatically extracts qcow2 from vagrant box archives (tar or gzip).

use std::path::{Path, PathBuf};

use crate::config::{Result, TestbedError, VmProfile};

use crate::qemu::download;

pub mod macos;

/// Minimum image size to consider a download valid (100 MB).
const MIN_IMAGE_SIZE: u64 = 100 * 1_048_576;

/// Known compressed image extensions we can auto-decompress.
const COMPRESSED_EXTENSIONS: &[&str] = &["qcow2.gz", "qcow2.xz"];

/// Resolve the image path for a profile.
///
/// Priority:
/// 1. `TESTBED_IMAGE_{PROFILE}` env var (e.g. `TESTBED_IMAGE_WINDOWS_BUILD=/path/to/image.qcow2`)
/// 2. Cached image in `~/.cache/foundation_testbed/images/`
/// 3. Image stores (testbed.toml `[[image_stores]]`) — download if available
/// 4. Download from profile `prebaked_url` or Vagrant Cloud
///
/// The environment variable must point to a valid file (qcow2 or compressed
/// qcow2). It is used as-is — no download, no extraction, no copy.
pub fn ensure_image(profile: &VmProfile) -> Result<std::path::PathBuf> {
    // macOS uses a different layout (directory with BaseSystem.qcow2 + opencore.qcow2 + data disk)
    // Try image stores / prebaked tarball first, then fall back to native IPSW creation.
    if profile.name.starts_with("macos") {
        return ensure_macos_from_store_or_fallback(profile);
    }

    // 1. Check environment variable override
    let env_key = format!("TESTBED_IMAGE_{}", profile.name.to_uppercase().replace('-', "_"));
    if let Ok(image_path) = std::env::var(&env_key) {
        let path = PathBuf::from(&image_path);
        if !path.exists() {
            return Err(TestbedError::DownloadFailed {
                status: 0,
                url: format!("TESTBED_IMAGE_{} points to non-existent file: {}", env_key, image_path),
            });
        }
        let meta = std::fs::metadata(&path).map_err(|e| TestbedError::Qcow2Error {
            message: format!("stat {}: {e}", path.display()),
        })?;
        if meta.len() < MIN_IMAGE_SIZE {
            return Err(TestbedError::Qcow2Error {
                message: format!("image at {} too small ({} bytes) — likely corrupted", path.display(), meta.len()),
            });
        }
        // If it's a compressed archive, decompress to cache first
        if is_compressed_qcow2(&path) {
            println!("  Decompressing {} from override path...", path.extension().unwrap_or_default().to_str().unwrap_or("unknown"));
            let dest = profile.image_cache_path();
            crate::export::decompress(&path, &dest)?;
            return Ok(dest);
        }
        println!("  Using image from override ({}): {}", env_key, path.display());
        return Ok(path);
    }

    let dest = profile.image_cache_path();

    // 2. Already cached (check for compressed variants too)
    if let Some(cached) = find_cached_image(&dest) {
        return crate::export::ensure_decompressed(&cached);
    }

    // 3. Check image stores for a pre-baked image
    if let Some(store_path) = try_download_from_store(profile, &dest)? {
        return Ok(store_path);
    }

    // 4. Download from profile prebaked_url or Vagrant Cloud
    let url = resolve_image_url(profile)?;

    let temp_dest = dest.with_extension("downloading");
    download::download(&url, &temp_dest)?;

    if is_compressed_qcow2(&temp_dest) {
        println!("  Decompressing {} archive...", temp_dest.extension().unwrap_or_default().to_str().unwrap_or("unknown"));
        crate::export::decompress(&temp_dest, &dest)?;
        let _ = std::fs::remove_file(&temp_dest);
    } else if is_vagrant_box(&temp_dest) {
        extract_qcow2_from_box(&temp_dest, &dest)?;
        let _ = std::fs::remove_file(&temp_dest);
    } else {
        std::fs::rename(&temp_dest, &dest).map_err(|e| TestbedError::Qcow2Error {
            message: format!("moving downloaded image to {dest:?}: {e}"),
        })?;
    }

    validate_image(&dest)?;

    Ok(dest)
}

/// Try to download a profile image from configured image stores.
///
/// Checks each store for `{profile_name}.qcow2` (and compressed variants).
/// Returns `Some(path)` if downloaded successfully, `None` if no store
/// has the image, or errors if download fails.
fn try_download_from_store(profile: &VmProfile, dest: &Path) -> Result<Option<PathBuf>> {
    let stores = crate::config::load_image_stores();
    if stores.is_empty() {
        return Ok(None);
    }

    let filename = format!("{}.qcow2", profile.name);

    for store in &stores {
        let Some(url) = store_download_url_direct(store, &filename) else { continue };
        let url_path = Path::new(&url);

        // For local stores, just check if the file exists
        if url_path.exists() {
            println!("  Found image in local store '{}': {}", store.name, url);
            std::fs::copy(url_path, dest).map_err(|e| TestbedError::Qcow2Error {
                message: format!("copying from local store: {e}"),
            })?;
            return Ok(Some(dest.to_path_buf()));
        }

        // For HTTP stores, try to download
        if url.starts_with("http://") || url.starts_with("https://") {
            println!("  Trying store '{}' → {}...", store.name, url);
            let temp_dest = dest.with_extension("downloading");
            if download::download(&url, &temp_dest).is_ok() {
                std::fs::rename(&temp_dest, dest).ok();
                if validate_image(dest).is_ok() {
                    return Ok(Some(dest.to_path_buf()));
                }
            }
            let _ = std::fs::remove_file(&temp_dest);
        }
    }

    Ok(None)
}

/// Build a direct download URL for a store + filename.
/// Only works for Http and Local store types.
fn store_download_url_direct(store: &crate::config::ImageStore, filename: &str) -> Option<String> {
    match store.store_type {
        crate::config::StoreType::Http => {
            Some(format!("{}/{}", store.destination.trim_end_matches('/'), filename))
        }
        crate::config::StoreType::Local => {
            Some(format!("{}/{}", store.destination.trim_end_matches('/'), filename))
        }
        crate::config::StoreType::R2 | crate::config::StoreType::S3 => {
            // Cloud stores require rclone/aws CLI — not supported for simple download
            None
        }
    }
}

/// Find a cached image, checking for compressed variants.
///
/// Returns the first valid file found: dest, dest.gz, dest.xz.
fn find_cached_image(dest: &std::path::Path) -> Option<std::path::PathBuf> {
    // Check uncompressed first
    if download::is_cached(dest) {
        return Some(dest.to_path_buf());
    }

    // Check compressed variants
    for ext in COMPRESSED_EXTENSIONS {
        let compressed = dest.with_extension(ext);
        if compressed.exists() {
            let meta = std::fs::metadata(&compressed).ok()?;
            if meta.len() > MIN_IMAGE_SIZE {
                return Some(compressed);
            }
        }
    }

    None
}

/// Check if a file looks like a compressed qcow2 archive (by extension + magic).
fn is_compressed_qcow2(path: &std::path::Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "gz" | "xz" => true,
        _ => false,
    }
}

/// Download an image from a specific URL.
pub fn download_from_url(url: &str, dest: &Path) -> Result<()> {
    download::download(url, dest)?;
    validate_image(dest)?;
    Ok(())
}

/// Check if a profile's image is already cached.
pub fn is_cached(profile: &VmProfile) -> bool {
    // Check env var override first
    let env_key = format!("TESTBED_IMAGE_{}", profile.name.to_uppercase().replace('-', "_"));
    if let Ok(path) = std::env::var(&env_key) {
        return PathBuf::from(&path).exists();
    }

    if profile.name.starts_with("macos") {
        let macos_dir = crate::config::image_cache_dir().join(format!("macos-{}", profile.name));
        macos_dir.join("BaseSystem.qcow2").exists()
            && macos_dir.join(macos::OPENCORE_EFI_FILENAME).exists()
            && macos_dir.join("macOS-data.qcow2").exists()
    } else {
        let dest = profile.image_cache_path();
        download::is_cached(&dest)
            || COMPRESSED_EXTENSIONS.iter().any(|ext| {
                dest.with_extension(ext).exists()
            })
    }
}

/// Remove a cached image for a profile.
pub fn evict(profile: &VmProfile) -> Result<()> {
    // Don't evict env-var override images — the file is user-managed
    let env_key = format!("TESTBED_IMAGE_{}", profile.name.to_uppercase().replace('-', "_"));
    if std::env::var(&env_key).is_ok() {
        return Ok(());
    }

    if profile.name.starts_with("macos") {
        let macos_dir = crate::config::image_cache_dir().join(format!("macos-{}", profile.name));
        if macos_dir.exists() {
            std::fs::remove_dir_all(&macos_dir).map_err(|e| TestbedError::Qcow2Error {
                message: format!("removing {macos_dir:?}: {e}"),
            })?;
        }
        Ok(())
    } else {
        let dest = profile.image_cache_path();
        // Remove all variants (uncompressed + compressed)
        let mut removed = false;
        if dest.exists() {
            std::fs::remove_file(&dest).map_err(|e| TestbedError::Qcow2Error {
                message: format!("removing {dest:?}: {e}"),
            })?;
            removed = true;
        }
        for ext in COMPRESSED_EXTENSIONS {
            let compressed = dest.with_extension(ext);
            if compressed.exists() {
                std::fs::remove_file(&compressed).map_err(|e| TestbedError::Qcow2Error {
                    message: format!("removing {compressed:?}: {e}"),
                })?;
                removed = true;
            }
        }
        if !removed {
            // Nothing to remove — not an error
        }
        Ok(())
    }
}

/// Get the size of a cached image.
pub fn cached_size(profile: &VmProfile) -> Result<u64> {
    // Check env var override first
    let env_key = format!("TESTBED_IMAGE_{}", profile.name.to_uppercase().replace('-', "_"));
    if let Ok(path) = std::env::var(&env_key) {
        let meta = std::fs::metadata(&path).map_err(|e| TestbedError::Qcow2Error {
            message: format!("stat {path}: {e}"),
        })?;
        return Ok(meta.len());
    }

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
        if std::io::Read::read_exact(&mut f, &mut buf).is_ok()
            && buf[0] == 0x1f && buf[1] == 0x8b {
                return true;
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

/// Ensure a macOS image from image stores, falling back to native IPSW creation.
///
/// Checks for a pre-baked tarball (`.tar.gz` containing BaseSystem.qcow2,
/// opencore.qcow2, macOS-data.qcow2) from image stores first. If no tarball
/// is found, falls back to `macos::ensure_macos_image()` for native IPSW creation.
fn ensure_macos_from_store_or_fallback(profile: &VmProfile) -> Result<PathBuf> {
    let macos_dir = crate::config::image_cache_dir().join(format!("macos-{}", profile.name));
    std::fs::create_dir_all(&macos_dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating macos image dir: {e}"),
    })?;

    let basesystem_qcow2 = macos_dir.join("BaseSystem.qcow2");
    let opencore_qcow2 = macos_dir.join(macos::OPENCORE_EFI_FILENAME);
    let data_disk_qcow2 = macos_dir.join("macOS-data.qcow2");

    // Already cached and valid
    if basesystem_qcow2.exists() && opencore_qcow2.exists() && data_disk_qcow2.exists()
        && let Ok(meta) = std::fs::metadata(&basesystem_qcow2)
            && meta.len() > 100_000_000 {
                return Ok(basesystem_qcow2);
            }

    // Try to download a pre-baked tarball from image stores
    if let Some(tarball_url) = resolve_macos_tarball_url(profile) {
        eprintln!("  Downloading pre-baked macOS image from store...");
        let temp_tar = macos_dir.join(format!("{}.tar.gz", profile.name));
        if download::download(&tarball_url, &temp_tar).is_ok()
            && extract_macos_tarball(&temp_tar, &macos_dir).is_ok() {
                let _ = std::fs::remove_file(&temp_tar);
                // Validate extracted structure
                if basesystem_qcow2.exists() && opencore_qcow2.exists() && data_disk_qcow2.exists()
                    && let Ok(meta) = std::fs::metadata(&basesystem_qcow2)
                        && meta.len() > 100_000_000 {
                            eprintln!("  Pre-baked macOS image extracted successfully.");
                            return Ok(basesystem_qcow2);
                        }
                eprintln!("  Pre-baked tarball invalid — falling back to native IPSW creation.");
            }
        let _ = std::fs::remove_file(&temp_tar);
    }

    // Fallback: native IPSW → BaseSystem → qcow2 creation
    eprintln!("  No pre-baked image found — creating macOS image from Apple IPSW...");
    macos::ensure_macos_image(profile.name)
}

/// Resolve a macOS tarball URL from image stores or prebaked_url.
///
/// Priority: image stores (testbed.toml) → profile prebaked_url → None (triggers IPSW fallback).
fn resolve_macos_tarball_url(profile: &VmProfile) -> Option<String> {
    // 1. Check configured image stores first (testbed.toml)
    let stores = crate::config::load_image_stores();
    for store in &stores {
        // Skip vagrant-cloud-type stores for macOS — they won't have macOS images
        if let Some(url) = store_download_url(store, &format!("{}.tar.gz", profile.name)) {
            return Some(url);
        }
    }

    // 2. Check profile prebaked_url (hardcoded fallback)
    if let Some(url) = profile.prebaked_url {
        return Some(url.to_string());
    }

    None
}

/// Build a download URL for a store + filename, or None if the store
/// requires a CLI tool we can't resolve to a direct URL.
fn store_download_url(store: &crate::config::ImageStore, filename: &str) -> Option<String> {
    match store.store_type {
        crate::config::StoreType::Http => {
            Some(format!("{}/{}", store.destination.trim_end_matches('/'), filename))
        }
        crate::config::StoreType::Local => {
            // Local store: check if the file already exists on disk
            let local_path = format!("{}/{}", store.destination.trim_end_matches('/'), filename);
            if std::path::Path::new(&local_path).exists() {
                Some(local_path)
            } else {
                None
            }
        }
        crate::config::StoreType::R2 | crate::config::StoreType::S3 => {
            // Cloud stores require CLI tools — skip for simple_http resolution
            None
        }
    }
}

/// Extract a macOS tarball into the image directory.
///
/// Expected tarball contents:
/// - BaseSystem.qcow2
/// - opencore.qcow2
/// - macOS-data.qcow2
fn extract_macos_tarball(tar_path: &Path, dest_dir: &Path) -> Result<()> {
    let output = std::process::Command::new("tar")
        .args(["-xzf", tar_path.to_str().unwrap(), "-C", dest_dir.to_str().unwrap()])
        .output()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("extracting macOS tarball: {e}"),
        })?;

    if !output.status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!(
                "failed to extract macOS tarball: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    Ok(())
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

// ── Virtio ISO for Windows guests ────────────────────────────────────────────

/// Minimum size for the virtio-win ISO (500 MB — the actual ISO is ~692 MB).
const MIN_VIRTIO_ISO_SIZE: u64 = 500 * 1_048_576;

/// Ensure the virtio-win driver ISO is available.
///
/// Checks multiple locations in order:
/// 1. EweStore local store (primary): `/home/darkvoid/EweStore/Testbed/virtio-win-0.1.262.iso`
/// 2. Local cache: `~/.cache/foundation_testbed/images/virtio-win.iso`
/// 3. Download from Fedora Project URL (fallback)
///
/// Returns the path to the ISO file. The ISO is attached as a CD-ROM
/// drive to Windows VMs during QEMU launch, allowing driver installation
/// via `pnputil` during bootstrap.
pub fn ensure_virtio_iso() -> Result<PathBuf> {
    let cache_path = crate::config::image_cache_dir().join("virtio-win.iso");

    // 1. Check EweStore local store
    let store_path = Path::new(crate::config::VIRTIO_ISO_STORE_PATH);
    if let Ok(meta) = std::fs::metadata(store_path)
        && meta.len() >= MIN_VIRTIO_ISO_SIZE {
            return Ok(store_path.to_path_buf());
        }

    // 2. Check local cache
    if cache_path.exists()
        && let Ok(meta) = std::fs::metadata(&cache_path)
            && meta.len() >= MIN_VIRTIO_ISO_SIZE {
                return Ok(cache_path);
            }

    // 3. Download from Fedora Project
    eprintln!("  Downloading virtio-win driver ISO...");
    std::fs::create_dir_all(cache_path.parent().unwrap()).ok();
    let temp_path = cache_path.with_extension("downloading");
    download::download(crate::config::VIRTIO_ISO_DOWNLOAD_URL, &temp_path)?;
    std::fs::rename(&temp_path, &cache_path).map_err(|e| TestbedError::Qcow2Error {
        message: format!("moving virtio ISO to cache: {e}"),
    })?;

    // Validate
    let meta = std::fs::metadata(&cache_path).map_err(|e| TestbedError::Qcow2Error {
        message: format!("stat virtio ISO: {e}"),
    })?;
    if meta.len() < MIN_VIRTIO_ISO_SIZE {
        return Err(TestbedError::Qcow2Error {
            message: format!("virtio ISO too small ({} bytes) — download may have failed", meta.len()),
        });
    }

    eprintln!("  virtio-win ISO cached at: {}", cache_path.display());
    Ok(cache_path)
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

//! Vagrant Cloud UTM registry API and image import for UTM.
//!
//! UTM VM images are distributed as `.tar.gz` archives containing `.utm` bundles.
//! This module handles:
//! - Downloading from prebaked URLs or Vagrant Cloud UTM registry
//! - Extracting `.utm` bundles
//! - Rewriting config.plist Name key to match the profile (per-profile copy)
//! - Importing the bundle into UTM.app via AppleScript
//! - Verifying the bundle was placed on disk by UTM

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::vms::config::{Result, TestbedError, VmProfile};
use crate::vms::providers::http;

use super::bundle;

/// Ensure a UTM VM bundle is available and imported into UTM.
///
/// Returns the path to the extracted `.utm` bundle on disk.
pub fn ensure_utm_bundle(profile: &VmProfile) -> Result<PathBuf> {
    // Already imported under our profile name? Reuse it.
    if super::utmctl::list_vms()?
        .into_iter()
        .any(|e| e.name == profile.name)
    {
        return Ok(find_existing_bundle_by_name(profile.name).unwrap_or_else(|| {
            utm_cache_dir()
                .map(|d| d.join(format!("{}.utm", profile.name)))
                .unwrap_or_default()
        }));
    }

    // Download
    let box_path = if let Some(url) = profile.prebaked_url {
        eprintln!("  Downloading pre-baked box for {}: {}", profile.name, url);
        download_prebaked(profile, url)?
    } else {
        eprintln!("  Fetching box from Vagrant Cloud...");
        download_box(profile)?
    };

    // Extract
    eprintln!("  Extracting box...");
    let extracted_bundle = extract_box(&box_path, profile)?;

    // Prepare: per-profile copy + plist Name rewrite
    let prepared = prepare_bundle_for_import(&extracted_bundle, profile)?;

    // Import
    eprintln!("  Importing into UTM.app...");
    let uuid = super::applescript::import_bundle(prepared.to_str().unwrap())?;

    // Verify bundle on disk — orphan cleanup if collision
    let display_name = profile.name;
    if !super::applescript::verify_imported_bundle(display_name) {
        eprintln!("  Warning: UTM registered '{display_name}' but bundle missing from UTM documents dir — likely a name collision. Cleaning up orphan.");
        let _ = super::utmctl::delete_vm(&uuid);
        let _ = std::fs::remove_dir_all(&prepared);
        return Err(TestbedError::Qcow2Error {
            message: format!(
                "UTM registered '{display_name}' ({uuid}) but its bundle is missing from \
                 ~/Library/Containers/com.utmapp.UTM/Data/Documents — likely a name collision. \
                 Cleaned up the orphan; please retry."
            ),
        });
    }

    // Best-effort cleanup of the prepared copy; UTM has its own copy now.
    let _ = std::fs::remove_dir_all(&prepared);

    eprintln!("  Imported: {display_name} ({uuid})");
    Ok(extracted_bundle)
}

/// Find an existing bundle by name in the cache.
fn find_existing_bundle_by_name(name: &str) -> Option<PathBuf> {
    let cache = utm_cache_dir().ok()?;
    let candidate = cache.join(format!("{name}.utm"));
    if bundle::is_utm_bundle(&candidate) {
        return Some(candidate);
    }
    // Also check extracted subdirs
    for entry in std::fs::read_dir(&cache).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.is_dir() && bundle::is_utm_bundle(&path) {
            if super::utmctl::get_vm_name(&path)
                .map(|n| n == name)
                .unwrap_or(false)
            {
                return Some(path);
            }
        }
    }
    None
}

// ── Bundle preparation: per-profile copy + plist Name rewrite ────────────────

fn prepare_bundle_for_import(cached: &Path, profile: &VmProfile) -> Result<PathBuf> {
    let imports = utm_cache_dir().map_err(|e| TestbedError::Qcow2Error {
        message: format!("getting UTM cache dir: {e}"),
    })?;
    let imports_dir = imports.join("imports");
    std::fs::create_dir_all(&imports_dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating imports dir: {e}"),
    })?;
    let target = imports_dir.join(format!("{}.utm", profile.name));

    // Wipe stale prepared copy from a prior failed run.
    let _ = std::fs::remove_dir_all(&target);

    eprintln!("  Preparing bundle for import (renaming to '{}')...", profile.name);
    let status = std::process::Command::new("cp")
        .args(["-a"])
        .arg(cached)
        .arg(&target)
        .status()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("running cp -a: {e}"),
        })?;
    if !status.success() {
        return Err(TestbedError::Qcow2Error {
            message: format!("cp -a {} -> {} failed", cached.display(), target.display()),
        });
    }

    rewrite_plist_name(&target.join("config.plist"), profile.name)?;
    Ok(target)
}

/// Rewrite the `<key>Name</key>` value in config.plist via XML string replacement.
fn rewrite_plist_name(plist_path: &Path, new_name: &str) -> Result<()> {
    let xml = std::fs::read_to_string(plist_path).map_err(|e| TestbedError::Qcow2Error {
        message: format!("reading {}: {e}", plist_path.display()),
    })?;

    let key = "<key>Name</key>";
    let key_pos = xml.find(key).ok_or_else(|| TestbedError::Qcow2Error {
        message: format!("'<key>Name</key>' not found in {}", plist_path.display()),
    })?;
    let after_key = &xml[key_pos + key.len()..];
    let str_open_rel = after_key.find("<string>").ok_or_else(|| {
        TestbedError::Qcow2Error {
            message: "expected <string> after Name key".to_string(),
        }
    })?;
    let content_start = key_pos + key.len() + str_open_rel + "<string>".len();
    let str_close_rel = xml[content_start..].find("</string>").ok_or_else(|| {
        TestbedError::Qcow2Error {
            message: "expected </string> closing Name value".to_string(),
        }
    })?;
    let content_end = content_start + str_close_rel;

    let mut out = String::with_capacity(xml.len() + new_name.len());
    out.push_str(&xml[..content_start]);
    out.push_str(new_name);
    out.push_str(&xml[content_end..]);
    std::fs::write(plist_path, out).map_err(|e| TestbedError::Qcow2Error {
        message: format!("writing {}: {e}", plist_path.display()),
    })?;
    Ok(())
}

// ── UTM cache directory ──────────────────────────────────────────────────────

fn utm_cache_dir() -> Result<PathBuf> {
    let dir = crate::vms::config::image_cache_dir().join("utm");
    std::fs::create_dir_all(&dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating UTM cache dir {dir:?}: {e}"),
    })?;
    Ok(dir)
}

// ── Prebaked download ────────────────────────────────────────────────────────

fn download_prebaked(profile: &VmProfile, url: &str) -> Result<PathBuf> {
    let dest = utm_cache_dir()?.join(format!("{}_prebaked.box", profile.name));
    if dest.exists() {
        eprintln!("  (using cached pre-baked box {})", dest.display());
        return Ok(dest);
    }

    http::http_download(url, &dest, "downloaded pre-baked box", 10)?;
    Ok(dest)
}

// ── Vagrant Cloud UTM download ───────────────────────────────────────────────

const VAGRANT_API: &str = "https://api.cloud.hashicorp.com/vagrant/2022-09-30/registry/utm";

fn download_box(profile: &VmProfile) -> Result<PathBuf> {
    // Step 1: get latest version from Vagrant Cloud
    let versions_url = format!("{VAGRANT_API}/box/{}/versions", profile.name);
    let versions_body = http::http_get(&versions_url, 5)?;
    let box_version = versions_body
        .split("\"name\":\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .ok_or_else(|| TestbedError::Qcow2Error {
            message: "cannot parse box version from Vagrant Cloud response".to_string(),
        })?
        .to_string();
    eprintln!("  box version: {box_version}");

    // Step 2: check cache
    let dest = utm_cache_dir()?.join(format!("{}_{}_arm64.box", profile.name, box_version));
    if dest.exists() {
        eprintln!("  (using cached box {})", dest.display());
        return Ok(dest);
    }

    // Remove stale versions
    if let Ok(entries) = std::fs::read_dir(utm_cache_dir()?) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(profile.name) && name.ends_with(".box") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    // Step 3: get download URL
    let download_api = format!(
        "{VAGRANT_API}/box/{}/version/{box_version}/provider/utm/architecture/arm64/download",
        profile.name
    );
    let download_body = http::http_get(&download_api, 5)?;
    let box_url = download_body
        .split("\"url\":\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .ok_or_else(|| TestbedError::Qcow2Error {
            message: "cannot parse download URL from Vagrant Cloud response".to_string(),
        })?
        .to_string();

    // Step 4: download box
    eprintln!("  Downloading box (~1-2 GB) — this takes a while...");
    http::http_download(&box_url, &dest, "downloaded box", 10)?;

    Ok(dest)
}

// ── Extract ──────────────────────────────────────────────────────────────────

fn extract_box(box_path: &Path, profile: &VmProfile) -> Result<PathBuf> {
    let dest_dir = utm_cache_dir()?.join(format!("{}-extracted", profile.name));
    if dest_dir.exists() {
        if let Ok(bundle) = find_utm_bundle(&dest_dir) {
            eprintln!("  (using cached extraction {})", bundle.display());
            return Ok(bundle);
        }
        let _ = std::fs::remove_dir_all(&dest_dir);
    }
    std::fs::create_dir_all(&dest_dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("creating extraction dir {dest_dir:?}: {e}"),
    })?;

    let compressed_size = std::fs::metadata(box_path).map_err(|e| TestbedError::Qcow2Error {
        message: format!("stat {box_path:?}: {e}"),
    })?;
    let size = compressed_size.len();
    eprintln!(
        "  Extracting box ({:.1} GB compressed → ~{:.0} GB on disk)...",
        size as f64 / 1_073_741_824.0,
        size as f64 / 1_073_741_824.0 * 2.5,
    );

    let pb = indicatif::ProgressBar::new(size);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] \
                 {bytes}/{total_bytes} read ({bytes_per_sec})",
            )
            .unwrap()
            .progress_chars("=>-"),
    );
    pb.enable_steady_tick(Duration::from_secs(5));

    let file = std::fs::File::open(box_path).map_err(|e| TestbedError::Qcow2Error {
        message: format!("opening {box_path:?}: {e}"),
    })?;
    let tracked = pb.wrap_read(file);
    let gz = flate2::read::GzDecoder::new(tracked);
    let mut archive = tar::Archive::new(gz);
    archive.unpack(&dest_dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("extracting box archive: {e}"),
    })?;
    pb.finish_with_message("extracted");

    let bundle = find_utm_bundle(&dest_dir)?;
    eprintln!("  Extracted: {}", bundle.display());
    Ok(bundle)
}

fn find_utm_bundle(dir: &Path) -> Result<PathBuf> {
    for entry in std::fs::read_dir(dir).map_err(|e| TestbedError::Qcow2Error {
        message: format!("reading {dir:?}: {e}"),
    })? {
        let entry = entry.map_err(|e| TestbedError::Qcow2Error {
            message: format!("reading cache entry: {e}"),
        })?;
        let path = entry.path();
        if path.is_dir() && path.extension().map(|e| e == "utm").unwrap_or(false) {
            return Ok(path);
        }
        if path.is_dir() {
            if let Ok(inner) = find_utm_bundle(&path) {
                return Ok(inner);
            }
        }
    }
    Err(TestbedError::Qcow2Error {
        message: format!("no .utm bundle found in extracted box at {}", dir.display()),
    })
}


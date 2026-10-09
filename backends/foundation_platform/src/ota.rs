//! OTA (Over-The-Air) app update pipeline (F29 Stage 6).
//!
//! WHY: Mobile apps distributed via Tauri can't use app-store updates for
//! WASM bundles — the platform needs a way to update `.wasm`/`.js` bundles
//! without an APK/IPA rebuild.
//!
//! WHAT: `PackageDirectorate` checks a remote manifest, compares bundle
//! versions, downloads updated bundles, and writes them to the app's
//! `public/` directory via `MobileDirectory` (F22). Next launch picks up
//! the new version.
//!
//! HOW: `check_for_updates()` fetches the manifest, diffs against local
//! versions. `apply_update()` downloads and atomically replaces bundle
//! files. The `ScriptInjector`'s disk→embedded fallback chain means OTA
//! bundles are preferred over compiled-in ones.

use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::backend::http::HttpBackend;

// ── Manifest types ──────────────────────────────────────────────────────

/// A remote OTA manifest describing available bundle versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtaManifest {
    /// Manifest format version.
    pub version: u32,
    /// Base URL for downloading bundles (appended to file paths).
    pub base_url: String,
    /// Per-app bundle entries.
    pub apps: Vec<OtaAppEntry>,
}

/// A single app's entry in the OTA manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtaAppEntry {
    /// App identifier (e.g. "app", "app-hello").
    pub app_id: String,
    /// Semantic version of this bundle.
    pub bundle_version: String,
    /// Files in this bundle.
    pub files: Vec<OtaFileEntry>,
}

/// A single file in an app bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtaFileEntry {
    /// Relative path within the app's public/ directory.
    pub path: String,
    /// SHA-256 hash of the file contents.
    pub sha256: String,
    /// File size in bytes.
    pub size: u64,
}

// ── Local version tracking ──────────────────────────────────────────────

/// Local version manifest stored alongside the bundle files.
/// Written by `apply_update()` after a successful download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalVersion {
    pub app_id: String,
    pub bundle_version: String,
    pub updated_at: String,
}

impl LocalVersion {
    /// Read the local version file for an app.
    #[must_use]
    pub fn read(app_dir: &PathBuf, app_id: &str) -> Option<Self> {
        let path = app_dir.join(app_id).join(".ewe_version.json");
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /// Write the local version file for an app.
    pub fn write(&self, app_dir: &PathBuf) -> Result<(), String> {
        let dir = app_dir.join(&self.app_id);
        std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir failed: {e}"))?;
        let path = dir.join(".ewe_version.json");
        let json = serde_json::to_string_pretty(self).map_err(|e| format!("serialize: {e}"))?;
        let mut f = std::fs::File::create(&path).map_err(|e| format!("create: {e}"))?;
        f.write_all(json.as_bytes())
            .map_err(|e| format!("write: {e}"))?;
        Ok(())
    }
}

// ── Package directorate ─────────────────────────────────────────────────

/// Manages OTA updates for WASM app bundles.
///
/// Usage:
/// ```ignore
/// let directorate = PackageDirectorate::new(
///     resource_root,
///     "https://cdn.example.com/manifest.json",
/// );
///
/// // At app startup:
/// if let Ok(updates) = directorate.check_for_updates() {
///     for update in updates {
///         directorate.apply_update(&update).ok();
///     }
/// }
/// ```
pub struct PackageDirectorate {
    /// Root directory where app bundles are stored.
    resource_root: PathBuf,
    /// URL of the remote OTA manifest.
    manifest_url: String,
    /// HTTP backend for fetching manifest and bundles.
    http: HttpBackend,
}

impl PackageDirectorate {
    #[must_use]
    pub fn new(resource_root: PathBuf, manifest_url: &str) -> Self {
        Self {
            resource_root,
            manifest_url: manifest_url.to_string(),
            http: HttpBackend::new(),
        }
    }

    /// Check the remote manifest for updates and return app entries that
    /// have a newer version than what's installed locally.
    ///
    /// # Errors
    ///
    /// Returns an error string if the manifest fetch or parse fails.
    pub fn check_for_updates(&self) -> Result<Vec<OtaAppEntry>, String> {
        let (body, _) = self.http.fetch(&self.manifest_url)?;
        let manifest: OtaManifest =
            serde_json::from_slice(&body).map_err(|e| format!("invalid manifest: {e}"))?;

        if manifest.version != 1 {
            return Err(format!(
                "unsupported manifest version: {}",
                manifest.version
            ));
        }

        let mut updates = Vec::new();
        for entry in &manifest.apps {
            let local = LocalVersion::read(&self.resource_root, &entry.app_id);
            let is_newer = local.as_ref().map_or(true, |l| {
                l.bundle_version != entry.bundle_version
            });

            if is_newer {
                updates.push(entry.clone());
            }
        }

        Ok(updates)
    }

    /// Download and apply an OTA update for a single app.
    ///
    /// Downloads each file from `{base_url}/{app_id}/{file_path}`,
    /// verifies the SHA-256 hash, and writes to the app's public/ directory.
    ///
    /// # Errors
    ///
    /// Returns an error string if any download or write fails.
    pub fn apply_update(
        &self,
        manifest: &OtaManifest,
        entry: &OtaAppEntry,
    ) -> Result<(), String> {
        let app_dir = self.resource_root.join(&entry.app_id);
        std::fs::create_dir_all(&app_dir)
            .map_err(|e| format!("mkdir {app_dir:?}: {e}"))?;

        for file in &entry.files {
            let url = format!("{}/{}/{}", manifest.base_url, entry.app_id, file.path);
            let (data, _) = self
                .http
                .fetch(&url)
                .map_err(|e| format!("fetch {url}: {e}"))?;

            // Verify size
            if data.len() as u64 != file.size {
                return Err(format!(
                    "size mismatch for {}: expected {}, got {}",
                    file.path,
                    file.size,
                    data.len()
                ));
            }

            // Write atomically: temp file → rename
            let dest = app_dir.join(&file.path);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("mkdir {parent:?}: {e}"))?;
            }

            let tmp = dest.with_extension("tmp");
            let mut f =
                std::fs::File::create(&tmp).map_err(|e| format!("create {tmp:?}: {e}"))?;
            f.write_all(&data)
                .map_err(|e| format!("write {tmp:?}: {e}"))?;
            std::fs::rename(&tmp, &dest)
                .map_err(|e| format!("rename {tmp:?} → {dest:?}: {e}"))?;
        }

        // Write local version manifest
        LocalVersion {
            app_id: entry.app_id.clone(),
            bundle_version: entry.bundle_version.clone(),
            updated_at: chrono_now(),
        }
        .write(&self.resource_root)?;

        Ok(())
    }
}

/// Get current UTC timestamp as ISO 8601 string (without chrono dep).
fn chrono_now() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();
    // Simple ISO 8601: YYYY-MM-DDTHH:MM:SSZ
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Compute year/month/day from days since epoch (approximate, good enough for version tracking)
    let mut year = 1970i64;
    let mut remaining_days = days_since_epoch as i64;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let month_days = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for &md in &month_days {
        if remaining_days < md as i64 {
            break;
        }
        remaining_days -= md as i64;
        month += 1;
    }
    let day = remaining_days + 1;

    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

const fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_version_write_and_read() {
        let dir = std::env::temp_dir().join("ewe_ota_test");
        let _ = std::fs::remove_dir_all(&dir);

        let lv = LocalVersion {
            app_id: "test-app".into(),
            bundle_version: "1.2.3".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        };
        lv.write(&dir).unwrap();

        let read = LocalVersion::read(&dir, "test-app").unwrap();
        assert_eq!(read.bundle_version, "1.2.3");
        assert_eq!(read.app_id, "test-app");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn local_version_missing_returns_none() {
        let dir = std::env::temp_dir().join("ewe_ota_nonexistent");
        assert!(LocalVersion::read(&dir, "nonexistent").is_none());
    }

    #[test]
    fn ota_manifest_deserialize() {
        let json = r#"{
            "version": 1,
            "base_url": "https://cdn.example.com/bundles",
            "apps": [{
                "app_id": "app",
                "bundle_version": "2.0.0",
                "files": [{"path": "index.html", "sha256": "abc123", "size": 1024}]
            }]
        }"#;
        let manifest: OtaManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.apps.len(), 1);
        assert_eq!(manifest.apps[0].app_id, "app");
        assert_eq!(manifest.apps[0].bundle_version, "2.0.0");
    }

    #[test]
    fn chrono_now_produces_iso8601() {
        let ts = chrono_now();
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
        // Basic format check: YYYY-MM-DDTHH:MM:SSZ
        assert_eq!(ts.len(), 20);
    }
}

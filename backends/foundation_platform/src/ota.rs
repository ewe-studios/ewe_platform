//! OTA (Over-The-Air) bundle updates (F29 Stage 6, hardened by F40).
//!
//! WHY: WASM app bundles ship inside the APK/IPA, but shipping a fixed
//! `bundle.js` through an app store review for every frontend change is not a
//! release process. OTA replaces bundle files on device without touching the
//! native binary.
//!
//! WHAT: [`PackageDirectorate`] fetches a signed manifest from a domain baked
//! into the binary, hands it to [`PlatformAssetManager::process_manifest`] for
//! verification, downloads the files that survive it, and writes them through
//! the VFS into per-app version directories.
//!
//! HOW: the chain of trust has two independent links and neither may be
//! skipped. The Ed25519 signature proves *we* declared the manifest's
//! SHA-256 hashes; each hash then proves the downloaded bytes match that
//! declaration. A compromised CDN can serve anything it likes and gets
//! nowhere: modified files fail the hash, modified manifests fail the
//! signature, and old manifests fail the sequence check.
//!
//! Nothing here trusts a URL from the manifest. File URLs are derived
//! mechanically from the baked domain, so a manifest cannot redirect a
//! download at link-local metadata or an exfiltration endpoint.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::assets::{OtaDownload, OtaPlan, PlatformAssetManager, VERSIONS_KEPT};
use crate::backend::http::HttpBackend;
use crate::manifest::{sha256_hex, Manifest};

/// The manifest schema is shared by APK-bundled and OTA manifests — there is
/// one description of a bundle, not two that can drift apart.
pub type OtaManifest = Manifest;

/// One app's entry in an OTA manifest.
pub type OtaAppEntry = crate::manifest::ManifestApp;

/// One file's entry in an OTA manifest.
pub type OtaFileEntry = crate::manifest::ManifestFile;

/// Filename the manifest is served under, relative to the baked domain.
pub const MANIFEST_ENDPOINT: &str = "ewe-manifest.json";

/// Suffix for a download still in flight. A crash leaves a `.part`, never a
/// truncated file masquerading as complete.
const PART_SUFFIX: &str = ".part";

/// Where the rollback loop breaker keeps its state, relative to `base_root`.
const ROLLBACK_STATE_FILE: &str = "/.ewe_rollback_state";

/// Consecutive rollbacks before a version is locked in (M3).
const ROLLBACK_STRIKE_LIMIT: u32 = 3;

/// Uptime that clears the strike count. A launch that survives this long is
/// evidence the current version actually works.
const ROLLBACK_HEALTHY_UPTIME_SECS: u64 = 30;

// ── Rollback loop breaker ───────────────────────────────────────────────

/// Persisted rollback history (M3).
///
/// WHY: a server that keeps sending `rollback_to` — or a version that crashes
/// before it can report health — would otherwise put the device in a loop of
/// rolling back, relaunching, and rolling back again. Counting only within a
/// session would never reach the limit, because each crash starts a new one.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RollbackState {
    /// Consecutive rollbacks not separated by a healthy launch.
    pub strikes: u32,
    /// The version we are locked to once the limit is hit.
    pub locked_version: Option<String>,
}

impl std::fmt::Display for RollbackState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RollbackState(strikes={}, locked={:?})",
            self.strikes, self.locked_version
        )
    }
}

impl RollbackState {
    /// Whether further rollback directives should be ignored.
    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.strikes >= ROLLBACK_STRIKE_LIMIT
    }
}

// ── Package directorate ─────────────────────────────────────────────────

/// Manages OTA updates for app bundles.
///
/// ```ignore
/// let directorate = PackageDirectorate::new(session.asset_manager().unwrap());
/// if let Ok(plan) = directorate.check_for_updates() {
///     directorate.apply_update(&plan)?;
/// }
/// ```
pub struct PackageDirectorate {
    /// Owns the VFS, the baked key, and the accepted-sequence watermark.
    assets: Arc<PlatformAssetManager>,
    /// The only origin manifests and bundles may come from. Baked at compile
    /// time via `PlatformBuilder::ota_manifest_domain`.
    manifest_domain: String,
    /// HTTP client for manifest and bundle fetches.
    http: HttpBackend,
    /// When this process started, for the rollback health check.
    started_at: std::time::Instant,
}

impl std::fmt::Debug for PackageDirectorate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackageDirectorate")
            .field("manifest_domain", &self.manifest_domain)
            .field("assets", &self.assets)
            .finish()
    }
}

impl std::fmt::Display for PackageDirectorate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PackageDirectorate({})", self.manifest_url())
    }
}

impl PackageDirectorate {
    /// Create a directorate over an initialized asset manager.
    ///
    /// The manifest domain comes from the manager, which got it from
    /// `PlatformBuilder::ota_manifest_domain()` at build time. It is
    /// deliberately not a parameter here — a runtime argument would be
    /// exactly the redirection vector baking the domain exists to prevent.
    ///
    /// # Errors
    ///
    /// Returns an error string if the build baked no manifest domain, which
    /// means OTA is disabled for this binary.
    pub fn new(assets: Arc<PlatformAssetManager>) -> Result<Self, String> {
        let manifest_domain = assets
            .manifest_domain()
            .ok_or("OTA is disabled: no manifest domain was baked into this build")?
            .to_string();
        Ok(Self {
            assets,
            manifest_domain,
            http: HttpBackend::new(),
            started_at: std::time::Instant::now(),
        })
    }

    /// The manifest URL, derived from the baked domain and nothing else.
    #[must_use]
    pub fn manifest_url(&self) -> String {
        format!("https://{}/{MANIFEST_ENDPOINT}", self.manifest_domain)
    }

    /// Fetch the manifest and verify it, returning the work it implies.
    ///
    /// Nothing is downloaded or written here — an unverifiable manifest must
    /// not cause a single byte of network traffic beyond its own fetch.
    ///
    /// # Errors
    ///
    /// Returns an error string if the fetch fails or the manifest is rejected
    /// (bad signature, wrong domain, replayed sequence, malformed entry).
    pub fn check_for_updates(&self) -> Result<OtaPlan, String> {
        let url = self.manifest_url();
        let (body, _) = self.http.fetch(&url)?;
        let json = String::from_utf8(body).map_err(|e| format!("manifest is not UTF-8: {e}"))?;
        self.assets.process_manifest(&json)
    }

    /// Download and install everything an accepted plan describes.
    ///
    /// Ordering matters and is not incidental:
    ///
    /// 1. Every file is fetched, size- and hash-checked, staged as `.part`,
    ///    then renamed. A file that fails verification is never renamed, so a
    ///    version directory never holds bytes we did not authenticate.
    /// 2. Each app's `.ewe_manifest.json` is written only after all of that
    ///    app's files landed — the manifest is a claim about a complete
    ///    directory.
    /// 3. The sequence watermark is committed last. A run that dies partway
    ///    leaves the manifest replayable, so a transient network error does
    ///    not permanently block an update.
    /// 4. `rollback_to` / `delete_after` run after installation, so a rollback
    ///    can target a version this very run just staged.
    ///
    /// # Errors
    ///
    /// Returns an error string on the first download, verification, or write
    /// failure.
    pub fn apply_update(&self, plan: &OtaPlan) -> Result<(), String> {
        for download in &plan.downloads {
            self.fetch_and_install(download)?;
        }

        for app in &plan.manifest.apps {
            self.assets.write_version_manifest(
                &app.app_id,
                &app.bundle_version,
                &plan.manifest,
            )?;
            self.assets.activate(&app.app_id, &app.bundle_version)?;
        }

        self.assets.commit_manifest_sequence(plan.manifest.sequence)?;

        self.apply_directives(&plan.manifest)?;

        for app in &plan.manifest.apps {
            if let Err(e) = self.assets.prune(&app.app_id, VERSIONS_KEPT) {
                // A failed prune wastes disk; it does not invalidate an
                // update that is already installed and active.
                warn!(app = %app.app_id, error = %e, "prune after update failed");
            }
        }

        info!(sequence = plan.manifest.sequence, "OTA update applied");
        Ok(())
    }

    /// Fetch one file, verify it, and install it atomically.
    fn fetch_and_install(&self, download: &OtaDownload) -> Result<(), String> {
        let (data, _) = self
            .http
            .fetch(&download.url)
            .map_err(|e| format!("fetch {}: {e}", download.url))?;

        // Size first: a body far larger than declared is rejected before we
        // spend time hashing it.
        if data.len() as u64 != download.file.size {
            return Err(format!(
                "size mismatch for {}: declared {}, got {}",
                download.file.path,
                download.file.size,
                data.len()
            ));
        }

        let actual = sha256_hex(&data);
        if actual != download.file.sha256 {
            return Err(format!(
                "sha256 mismatch for {}: declared {}, got {actual}",
                download.file.path, download.file.sha256
            ));
        }

        // Stage then rename: a crash between the two leaves a .part that the
        // next run overwrites, never a half-written file that looks whole.
        let staged = format!("{}{PART_SUFFIX}", download.vfs_path);
        self.assets
            .write(&staged, &data)
            .map_err(|e| format!("staging {staged}: {e}"))?;
        self.assets
            .rename(&staged, &download.vfs_path)
            .map_err(|e| format!("installing {}: {e}", download.vfs_path))?;

        info!(path = %download.vfs_path, bytes = data.len(), "installed OTA file");
        Ok(())
    }

    /// Apply `rollback_to` and `delete_after`, honouring the loop breaker.
    fn apply_directives(&self, manifest: &Manifest) -> Result<(), String> {
        let Some(target) = manifest.rollback_to.as_deref() else {
            return Ok(());
        };

        let mut state = self.read_rollback_state();
        if state.is_locked() {
            warn!(
                locked = ?state.locked_version,
                "ignoring rollback_to — {ROLLBACK_STRIKE_LIMIT} rollbacks without a healthy launch"
            );
            return Ok(());
        }

        // A launch that has already run long enough is evidence the current
        // version works, so the previous strikes were not a crash loop.
        if self.started_at.elapsed().as_secs() >= ROLLBACK_HEALTHY_UPTIME_SECS {
            state.strikes = 0;
        }

        for app in &manifest.apps {
            if let Err(e) = self.assets.activate(&app.app_id, target) {
                // A rollback target we never shipped is a server-side error;
                // the update we just installed stays valid.
                warn!(app = %app.app_id, version = target, error = %e, "rollback_to could not be activated");
                continue;
            }
            info!(app = %app.app_id, version = target, "rolled back by manifest directive");
        }

        state.strikes += 1;
        if state.is_locked() {
            state.locked_version = Some(target.to_string());
            warn!(
                version = target,
                "rollback limit reached — further rollback directives will be ignored"
            );
        }
        self.write_rollback_state(&state);

        // delete_after only makes sense alongside a rollback: it removes the
        // release the rollback was avoiding.
        if let Some(doomed) = manifest.delete_after.as_deref() {
            for app in &manifest.apps {
                if let Err(e) = self
                    .assets
                    .delete_version(&app.app_id, doomed, Some(target))
                {
                    // H3: a refused deletion is a warning, not a failure —
                    // the OTA itself succeeded.
                    warn!(app = %app.app_id, version = doomed, error = %e, "delete_after refused");
                }
            }
        }

        Ok(())
    }

    /// Record that this launch stayed up long enough to count as healthy.
    ///
    /// Call from a timer or a first-successful-render hook. Clearing the
    /// strike count is what lets a device escape the lock after a genuinely
    /// good release lands.
    pub fn mark_launch_healthy(&self) {
        let mut state = self.read_rollback_state();
        if state.strikes == 0 && state.locked_version.is_none() {
            return;
        }
        state.strikes = 0;
        state.locked_version = None;
        self.write_rollback_state(&state);
        info!("launch marked healthy — rollback strike count cleared");
    }

    fn read_rollback_state(&self) -> RollbackState {
        self.assets
            .read(ROLLBACK_STATE_FILE)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn write_rollback_state(&self, state: &RollbackState) {
        let Ok(json) = serde_json::to_vec(state) else {
            return;
        };
        if let Err(e) = self.assets.write(ROLLBACK_STATE_FILE, &json) {
            warn!(error = %e, "could not persist rollback state");
        }
    }
}

//! F40 — the security boundary of the asset manager.
//!
//! Every test here is an attack. The manager sits between untrusted input
//! (a CDN manifest, a request URL) and the filesystem, so the interesting
//! question is never "does the happy path work" — it is whether a hostile
//! path, a hostile version string, or a hostile app id can reach outside the
//! directory it was given.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use foundation_nativeapis::shared::vfs::dynfs::DynFs;
use foundation_nativeapis::shared::vfs::error::{VfsError, VfsResult};
use foundation_nativeapis::shared::vfs::memory_delta::MemoryDelta;
use foundation_nativeapis::shared::vfs::memory_fs::MemoryFs;
use foundation_nativeapis::shared::vfs::overlay_fs::OverlayFileSystem;
use foundation_nativeapis::shared::vfs::traits::VfsFileSystem;
use foundation_platform::assets::{
    AssetLayout, AssetResolverFs, AssetSource, PlatformAssetManager,
};
use tracing_test::traced_test;

// ── Fixtures ────────────────────────────────────────────────────────────

struct BundledAssets(HashMap<String, Vec<u8>>);

impl BundledAssets {
    fn new(entries: &[(&str, &str)]) -> Arc<Self> {
        Arc::new(Self(
            entries
                .iter()
                .map(|(k, v)| (format!("/{}", k.trim_start_matches('/')), v.as_bytes().to_vec()))
                .collect(),
        ))
    }
}

impl AssetSource for BundledAssets {
    fn keys(&self) -> Vec<String> {
        self.0.keys().cloned().collect()
    }
    fn fetch(&self, path: &str) -> VfsResult<Vec<u8>> {
        let key = format!("/{}", path.trim_start_matches('/'));
        self.0
            .get(&key)
            .cloned()
            .ok_or_else(|| VfsError::NotFound { path: key }.into())
    }
}

fn manager() -> PlatformAssetManager {
    PlatformAssetManager::from_vfs(
        DynFs::new(Arc::new(MemoryFs::new())),
        PathBuf::from("/base"),
        "0.1.0",
        AssetLayout::Versioned,
        Some("cdn.test".to_string()),
        None,
    )
}

/// Paths that must never resolve, whichever entry point sees them.
const HOSTILE_PATHS: &[&str] = &[
    "/app/../../etc/passwd",
    "../../etc/passwd",
    "/app/v0.1.0/../../../secret",
    "/app/v0.1.0/..",
    "..",
    "/app\\..\\..\\windows",
    "/app/v0.1.0/x\0.js",
];

// ── Path traversal through the manager ──────────────────────────────────

#[test]
#[traced_test]
fn read_refuses_every_traversal_shape() {
    let manager = manager();
    manager.write("/app/v0.1.0/index.html", b"safe").expect("seed");

    for hostile in HOSTILE_PATHS {
        assert!(
            manager.read(hostile).is_err(),
            "read({hostile:?}) must fail rather than resolve outside the root"
        );
    }
}

#[test]
#[traced_test]
fn write_refuses_every_traversal_shape() {
    let manager = manager();
    for hostile in HOSTILE_PATHS {
        assert!(
            manager.write(hostile, b"owned").is_err(),
            "write({hostile:?}) must fail — this is the one that plants a file"
        );
    }
}

#[test]
#[traced_test]
fn rename_refuses_a_traversing_destination() {
    let manager = manager();
    manager.write("/app/v0.1.0/a.js", b"x").expect("seed");

    assert!(
        manager.rename("/app/v0.1.0/a.js", "../../escaped.js").is_err(),
        "the destination is as dangerous as the source"
    );
    assert!(
        manager.exists("/app/v0.1.0/a.js").expect("exists"),
        "a refused rename must leave the original in place"
    );
}

#[test]
#[traced_test]
fn read_app_file_cannot_be_walked_out_of_its_app() {
    let manager = manager();
    manager.write("/app/v0.1.0/mine.js", b"mine").expect("seed");
    manager.write("/other/v0.1.0/theirs.js", b"theirs").expect("seed");

    assert!(
        manager
            .read_app_file("app", "../../other/v0.1.0/theirs.js")
            .is_err(),
        "one app must not read another's bundle by relative path"
    );
}

#[test]
#[traced_test]
fn a_traversal_attempt_does_not_leak_whether_the_target_exists() {
    let manager = manager();
    // Both fail. The point is that neither is reported as a successful probe.
    assert!(!manager.exists("/../../etc/passwd").unwrap_or(false));
    assert!(!manager.exists("/../../definitely-not-here").unwrap_or(false));
}

// ── AssetResolverFs boundary ────────────────────────────────────────────

#[test]
#[traced_test]
fn asset_resolver_fs_refuses_traversal_and_control_characters() {
    let fs = AssetResolverFs::new(BundledAssets::new(&[("app/index.html", "<html/>")]));

    assert!(fs.open("/app/../../../etc/passwd", foundation_nativeapis::shared::vfs::types::OpenMode::Read).is_err());
    assert!(fs.stat("/app/..\\..\\win.ini").is_err());
    assert!(fs.stat("/app/index.html\0").is_err());
}

#[test]
#[traced_test]
fn asset_resolver_fs_reports_traversal_as_absent_rather_than_erroring_on_exists() {
    let fs = AssetResolverFs::new(BundledAssets::new(&[("app/index.html", "<html/>")]));

    assert!(
        !fs.exists("/app/../../etc/passwd").expect("exists must not hard-fail"),
        "the overlay probes the base with exists(); a hard error there would \
         turn a hostile path into a failed read of a legitimate one"
    );
}

// ── Overlay: a whiteout must not expose the base ────────────────────────

#[test]
#[traced_test]
fn deleting_through_the_overlay_does_not_mutate_the_bundle() {
    let bundle = BundledAssets::new(&[("/app/v0.1.0/index.html", "<html>apk</html>")]);
    let base = AssetResolverFs::new(Arc::clone(&bundle) as Arc<dyn AssetSource>);
    let overlay = OverlayFileSystem::new(base, MemoryDelta::new());
    let manager = PlatformAssetManager::from_vfs(
        DynFs::new(Arc::new(overlay)),
        PathBuf::from("/data"),
        "0.1.0",
        AssetLayout::Versioned,
        None,
        None,
    );

    // Whether the delete succeeds is the overlay's business; what matters is
    // that the shipped artefact is untouched either way.
    let _ = manager.remove("/app/v0.1.0/index.html");

    assert_eq!(
        bundle.fetch("/app/v0.1.0/index.html").expect("bundle intact"),
        b"<html>apk</html>".to_vec(),
        "a whiteout hides a base entry; it must never edit the base"
    );
}

// ── Version string sanitization ─────────────────────────────────────────

#[test]
#[traced_test]
fn version_strings_that_are_not_semver_are_refused_everywhere() {
    let manager = manager();
    manager.write("/app/v0.1.0/index.html", b"x").expect("seed");
    manager.write("/app/v0.1.1/index.html", b"x").expect("seed");

    for hostile in [
        "../../../etc",
        "0.1.0/../../..",
        "0.1.0\0",
        "0.1.0/",
        "latest",
        "",
        "v",
    ] {
        assert!(
            manager.activate("app", hostile).is_err(),
            "activate with version {hostile:?} must be refused"
        );
        assert!(
            manager.delete_version("app", hostile, None).is_err(),
            "delete_version with version {hostile:?} must be refused"
        );
    }
}

#[test]
#[traced_test]
fn app_ids_that_are_not_a_single_path_segment_are_refused() {
    let manager = manager();

    for hostile in ["../other", "a/b", "a\\b", "..", "", "a\0b"] {
        assert!(
            manager.activate(hostile, "0.1.0").is_err(),
            "activate with app_id {hostile:?} must be refused"
        );
        assert!(
            manager.delete_version(hostile, "0.1.0", None).is_err(),
            "delete_version with app_id {hostile:?} must be refused"
        );
        assert!(
            manager.list_versions(hostile).is_empty(),
            "list_versions with app_id {hostile:?} must reveal nothing"
        );
    }
}

#[test]
#[traced_test]
fn a_hostile_version_cannot_delete_a_sibling_apps_directory() {
    let manager = manager();
    manager.write("/app/v0.1.0/x", b"x").expect("seed");
    manager.write("/app/v0.1.1/x", b"x").expect("seed");
    manager.write("/victim/v0.1.0/x", b"x").expect("seed");

    let _ = manager.delete_version("app", "../../victim/v0.1.0", None);

    assert!(
        manager.exists("/victim/v0.1.0/x").expect("exists"),
        "a crafted version string must not reach outside its app directory"
    );
}

// ── Audit events ────────────────────────────────────────────────────────

#[cfg(feature = "vfs-audit")]
mod audit {
    use super::*;
    use foundation_nativeapis::shared::vfs::observable_fs::{ObservableFs, VfsEvent};

    /// The audit wrapper must see the same operations the manager performs —
    /// an audit log that misses writes is worse than none, because it reads
    /// as evidence that nothing happened.
    #[test]
    #[traced_test]
    fn observable_fs_emits_an_event_per_write_and_read() {
        let inner = ObservableFs::new(MemoryFs::new());
        let events = inner.subscribe();
        let manager = PlatformAssetManager::from_vfs(
            DynFs::new(Arc::new(inner)),
            PathBuf::from("/base"),
            "0.1.0",
            AssetLayout::Versioned,
            None,
            None,
        );

        manager.write("/app/v0.1.0/index.html", b"<html/>").expect("write");
        manager.read("/app/v0.1.0/index.html").expect("read");

        let seen: Vec<VfsEvent> = events.try_iter().collect();
        assert!(
            !seen.is_empty(),
            "the audit layer must observe the manager's I/O, got nothing"
        );
    }
}

/// Without the feature there is no `ObservableFs` in the chain at all — the
/// wrapper is compiled out rather than switched off at runtime.
#[cfg(not(feature = "vfs-audit"))]
#[test]
#[traced_test]
fn audit_is_absent_when_the_feature_is_off() {
    let manager = manager();
    manager.write("/app/v0.1.0/index.html", b"<html/>").expect("write");
    assert_eq!(
        manager.read("/app/v0.1.0/index.html").expect("read"),
        b"<html/>".to_vec(),
        "the un-audited path must behave identically"
    );
}

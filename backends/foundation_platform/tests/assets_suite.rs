//! F40 — `PlatformAssetManager`: layout, VFS I/O, overlay semantics, and
//! per-app version management.
//!
//! Everything here runs on the host. `MemoryFs` covers the Flat and Versioned
//! layouts, and `OverlayFileSystem<AssetResolverFs, MemoryDelta>` reproduces
//! the Android arrangement — an immutable bundle underneath, a writable delta
//! on top — without an APK or a device.

use std::collections::HashMap;
use std::sync::Arc;

use foundation_nativeapis::shared::vfs::dynfs::DynFs;
use foundation_nativeapis::shared::vfs::error::{VfsError, VfsResult};
use foundation_nativeapis::shared::vfs::memory_delta::MemoryDelta;
use foundation_nativeapis::shared::vfs::memory_fs::MemoryFs;
use foundation_nativeapis::shared::vfs::overlay_fs::OverlayFileSystem;
use foundation_nativeapis::shared::vfs::traits::{VfsDirectory, VfsFileSystem};
use foundation_platform::assets::{
    parse_semver, AssetLayout, AssetResolverFs, AssetSource, PlatformAssetManager,
};
use tracing_test::traced_test;

// ── Fixtures ────────────────────────────────────────────────────────────

/// An [`AssetSource`] over a fixed map, standing in for the APK bundle.
struct BundledAssets(HashMap<String, Vec<u8>>);

impl BundledAssets {
    fn new(entries: &[(&str, &str)]) -> Arc<Self> {
        Arc::new(Self(
            entries
                .iter()
                .map(|(k, v)| (normalize(k), v.as_bytes().to_vec()))
                .collect(),
        ))
    }
}

fn normalize(path: &str) -> String {
    format!("/{}", path.trim_start_matches('/'))
}

impl AssetSource for BundledAssets {
    fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.0.keys().cloned().collect();
        keys.sort();
        keys
    }

    fn fetch(&self, path: &str) -> VfsResult<Vec<u8>> {
        self.0
            .get(&normalize(path))
            .cloned()
            .ok_or_else(|| VfsError::NotFound { path: normalize(path) }.into())
    }
}

/// A manager over an empty in-memory filesystem.
fn memory_manager(version: &str, layout: AssetLayout) -> PlatformAssetManager {
    PlatformAssetManager::from_vfs(
        DynFs::new(Arc::new(MemoryFs::new())),
        std::path::PathBuf::from("/base"),
        version,
        layout,
        Some("cdn.test".to_string()),
        None,
    )
}

/// A manager over the Android arrangement: read-only bundle + writable delta.
fn overlay_manager(version: &str, bundle: &[(&str, &str)]) -> PlatformAssetManager {
    let base = AssetResolverFs::new(BundledAssets::new(bundle));
    let overlay = OverlayFileSystem::new(base, MemoryDelta::new());
    PlatformAssetManager::from_vfs(
        DynFs::new(Arc::new(overlay)),
        std::path::PathBuf::from("/data/app"),
        version,
        AssetLayout::Versioned,
        Some("cdn.test".to_string()),
        None,
    )
}

/// Create the version directories a test needs, as `initialize()` would.
fn seed_versions(manager: &PlatformAssetManager, app_id: &str, versions: &[&str]) {
    for version in versions {
        manager
            .write(
                &format!("/{app_id}/v{version}/index.html"),
                format!("<html>{app_id}@{version}</html>").as_bytes(),
            )
            .expect("seed version directory");
    }
}

// ── Layout selection ────────────────────────────────────────────────────

#[test]
#[traced_test]
fn flat_layout_app_root_has_no_version_segment() {
    let manager = memory_manager("0.1.0", AssetLayout::Flat);
    assert_eq!(
        manager.app_root("app"),
        std::path::PathBuf::from("/base/app"),
        "desktop and iOS serve straight out of resource_dir()/{{app}}"
    );
}

#[test]
#[traced_test]
fn flat_layout_reports_no_versions_and_ignores_activation() {
    let manager = memory_manager("0.1.0", AssetLayout::Flat);
    seed_versions(&manager, "app", &["0.1.0", "0.2.0"]);

    assert!(
        manager.list_versions("app").is_empty(),
        "Flat has no version directories to list, whatever is on disk"
    );
    manager
        .activate("app", "0.2.0")
        .expect("activation is a no-op, not an error, under Flat");
    assert_eq!(
        manager.app_root("app"),
        std::path::PathBuf::from("/base/app"),
        "a no-op activation must not move the root"
    );
}

#[test]
#[traced_test]
fn versioned_layout_app_root_tracks_the_active_version() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0", "0.1.1"]);

    assert_eq!(
        manager.app_root("app"),
        std::path::PathBuf::from("/base/app/v0.1.0"),
        "a fresh install serves the version the binary shipped with"
    );

    manager.activate("app", "0.1.1").expect("activate");
    assert_eq!(
        manager.app_root("app"),
        std::path::PathBuf::from("/base/app/v0.1.1"),
    );
}

#[test]
#[traced_test]
fn activating_one_app_leaves_every_other_app_alone() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0", "0.1.1"]);
    seed_versions(&manager, "app-hello", &["0.1.0"]);

    manager.activate("app", "0.1.1").expect("activate");

    assert_eq!(manager.active_version("app"), "0.1.1");
    assert_eq!(
        manager.active_version("app-hello"),
        "0.1.0",
        "each app owns its own lifecycle — that is the point of app-first layout"
    );
}

// ── VFS read / write ────────────────────────────────────────────────────

#[test]
#[traced_test]
fn write_then_read_round_trips_and_creates_parent_directories() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);

    manager
        .write("/app/v0.1.0/nested/deep/bundle.js", b"console.log(1)")
        .expect("write through nonexistent parents");

    assert_eq!(
        manager.read("/app/v0.1.0/nested/deep/bundle.js").expect("read"),
        b"console.log(1)".to_vec()
    );
    assert!(manager.exists("/app/v0.1.0/nested/deep").expect("exists"));
}

#[test]
#[traced_test]
fn reading_a_missing_file_reports_not_found() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    assert!(manager.read("/app/v0.1.0/absent.js").is_err());
    assert!(!manager.exists("/app/v0.1.0/absent.js").expect("exists"));
}

#[test]
#[traced_test]
fn read_app_file_resolves_against_the_active_version() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    manager.write("/app/v0.1.0/bundle.js", b"old").expect("write v0.1.0");
    manager.write("/app/v0.1.1/bundle.js", b"new").expect("write v0.1.1");

    assert_eq!(manager.read_app_file("app", "bundle.js").expect("read"), b"old".to_vec());

    manager.activate("app", "0.1.1").expect("activate");
    assert_eq!(
        manager.read_app_file("app", "bundle.js").expect("read"),
        b"new".to_vec(),
        "the same relative path must follow the app's active version"
    );
}

// ── Overlay semantics ───────────────────────────────────────────────────

#[test]
#[traced_test]
fn overlay_reads_the_bundle_when_the_delta_is_empty() {
    let manager = overlay_manager("0.1.0", &[("/app/v0.1.0/index.html", "<html>apk</html>")]);

    assert_eq!(
        manager.read_app_file("app", "index.html").expect("read from bundle"),
        b"<html>apk</html>".to_vec(),
        "nothing was extracted, so this can only have come from the base layer"
    );
}

#[test]
#[traced_test]
fn overlay_delta_shadows_the_bundle() {
    let manager = overlay_manager("0.1.0", &[("/app/v0.1.0/index.html", "<html>apk</html>")]);

    manager
        .write("/app/v0.1.0/index.html", b"<html>ota</html>")
        .expect("write to delta");

    assert_eq!(
        manager.read_app_file("app", "index.html").expect("read"),
        b"<html>ota</html>".to_vec(),
        "an OTA'd file must win over the copy inside the bundle"
    );
}

#[test]
#[traced_test]
fn overlay_write_leaves_the_bundle_untouched() {
    let bundle = BundledAssets::new(&[("/app/v0.1.0/index.html", "<html>apk</html>")]);
    let base = AssetResolverFs::new(Arc::clone(&bundle) as Arc<dyn AssetSource>);
    let overlay = OverlayFileSystem::new(base, MemoryDelta::new());
    let manager = PlatformAssetManager::from_vfs(
        DynFs::new(Arc::new(overlay)),
        std::path::PathBuf::from("/data/app"),
        "0.1.0",
        AssetLayout::Versioned,
        None,
        None,
    );

    manager
        .write("/app/v0.1.0/index.html", b"<html>ota</html>")
        .expect("write");

    assert_eq!(
        bundle.fetch("/app/v0.1.0/index.html").expect("bundle still readable"),
        b"<html>apk</html>".to_vec(),
        "copy-on-write must not mutate the read-only base"
    );
}

#[test]
#[traced_test]
fn overlay_serves_a_file_only_the_delta_has() {
    let manager = overlay_manager("0.1.0", &[("/app/v0.1.0/index.html", "<html>apk</html>")]);
    manager
        .write("/app/v0.1.0/added-by-ota.js", b"new file")
        .expect("write");

    assert_eq!(
        manager.read_app_file("app", "added-by-ota.js").expect("read"),
        b"new file".to_vec()
    );
}

// ── AssetResolverFs ─────────────────────────────────────────────────────

#[test]
#[traced_test]
fn asset_resolver_fs_reads_and_reports_existence() {
    let fs = AssetResolverFs::new(BundledAssets::new(&[("app/index.html", "<html/>")]));

    assert_eq!(fs.read_file("/app/index.html").expect("read"), b"<html/>".to_vec());
    assert!(fs.exists("/app/index.html").expect("exists"));
    assert!(!fs.exists("/app/absent.html").expect("exists"));
    assert!(
        fs.exists("/app").expect("exists"),
        "a directory prefix must exist even though no key names it — an APK \
         stores a flat list with no directory entries"
    );
}

#[test]
#[traced_test]
fn asset_resolver_fs_lists_immediate_children_only() {
    let fs = AssetResolverFs::new(BundledAssets::new(&[
        ("app/index.html", "a"),
        ("app/nested/deep.js", "b"),
        ("app-hello/index.html", "c"),
    ]));

    let dir = fs.open_directory("/app").expect("open /app");
    let mut names: Vec<String> = dir.list().expect("list").into_iter().map(|e| e.name).collect();
    names.sort();

    assert_eq!(
        names,
        vec!["index.html", "nested"],
        "`nested` is a synthesized directory; `app-hello` is a sibling, not a child"
    );
}

#[test]
#[traced_test]
fn asset_resolver_fs_rejects_every_write() {
    let fs = AssetResolverFs::new(BundledAssets::new(&[("app/index.html", "<html/>")]));

    assert!(fs.write_file("/app/index.html", b"tampered").is_err());
    assert!(fs.create("/app/new.js", 0o644).is_err());
    assert!(fs.remove("/app/index.html").is_err());
    assert!(fs.mkdir("/app/sub").is_err());
    assert!(
        fs.rename("/app/index.html", "/app/other.html").is_err(),
        "the bundle is the shipped artefact — nothing may edit it in place"
    );
}

// ── Version listing and semver ordering ─────────────────────────────────

#[test]
#[traced_test]
fn list_versions_sorts_numerically_not_lexically() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.2.0", "0.10.0", "0.1.0"]);

    assert_eq!(
        manager.list_versions("app"),
        vec!["0.1.0", "0.2.0", "0.10.0"],
        "lexical order would put v0.10.0 before v0.2.0 and prune the newest release"
    );
}

#[test]
#[traced_test]
fn list_versions_handles_multi_digit_components() {
    let manager = memory_manager("1.0.0", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["1.9.9", "1.10.0", "1.10.10", "2.0.0"]);

    assert_eq!(
        manager.list_versions("app"),
        vec!["1.9.9", "1.10.0", "1.10.10", "2.0.0"]
    );
}

#[test]
#[traced_test]
fn list_versions_ignores_entries_that_are_not_versions() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0"]);
    manager.write("/app/vnot-semver/x", b"x").expect("write");
    manager.write("/app/plain/x", b"x").expect("write");
    manager.write("/app/loose.txt", b"x").expect("write");

    assert_eq!(
        manager.list_versions("app"),
        vec!["0.1.0"],
        "only `v` + semver directories count as versions"
    );
}

#[test]
#[traced_test]
fn list_versions_is_empty_for_an_unknown_app() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    assert!(manager.list_versions("never-installed").is_empty());
}

#[test]
#[traced_test]
fn list_apps_returns_every_app_directory_sorted() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    seed_versions(&manager, "app-settings", &["0.2.0"]);
    seed_versions(&manager, "app", &["0.1.0"]);
    seed_versions(&manager, "app-hello", &["0.1.0"]);

    assert_eq!(
        manager.list_apps(),
        vec!["app", "app-hello", "app-settings"]
    );
}

#[test]
#[traced_test]
fn parse_semver_accepts_a_v_prefix_and_rejects_everything_malformed() {
    assert_eq!(parse_semver("0.1.2"), Some((0, 1, 2)));
    assert_eq!(parse_semver("v0.1.2"), Some((0, 1, 2)), "the v prefix is internal");
    assert_eq!(parse_semver("10.20.30"), Some((10, 20, 30)));

    assert_eq!(parse_semver("0.1"), None, "three components are required");
    assert_eq!(parse_semver("0.1.2.3"), None, "four components are not semver");
    assert_eq!(parse_semver("0.1.x"), None);
    assert_eq!(parse_semver(""), None);
    assert_eq!(parse_semver("../etc"), None);
}

// ── Activation ──────────────────────────────────────────────────────────

#[test]
#[traced_test]
fn activate_refuses_a_version_that_is_not_installed() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0"]);

    let err = manager
        .activate("app", "9.9.9")
        .expect_err("activating a version we never installed must fail");
    assert!(err.contains("does not exist"), "got {err:?}");
    assert_eq!(
        manager.active_version("app"),
        "0.1.0",
        "a refused activation must leave the previous one in place"
    );
}

#[test]
#[traced_test]
fn activate_refuses_a_version_string_that_is_not_semver() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0"]);

    assert!(manager.activate("app", "../../etc").is_err());
    assert!(manager.activate("app", "latest").is_err());
    assert!(manager.activate("app", "0.1.0\0").is_err());
}

#[test]
#[traced_test]
fn activate_refuses_an_app_id_that_could_escape_its_directory() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    assert!(manager.activate("../other", "0.1.0").is_err());
    assert!(manager.activate("a/b", "0.1.0").is_err());
    assert!(manager.activate("", "0.1.0").is_err());
}

// ── Pruning ─────────────────────────────────────────────────────────────

#[test]
#[traced_test]
fn prune_keeps_exactly_the_requested_number_of_newest_versions() {
    let manager = memory_manager("0.1.3", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0", "0.1.1", "0.1.2", "0.1.3"]);

    manager.prune("app", 2).expect("prune");

    assert_eq!(manager.list_versions("app"), vec!["0.1.2", "0.1.3"]);
}

#[test]
#[traced_test]
fn prune_uses_semver_order_so_v0_10_survives_v0_2() {
    let manager = memory_manager("0.10.0", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0", "0.2.0", "0.10.0"]);

    manager.prune("app", 2).expect("prune");

    assert_eq!(
        manager.list_versions("app"),
        vec!["0.2.0", "0.10.0"],
        "lexical pruning would have deleted v0.10.0, the newest release"
    );
}

#[test]
#[traced_test]
fn prune_never_deletes_the_active_version_however_old() {
    let manager = memory_manager("0.1.3", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0", "0.1.1", "0.1.2", "0.1.3"]);
    manager.activate("app", "0.1.0").expect("roll back to the oldest");

    manager.prune("app", 2).expect("prune");

    let remaining = manager.list_versions("app");
    assert!(
        remaining.contains(&"0.1.0".to_string()),
        "pruning the running version would delete the app out from under itself; got {remaining:?}"
    );
    assert_eq!(remaining.len(), 2, "the keep count still applies: {remaining:?}");
}

#[test]
#[traced_test]
fn prune_is_per_app() {
    let manager = memory_manager("0.1.2", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0", "0.1.1", "0.1.2"]);
    seed_versions(&manager, "app-hello", &["0.1.0"]);

    manager.prune("app", 2).expect("prune");

    assert_eq!(manager.list_versions("app"), vec!["0.1.1", "0.1.2"]);
    assert_eq!(
        manager.list_versions("app-hello"),
        vec!["0.1.0"],
        "one app's prune must not touch another's rollback target"
    );
}

#[test]
#[traced_test]
fn prune_on_an_unknown_app_or_flat_layout_is_a_no_op() {
    let versioned = memory_manager("0.1.0", AssetLayout::Versioned);
    versioned.prune("never-installed", 2).expect("must not error");

    let flat = memory_manager("0.1.0", AssetLayout::Flat);
    seed_versions(&flat, "app", &["0.1.0", "0.1.1", "0.1.2"]);
    flat.prune("app", 1).expect("must not error");
    assert!(
        flat.exists("/app/v0.1.0/index.html").expect("exists"),
        "Flat has no version lifecycle to prune, so nothing may be deleted"
    );
}

// ── Deletion ────────────────────────────────────────────────────────────

#[test]
#[traced_test]
fn delete_version_refuses_the_active_version() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0", "0.1.1"]);

    let err = manager
        .delete_version("app", "0.1.0", None)
        .expect_err("deleting what we are serving must be refused");
    assert!(err.contains("active"), "got {err:?}");
}

#[test]
#[traced_test]
fn delete_version_refuses_the_rollback_target() {
    let manager = memory_manager("0.1.2", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0", "0.1.2"]);

    let err = manager
        .delete_version("app", "0.1.0", Some("0.1.0"))
        .expect_err("deleting the version we are about to roll back to must be refused");
    assert!(err.contains("rollback"), "got {err:?}");
}

#[test]
#[traced_test]
fn delete_version_refuses_to_leave_an_app_with_nothing() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0"]);
    manager.activate("app", "0.1.0").expect("activate");

    // Even naming a different active version, the last directory must stay.
    let only = memory_manager("0.2.0", AssetLayout::Versioned);
    seed_versions(&only, "app", &["0.1.0"]);
    let err = only
        .delete_version("app", "0.1.0", None)
        .expect_err("an app with no version directory can serve nothing");
    assert!(err.contains("only version"), "got {err:?}");
}

#[test]
#[traced_test]
fn delete_version_that_does_not_exist_is_a_no_op_not_an_error() {
    let manager = memory_manager("0.1.0", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0", "0.1.1"]);

    manager
        .delete_version("app", "9.9.9", None)
        .expect("a stale delete_after must not abort the rest of an OTA");
    assert_eq!(manager.list_versions("app"), vec!["0.1.0", "0.1.1"]);
}

#[test]
#[traced_test]
fn delete_version_removes_the_whole_directory() {
    let manager = memory_manager("0.1.1", AssetLayout::Versioned);
    seed_versions(&manager, "app", &["0.1.0", "0.1.1"]);
    manager
        .write("/app/v0.1.0/.ewe_manifest.json", b"{}")
        .expect("seed manifest");

    manager.delete_version("app", "0.1.0", None).expect("delete");

    assert_eq!(manager.list_versions("app"), vec!["0.1.1"]);
    assert!(
        !manager.exists("/app/v0.1.0/.ewe_manifest.json").expect("exists"),
        "a manifest must never outlive the version it describes"
    );
}

// ── Concurrency ─────────────────────────────────────────────────────────

#[test]
#[traced_test]
fn concurrent_reads_and_mutations_stay_consistent() {
    let manager = Arc::new(memory_manager("0.1.4", AssetLayout::Versioned));
    seed_versions(&manager, "app", &["0.1.0", "0.1.1", "0.1.2", "0.1.3", "0.1.4"]);

    let mut handles = Vec::new();
    for _ in 0..4 {
        let manager = Arc::clone(&manager);
        handles.push(std::thread::spawn(move || {
            for _ in 0..50 {
                // The active version is never pruned, so this read must
                // always succeed no matter how the prune below interleaves.
                manager
                    .read_app_file("app", "index.html")
                    .expect("the active version must stay readable throughout");
            }
        }));
    }
    {
        let manager = Arc::clone(&manager);
        handles.push(std::thread::spawn(move || {
            for _ in 0..10 {
                manager.prune("app", 2).expect("prune");
            }
        }));
    }

    for handle in handles {
        handle.join().expect("no thread may panic");
    }

    assert_eq!(manager.list_versions("app"), vec!["0.1.3", "0.1.4"]);
}

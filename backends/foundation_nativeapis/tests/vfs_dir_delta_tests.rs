#![cfg(feature = "vfs-native")]

use foundation_nativeapis::native::vfs::{DirectoryDelta, NativeFs};
use foundation_nativeapis::shared::vfs::{
    DeltaStore, MemoryDelta, OverlayFileSystem, VfsDirectory, VfsFile, VfsFileSystem,
};
use std::fs;
use std::path::PathBuf;
use tracing_test::traced_test;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "vfs_dir_delta_{name}_{}_{}", std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[traced_test]
fn test_write_creates_file_in_shadow() {
    let tmp = TempDir::new("write_shadow");
    let shadow = tmp.path().join("shadow");
    let delta = DirectoryDelta::new(&shadow).unwrap();

    delta.write_file("/test.txt", b"shadow data").unwrap();

    assert!(shadow.join("test.txt").exists());
    let on_disk = fs::read(shadow.join("test.txt")).unwrap();
    assert_eq!(on_disk, b"shadow data");
}

#[test]
#[traced_test]
fn test_whiteout_creates_sentinel() {
    let tmp = TempDir::new("sentinel");
    let shadow = tmp.path().join("shadow");
    let delta = DirectoryDelta::new(&shadow).unwrap();

    delta.add_whiteout("/deleted.txt", 5).unwrap();

    let sentinel = shadow.join(".wh.deleted.txt");
    assert!(sentinel.exists());
    let content = fs::read_to_string(&sentinel).unwrap();
    assert_eq!(content, "5");
}

#[test]
#[traced_test]
fn test_whiteout_check_from_cache() {
    let tmp = TempDir::new("cache");
    let shadow = tmp.path().join("shadow");
    let delta = DirectoryDelta::new(&shadow).unwrap();

    assert!(delta.is_whiteout("/foo.txt").unwrap().is_none());

    delta.add_whiteout("/foo.txt", 3).unwrap();
    assert_eq!(delta.is_whiteout("/foo.txt").unwrap(), Some(3));
}

#[test]
#[traced_test]
fn test_whiteout_inheritance() {
    let tmp = TempDir::new("inherit");
    let shadow = tmp.path().join("shadow");
    let delta = DirectoryDelta::new(&shadow).unwrap();

    delta.add_whiteout("/parent", 10).unwrap();
    assert_eq!(
        delta.is_whiteout("/parent/child.txt").unwrap(),
        Some(10)
    );
    assert_eq!(
        delta.is_whiteout("/parent/sub/deep.txt").unwrap(),
        Some(10)
    );
}

#[test]
#[traced_test]
fn test_whiteout_scan_on_construction() {
    let tmp = TempDir::new("scan");
    let shadow = tmp.path().join("shadow");
    fs::create_dir_all(&shadow).unwrap();

    // Manually create sentinel files
    fs::write(shadow.join(".wh.old.txt"), "7").unwrap();
    fs::create_dir_all(shadow.join("src")).unwrap();
    fs::write(shadow.join("src").join(".wh.removed.rs"), "12").unwrap();

    let delta = DirectoryDelta::new(&shadow).unwrap();
    assert_eq!(delta.is_whiteout("/old.txt").unwrap(), Some(7));
    assert_eq!(delta.is_whiteout("/src/removed.rs").unwrap(), Some(12));
}

#[test]
#[traced_test]
fn test_reset_clears_shadow() {
    let tmp = TempDir::new("reset");
    let shadow = tmp.path().join("shadow");
    let delta = DirectoryDelta::new(&shadow).unwrap();

    delta.write_file("/file.txt", b"data").unwrap();
    delta.add_whiteout("/gone.txt", 1).unwrap();

    delta.reset().unwrap();
    assert!(!delta.exists("/file.txt").unwrap());
    assert!(delta.is_whiteout("/gone.txt").unwrap().is_none());
}

#[test]
#[traced_test]
fn test_listing_excludes_sentinels() {
    let tmp = TempDir::new("filter");
    let shadow = tmp.path().join("shadow");
    let delta = DirectoryDelta::new(&shadow).unwrap();

    delta.write_file("/visible.txt", b"see me").unwrap();
    delta.add_whiteout("/hidden.txt", 1).unwrap();

    let dir = delta.open_directory("/").unwrap();
    let entries = dir.list().unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"visible.txt"));
    assert!(!names.iter().any(|n| n.starts_with(".wh.")));
}

#[test]
#[traced_test]
fn test_whiteout_path_traversal_rejected() {
    let tmp = TempDir::new("traversal");
    let shadow = tmp.path().join("shadow");
    let delta = DirectoryDelta::new(&shadow).unwrap();

    let result = delta.add_whiteout("/../../../etc/passwd", 1);
    assert!(result.is_err(), "path traversal in whiteout should be rejected");

    let result = delta.add_whiteout("/foo/../../etc/passwd", 1);
    assert!(result.is_err(), "path traversal in whiteout should be rejected");
}

#[test]
#[traced_test]
fn test_end_to_end_overlay_with_native_base() {
    let tmp = TempDir::new("e2e");
    let base_dir = tmp.path().join("base");
    let shadow_dir = tmp.path().join("shadow");
    fs::create_dir_all(&base_dir).unwrap();

    // Populate base
    fs::create_dir_all(base_dir.join("src")).unwrap();
    fs::write(base_dir.join("src/main.rs"), b"fn main() {}").unwrap();
    fs::write(base_dir.join("README.md"), b"# Hello").unwrap();

    let base = NativeFs::new(&base_dir).unwrap();
    let delta = DirectoryDelta::new(&shadow_dir).unwrap();
    let overlay = OverlayFileSystem::new(base, delta);

    // Read base files through overlay
    assert_eq!(
        overlay.read_file("/src/main.rs").unwrap(),
        b"fn main() {}"
    );

    // Create new file through overlay
    overlay.write_file("/new.txt", b"added").unwrap();
    assert_eq!(overlay.read_file("/new.txt").unwrap(), b"added");
    assert!(shadow_dir.join("new.txt").exists());

    // Modify base file through overlay (CoW)
    overlay
        .write_file("/src/main.rs", b"fn main() { modified }")
        .unwrap();
    assert_eq!(
        overlay.read_file("/src/main.rs").unwrap(),
        b"fn main() { modified }"
    );
    // Base unchanged
    assert_eq!(
        fs::read(base_dir.join("src/main.rs")).unwrap(),
        b"fn main() {}"
    );

    // Delete base file through overlay
    overlay.remove("/README.md").unwrap();
    assert!(!overlay.exists("/README.md").unwrap());
    // Base still has it
    assert!(base_dir.join("README.md").exists());
    // Whiteout sentinel exists
    assert!(shadow_dir.join(".wh.README.md").exists());

    // Reset delta — everything goes back to base
    overlay.delta().reset().unwrap();
    assert_eq!(
        overlay.read_file("/src/main.rs").unwrap(),
        b"fn main() {}"
    );
    assert!(overlay.exists("/README.md").unwrap());
    assert!(!overlay.exists("/new.txt").unwrap());
}

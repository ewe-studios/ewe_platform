//! Integration tests: OverlayFileSystem through valtron.
//!
//! These tests verify that OverlayFileSystem<SyncLibsqlDelta, MemoryDelta>
//! and OverlayFileSystem<MemoryFs, MemoryDelta> work correctly when all
//! operations go through valtron's executor.

#![cfg(feature = "vfs")]

use foundation_core::valtron::{initialize_pool, PoolGuard};
use foundation_nativeapis::shared::vfs::{
    DeltaStore, MemoryDelta, MemoryFs, OverlayFileSystem, VfsFileSystem,
};

fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

// ── Overlay<MemoryFs, MemoryDelta> via Valtron ──

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn overlay_memory_read_passthrough() {
    let _guard = init_pool();

    let base = MemoryFs::new();
    base.mkdir("/src").unwrap();
    base.write_file("/src/main.rs", b"fn main() {}").unwrap();
    base.write_file("/readme.md", b"# Project").unwrap();

    let delta = MemoryDelta::new();
    let overlay = OverlayFileSystem::new(base, delta);

    // Read from base through overlay
    let data = overlay.read_file("/src/main.rs").unwrap();
    assert_eq!(&data, b"fn main() {}");

    let data = overlay.read_file("/readme.md").unwrap();
    assert_eq!(&data, b"# Project");
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn overlay_memory_write_to_delta() {
    let _guard = init_pool();

    let base = MemoryFs::new();
    let delta = MemoryDelta::new();
    let overlay = OverlayFileSystem::new(base, delta);

    // Write goes to delta
    overlay.write_file("/new.txt", b"new content").unwrap();
    assert_eq!(overlay.read_file("/new.txt").unwrap(), b"new content");
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn overlay_memory_cow() {
    let _guard = init_pool();

    let base = MemoryFs::new();
    base.write_file("/file.txt", b"original").unwrap();

    let delta = MemoryDelta::new();
    let overlay = OverlayFileSystem::new(base, delta);

    // CoW: write to overlay copies base file to delta first
    overlay.write_file("/file.txt", b"modified").unwrap();

    // Read returns delta version
    assert_eq!(overlay.read_file("/file.txt").unwrap(), b"modified");

    assert!(overlay.version() > 0);
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn overlay_memory_whiteout_hides_base() {
    let _guard = init_pool();

    let base = MemoryFs::new();
    base.write_file("/old.txt", b"should be hidden").unwrap();
    base.mkdir("/old_dir").unwrap();

    let delta = MemoryDelta::new();
    delta.add_whiteout("/old.txt", 1).unwrap();

    let overlay = OverlayFileSystem::new(base, delta);

    // Whiteout hides base file
    assert!(!overlay.exists("/old.txt").unwrap());

    // Reading should fail
    assert!(overlay.read_file("/old.txt").is_err());
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn overlay_memory_directory_listing() {
    let _guard = init_pool();

    let base = MemoryFs::new();
    base.mkdir("/dir").unwrap();
    base.write_file("/dir/base.txt", b"from base").unwrap();

    let delta = MemoryDelta::new();
    delta.add_whiteout("/dir/base.txt", 1).unwrap();

    // Write a new file to delta at same path
    delta.write_file("/dir/base.txt", b"from delta").unwrap();
    delta.remove_whiteout("/dir/base.txt").unwrap();

    let overlay = OverlayFileSystem::new(base, delta);

    // Delta version overrides base
    assert_eq!(overlay.read_file("/dir/base.txt").unwrap(), b"from delta");
}

// ── Overlay with Nested Dirs ──

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn overlay_memory_nested_dirs() {
    let _guard = init_pool();

    let base = MemoryFs::new();
    base.mkdir("/a").unwrap();
    base.mkdir("/a/b").unwrap();
    base.write_file("/a/b/c.txt", b"nested").unwrap();

    let delta = MemoryDelta::new();
    let overlay = OverlayFileSystem::new(base, delta);

    // Read nested
    assert_eq!(overlay.read_file("/a/b/c.txt").unwrap(), b"nested");

    // Write nested
    overlay.write_file("/a/b/d.txt", b"also nested").unwrap();
    assert_eq!(overlay.read_file("/a/b/d.txt").unwrap(), b"also nested");
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn overlay_memory_remove_all() {
    let _guard = init_pool();

    let base = MemoryFs::new();
    base.mkdir("/project").unwrap();
    base.write_file("/project/a.txt", b"a").unwrap();
    base.write_file("/project/b.txt", b"b").unwrap();

    let delta = MemoryDelta::new();
    let overlay = OverlayFileSystem::new(base, delta);

    overlay.remove_all("/project").unwrap();
    assert!(!overlay.exists("/project").unwrap());
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn overlay_memory_mkdir_all() {
    let _guard = init_pool();

    let base = MemoryFs::new();
    let delta = MemoryDelta::new();
    let overlay = OverlayFileSystem::new(base, delta);

    overlay.mkdir_all("/deep/nested/path").unwrap();
    assert!(overlay.exists("/deep").unwrap());
    assert!(overlay.exists("/deep/nested").unwrap());
    assert!(overlay.exists("/deep/nested/path").unwrap());
}

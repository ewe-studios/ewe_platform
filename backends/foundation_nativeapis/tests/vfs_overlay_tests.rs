use foundation_nativeapis::shared::vfs::{
    DeltaStore, MemoryDelta, MemoryFs, OpenMode, OverlayFileSystem, VfsDirectory, VfsFile,
    VfsFileSystem,
};
use tracing_test::traced_test;

fn setup() -> OverlayFileSystem<MemoryFs, MemoryDelta> {
    let base = MemoryFs::new();
    let delta = MemoryDelta::new();
    OverlayFileSystem::new(base, delta)
}

fn setup_with_base_files() -> OverlayFileSystem<MemoryFs, MemoryDelta> {
    let base = MemoryFs::new();
    base.mkdir("/src").unwrap();
    base.write_file("/src/main.rs", b"fn main() {}").unwrap();
    base.write_file("/src/lib.rs", b"pub mod lib;").unwrap();
    base.write_file("/readme.md", b"# Project").unwrap();

    let delta = MemoryDelta::new();
    OverlayFileSystem::new(base, delta)
}

// ── Read passthrough ──

#[test]
#[traced_test]
fn test_read_base_file() {
    let overlay = setup_with_base_files();
    let data = overlay.read_file("/src/main.rs").unwrap();
    assert_eq!(data, b"fn main() {}");
}

#[test]
#[traced_test]
fn test_read_base_directory() {
    let overlay = setup_with_base_files();
    let dir = overlay.open_directory("/src").unwrap();
    let entries = dir.list().unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"main.rs"));
    assert!(names.contains(&"lib.rs"));
}

#[test]
#[traced_test]
fn test_stat_base_file() {
    let overlay = setup_with_base_files();
    let meta = overlay.stat("/readme.md").unwrap();
    assert_eq!(meta.size, 9); // "# Project" = 9 bytes
}

#[test]
#[traced_test]
fn test_exists_base_file() {
    let overlay = setup_with_base_files();
    assert!(overlay.exists("/src/main.rs").unwrap());
    assert!(!overlay.exists("/nope.txt").unwrap());
}

// ── Write to delta ──

#[test]
#[traced_test]
fn test_create_new_file() {
    let overlay = setup_with_base_files();
    overlay.write_file("/new.txt", b"new content").unwrap();

    assert!(overlay.exists("/new.txt").unwrap());
    assert_eq!(overlay.read_file("/new.txt").unwrap(), b"new content");

    // Not in base
    assert!(!overlay.base().exists("/new.txt").unwrap());
    // In delta
    assert!(overlay.delta().exists("/new.txt").unwrap());
}

#[test]
#[traced_test]
fn test_mkdir_through_overlay() {
    let overlay = setup_with_base_files();
    overlay.mkdir("/build").unwrap();
    assert!(overlay.exists("/build").unwrap());

    let dir = overlay.open_directory("/").unwrap();
    let entries = dir.list().unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"build"));
    assert!(names.contains(&"src"));
    assert!(names.contains(&"readme.md"));
}

#[test]
#[traced_test]
fn test_write_file_convenience() {
    let overlay = setup();
    overlay.write_file("/data.bin", b"binary data").unwrap();
    let data = overlay.read_file("/data.bin").unwrap();
    assert_eq!(data, b"binary data");
}

// ── Copy-on-Write ──

#[test]
#[traced_test]
fn test_cow_on_write() {
    let overlay = setup_with_base_files();

    let file = overlay.open("/src/main.rs", OpenMode::Write).unwrap();
    file.write_at(b"fn main() { println!(\"hello\"); }", 0)
        .unwrap();

    // Base unchanged
    let base_data = overlay.base().read_file("/src/main.rs").unwrap();
    assert_eq!(base_data, b"fn main() {}");

    // Delta has modified copy
    let overlay_data = overlay.read_file("/src/main.rs").unwrap();
    assert_eq!(overlay_data, b"fn main() { println!(\"hello\"); }");
}

#[test]
#[traced_test]
fn test_cow_preserves_content() {
    let overlay = setup_with_base_files();

    // Open for write triggers CoW — delta should start with base content
    let file = overlay.open("/readme.md", OpenMode::ReadWrite).unwrap();
    let mut buf = vec![0u8; 20];
    let n = file.read_at(&mut buf, 0).unwrap();
    assert_eq!(&buf[..n], b"# Project");
}

#[test]
#[traced_test]
fn test_cow_preserves_metadata() {
    let overlay = setup_with_base_files();
    let base_meta = overlay.base().stat("/readme.md").unwrap();

    let _file = overlay.open("/readme.md", OpenMode::Write).unwrap();
    let delta_meta = overlay.delta().stat("/readme.md").unwrap();

    assert_eq!(delta_meta.permissions, base_meta.permissions);
}

#[test]
#[traced_test]
fn test_cow_creates_parent_dirs() {
    let base = MemoryFs::new();
    base.mkdir("/a").unwrap();
    base.mkdir("/a/b").unwrap();
    base.write_file("/a/b/deep.txt", b"deep").unwrap();

    let delta = MemoryDelta::new();
    let overlay = OverlayFileSystem::new(base, delta);

    let _file = overlay.open("/a/b/deep.txt", OpenMode::Write).unwrap();

    // Delta should now have /a and /a/b directories
    assert!(overlay.delta().exists("/a").unwrap());
    assert!(overlay.delta().exists("/a/b").unwrap());
    assert!(overlay.delta().exists("/a/b/deep.txt").unwrap());
}

// ── Delete (whiteout) ──

#[test]
#[traced_test]
fn test_delete_base_file() {
    let overlay = setup_with_base_files();
    overlay.remove("/readme.md").unwrap();

    assert!(!overlay.exists("/readme.md").unwrap());
    // Base still has it
    assert!(overlay.base().exists("/readme.md").unwrap());
    // Delta has whiteout
    assert!(overlay.delta().is_whiteout("/readme.md").unwrap().is_some());
}

#[test]
#[traced_test]
fn test_delete_delta_file() {
    let overlay = setup();
    overlay.write_file("/tmp.txt", b"temp").unwrap();
    overlay.remove("/tmp.txt").unwrap();
    assert!(!overlay.exists("/tmp.txt").unwrap());
    // No whiteout needed (not in base)
    assert!(overlay.delta().is_whiteout("/tmp.txt").unwrap().is_none());
}

#[test]
#[traced_test]
fn test_delete_cow_file() {
    let overlay = setup_with_base_files();

    // CoW the file first
    let _file = overlay.open("/src/main.rs", OpenMode::Write).unwrap();
    drop(_file);

    // Now delete
    overlay.remove("/src/main.rs").unwrap();
    assert!(!overlay.exists("/src/main.rs").unwrap());

    // Whiteout exists (was in base)
    assert!(overlay
        .delta()
        .is_whiteout("/src/main.rs")
        .unwrap()
        .is_some());
}

#[test]
#[traced_test]
fn test_delete_returns_not_found() {
    let overlay = setup();
    let result = overlay.remove("/nonexistent.txt");
    assert!(result.is_err());
}

// ── Whiteout + recreation ──

#[test]
#[traced_test]
fn test_create_at_whiteout_path() {
    let overlay = setup_with_base_files();
    overlay.remove("/readme.md").unwrap();
    assert!(!overlay.exists("/readme.md").unwrap());

    // Recreate
    overlay.write_file("/readme.md", b"# New Readme").unwrap();
    assert!(overlay.exists("/readme.md").unwrap());
    assert_eq!(overlay.read_file("/readme.md").unwrap(), b"# New Readme");
}

#[test]
#[traced_test]
fn test_directory_whiteout_hides_children() {
    let overlay = setup_with_base_files();
    overlay.remove("/src/main.rs").unwrap();
    overlay.remove("/src/lib.rs").unwrap();
    overlay.remove("/src").unwrap();

    assert!(!overlay.exists("/src").unwrap());
    assert!(!overlay.exists("/src/main.rs").unwrap());
    assert!(!overlay.exists("/src/lib.rs").unwrap());
}

#[test]
#[traced_test]
fn test_recreate_in_whiteout_directory() {
    let overlay = setup_with_base_files();

    // Delete all src children first, then the dir
    overlay.remove("/src/main.rs").unwrap();
    overlay.remove("/src/lib.rs").unwrap();
    overlay.remove("/src").unwrap();
    assert!(!overlay.exists("/src").unwrap());

    // Recreate src dir and a new file
    overlay.mkdir("/src").unwrap();
    overlay.write_file("/src/new.rs", b"new file").unwrap();

    assert!(overlay.exists("/src").unwrap());
    assert!(overlay.exists("/src/new.rs").unwrap());
    // Old base files still hidden by whiteouts
    assert!(!overlay.exists("/src/main.rs").unwrap());
}

// ── Directory merging ──

#[test]
#[traced_test]
fn test_merge_base_and_delta_entries() {
    let overlay = setup_with_base_files();
    overlay.write_file("/src/new.rs", b"added").unwrap();

    let dir = overlay.open_directory("/src").unwrap();
    let entries = dir.list().unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"main.rs"));
    assert!(names.contains(&"lib.rs"));
    assert!(names.contains(&"new.rs"));
    assert_eq!(entries.len(), 3);
}

#[test]
#[traced_test]
fn test_merge_excludes_whiteouts() {
    let overlay = setup_with_base_files();
    overlay.remove("/src/main.rs").unwrap();

    let dir = overlay.open_directory("/src").unwrap();
    let entries = dir.list().unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(!names.contains(&"main.rs"));
    assert!(names.contains(&"lib.rs"));
}

#[test]
#[traced_test]
fn test_merge_delta_shadows_base() {
    let overlay = setup_with_base_files();
    // Modify base file via overlay (CoW)
    overlay
        .write_file("/src/main.rs", b"fn main() { modified }")
        .unwrap();

    let dir = overlay.open_directory("/src").unwrap();
    let entries = dir.list().unwrap();
    // main.rs should appear exactly once (from delta, shadowing base)
    let main_count = entries.iter().filter(|e| e.name == "main.rs").count();
    assert_eq!(main_count, 1);
}

#[test]
#[traced_test]
fn test_list_nonexistent_dir() {
    let overlay = setup();
    let result = overlay.open_directory("/nope");
    assert!(result.is_err());
}

// ── Rename ──

#[test]
#[traced_test]
fn test_rename_delta_file() {
    let overlay = setup();
    overlay.write_file("/old.txt", b"data").unwrap();
    overlay.rename("/old.txt", "/new.txt").unwrap();

    assert!(!overlay.exists("/old.txt").unwrap());
    assert_eq!(overlay.read_file("/new.txt").unwrap(), b"data");
}

#[test]
#[traced_test]
fn test_rename_base_file() {
    let overlay = setup_with_base_files();
    overlay.rename("/readme.md", "/README.md").unwrap();

    assert!(!overlay.exists("/readme.md").unwrap());
    assert!(overlay.exists("/README.md").unwrap());
    assert_eq!(overlay.read_file("/README.md").unwrap(), b"# Project");
    // Base still has original
    assert!(overlay.base().exists("/readme.md").unwrap());
    // Whiteout on old path
    assert!(overlay
        .delta()
        .is_whiteout("/readme.md")
        .unwrap()
        .is_some());
}

#[test]
#[traced_test]
fn test_rename_to_existing_path() {
    let overlay = setup_with_base_files();
    let result = overlay.rename("/readme.md", "/src/main.rs");
    assert!(result.is_err());
}

#[test]
#[traced_test]
fn test_rename_nonexistent() {
    let overlay = setup();
    let result = overlay.rename("/nope.txt", "/dest.txt");
    assert!(result.is_err());
}

// ── Reset / lifecycle ──

#[test]
#[traced_test]
fn test_reset_restores_base() {
    let overlay = setup_with_base_files();

    // Make changes
    overlay.remove("/readme.md").unwrap();
    overlay.write_file("/new.txt", b"new").unwrap();

    // Reset delta
    overlay.delta().reset().unwrap();

    // Base files visible again
    assert!(overlay.exists("/readme.md").unwrap());
    assert_eq!(overlay.read_file("/readme.md").unwrap(), b"# Project");
    // Delta files gone
    assert!(!overlay.exists("/new.txt").unwrap());
}

#[test]
#[traced_test]
fn test_version_after_reset() {
    let overlay = setup_with_base_files();
    overlay.write_file("/a.txt", b"data").unwrap();
    let v_before = overlay.version();
    assert!(v_before > 0);

    overlay.delta().reset().unwrap();
    let v_after = overlay.version();
    // Version counter is NOT reset — it's monotonic for the overlay lifetime
    assert!(v_after >= v_before);
}

// ── Stacked overlays ──

#[test]
#[traced_test]
fn test_stacked_overlay() {
    let base = MemoryFs::new();
    base.write_file("/base.txt", b"base data").unwrap();

    let inner_delta = MemoryDelta::new();
    let inner = OverlayFileSystem::new(base, inner_delta);
    inner.write_file("/inner.txt", b"inner data").unwrap();

    let outer_delta = MemoryDelta::new();
    let outer = OverlayFileSystem::new(inner, outer_delta);
    outer.write_file("/outer.txt", b"outer data").unwrap();

    assert_eq!(outer.read_file("/base.txt").unwrap(), b"base data");
    assert_eq!(outer.read_file("/inner.txt").unwrap(), b"inner data");
    assert_eq!(outer.read_file("/outer.txt").unwrap(), b"outer data");
}

#[test]
#[traced_test]
fn test_stacked_whiteouts() {
    let base = MemoryFs::new();
    base.write_file("/hidden.txt", b"data").unwrap();

    let inner_delta = MemoryDelta::new();
    let inner = OverlayFileSystem::new(base, inner_delta);
    inner.remove("/hidden.txt").unwrap();

    let outer_delta = MemoryDelta::new();
    let outer = OverlayFileSystem::new(inner, outer_delta);

    assert!(!outer.exists("/hidden.txt").unwrap());
}

// ── Edge cases ──

#[test]
#[traced_test]
fn test_open_base_file_for_write_triggers_cow() {
    let overlay = setup_with_base_files();
    // Opening for write should trigger CoW, not return ReadOnly error
    let file = overlay.open("/src/main.rs", OpenMode::Write).unwrap();
    let n = file.write_at(b"modified", 0).unwrap();
    assert_eq!(n, 8);
}

#[test]
#[traced_test]
fn test_concurrent_version_increment() {
    let overlay = setup();
    let v0 = overlay.version();
    overlay.write_file("/a.txt", b"a").unwrap();
    let v1 = overlay.version();
    overlay.write_file("/b.txt", b"b").unwrap();
    let v2 = overlay.version();
    assert!(v1 > v0);
    assert!(v2 > v1);
}

#[test]
#[traced_test]
fn test_root_directory_always_exists() {
    let overlay = setup();
    assert!(overlay.exists("/").unwrap());
    let dir = overlay.open_directory("/").unwrap();
    let entries = dir.list().unwrap();
    assert!(entries.is_empty());
}

#[test]
#[traced_test]
fn test_path_normalization() {
    let overlay = setup_with_base_files();
    // Trailing slash should be normalized
    assert!(overlay.exists("/src/").unwrap());
    // Double slash
    assert!(overlay.exists("//src").unwrap());
    // Read file with trailing slash normalization
    let data = overlay.read_file("/readme.md").unwrap();
    assert_eq!(data, b"# Project");
}

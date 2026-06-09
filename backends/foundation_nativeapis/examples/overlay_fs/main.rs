use foundation_nativeapis::shared::vfs::{
    DeltaStore, MemoryDelta, MemoryFs, OpenMode, VfsDirectory, VfsFile, VfsFileSystem, VfsFileType,
};
use foundation_nativeapis::shared::vfs::OverlayFileSystem;

fn main() {
    println!("=== OverlayFileSystem Example ===\n");

    // Base layer: read-only MemoryFs with some initial files
    let base = MemoryFs::new();
    base.mkdir("/src").unwrap();
    base.write_file("/src/main.rs", b"fn main() {}").unwrap();
    base.write_file("/src/lib.rs", b"pub mod utils;").unwrap();
    base.write_file("/README.md", b"# My Project").unwrap();
    println!("Base layer: 3 files in /src + /README.md");

    // Delta layer: writable MemoryDelta
    let delta = MemoryDelta::new();

    // Compose the overlay
    let overlay = OverlayFileSystem::new(base, delta);

    // Read from base layer (pass-through)
    let data = overlay.read_file("/src/main.rs").unwrap();
    println!("Read base file: {:?}", String::from_utf8_lossy(&data));

    // Write to overlay — copy-on-write
    overlay.write_file("/src/main.rs", b"fn main() { println!(\"Hello!\"); }").unwrap();
    let modified = overlay.read_file("/src/main.rs").unwrap();
    println!("After CoW write: {:?}", String::from_utf8_lossy(&modified));
    println!("Overlay version: {}", overlay.version());

    // Create a new file in the overlay
    overlay.mkdir("/tests").unwrap();
    overlay.write_file("/tests/test_main.rs", b"#[test] fn it_works() { assert!(true); }").unwrap();
    println!("Created /tests/test_main.rs in delta layer");

    // Whiteout: delete a base file
    overlay.remove("/src/lib.rs").unwrap();
    println!("Deleted /src/lib.rs (whiteout in delta)");
    assert!(!overlay.exists("/src/lib.rs").unwrap());

    // Base still has lib.rs — overlay hides it
    let base_has_it = overlay.base().exists("/src/lib.rs").unwrap();
    println!("Base still has lib.rs: {}", base_has_it);

    // List merged directory — base + delta entries minus whiteouts
    let dir = overlay.open_directory("/src").unwrap();
    let entries = dir.list().unwrap();
    println!("\nMerged /src listing:");
    for entry in &entries {
        println!("  {:?} {:?}", entry.file_type, entry.name);
    }

    // Delta inspection
    let delta = overlay.delta();
    println!("\nDelta has whiteout for /src/lib.rs: {:?}", delta.is_whiteout("/src/lib.rs").unwrap());

    // Flush + reset cycle
    delta.flush().unwrap();
    println!("Delta flushed");
    delta.reset().unwrap();
    println!("Delta reset — overlay is back to base state");

    // After reset, lib.rs is visible again
    assert!(overlay.exists("/src/lib.rs").unwrap());
    println!("/src/lib.rs visible again after delta reset");

    println!("\n=== Done ===");
}

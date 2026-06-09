/// Example: SQLite Delta Store — Persistent VFS Overlay.
///
/// Demonstrates how to persist VFS operations in a SQLite database
/// using `LibsqlDelta`. Files written through the overlay are stored
/// in SQLite and survive restarts.
///
/// ```bash
/// cargo run -p foundation_nativeapis --features "vfs-sqlite" --example vfs_sqlite_delta
/// ```

use std::path::PathBuf;

use foundation_nativeapis::shared::vfs::{
    VfsDirectory, VfsFileSystem,
    overlay_fs::OverlayFileSystem,
};

fn main() {
    println!("=== SQLite Delta Store Example ===\n");

    // Create a real temp directory as the base filesystem
    let base_dir = std::env::temp_dir().join("vfs-sqlite-base");
    std::fs::create_dir_all(&base_dir).ok();

    // Pre-populate the base with some files
    std::fs::write(base_dir.join("readme.txt"), "Base content\n").unwrap();
    std::fs::write(base_dir.join("config.ini"), "[app]\nport=8080\n").unwrap();

    let db_path = std::env::temp_dir().join("vfs-sqlite-example.db");
    let _ = std::fs::remove_file(&db_path);

    println!("Base dir:    {base_dir:?}");
    println!("SQLite DB:   {db_path:?}\n");

    // ── Phase 1: Create overlay with SQLite delta, write files ──
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ Phase 1: Write files (persisted to SQLite)              │");
    println!("└─────────────────────────────────────────────────────────┘\n");

    let base = foundation_nativeapis::native::vfs::NativeFs::new(&base_dir).unwrap();
    let delta = foundation_nativeapis::shared::vfs::libsql_delta::LibsqlDelta::new(&db_path).unwrap();
    let overlay = OverlayFileSystem::new(base, delta);

    overlay.write_file("/virtual/hello.txt", b"Hello from SQLite delta!\n").unwrap();
    overlay.write_file("/virtual/data.json", br#"{"key": "value"}"#).unwrap();
    overlay.mkdir("/virtual/subdir").unwrap();
    overlay.write_file("/virtual/subdir/nested.txt", b"Nested content\n").unwrap();

    println!("Written to SQLite delta:");
    println!("  /virtual/hello.txt        (25 bytes)");
    println!("  /virtual/data.json        (16 bytes)");
    println!("  /virtual/subdir/nested.txt (15 bytes)\n");

    // ── Phase 2: Read back ──
    println!("┌─────────────────────────────────────────────────────────┐");
    println!("│ Phase 2: Read files back                                │");
    println!("└─────────────────────────────────────────────────────────┘\n");

    let data = overlay.read_file("/virtual/hello.txt").unwrap();
    println!("  /virtual/hello.txt: {}", String::from_utf8_lossy(&data));

    let data = overlay.read_file("/virtual/data.json").unwrap();
    println!("  /virtual/data.json: {}", String::from_utf8_lossy(&data));

    let data = overlay.read_file("readme.txt").unwrap();
    println!("  readme.txt (base):  {}", String::from_utf8_lossy(&data));

    // ── Phase 3: Copy-on-Write ──
    println!("\n┌─────────────────────────────────────────────────────────┐");
    println!("│ Phase 3: Overlay semantics (copy-on-write)              │");
    println!("└─────────────────────────────────────────────────────────┘\n");

    // Modify a file that exists in the base — goes to delta (CoW)
    overlay.write_file("config.ini", b"[app]\nport=9090\nmodified=true\n").unwrap();
    println!("Modified config.ini in overlay (CoW)\n");

    let data = overlay.read_file("config.ini").unwrap();
    println!("  Overlay sees:  {}", String::from_utf8_lossy(&data).trim());

    // Base file on disk is unchanged
    let data = std::fs::read_to_string(base_dir.join("config.ini")).unwrap();
    println!("  Disk still:    {}", data.trim());

    // ── Phase 4: Directory listing ──
    println!("\n┌─────────────────────────────────────────────────────────┐");
    println!("│ Phase 4: Merged directory listing                       │");
    println!("└─────────────────────────────────────────────────────────┘\n");

    let dir = overlay.open_directory("/").unwrap();
    let entries = VfsDirectory::list(&dir).unwrap();
    println!("Root directory (merged base ∪ delta):");
    for e in &entries {
        println!("  {} ({:?})", e.name, e.file_type);
    }

    let dir = overlay.open_directory("/virtual").unwrap();
    let entries = VfsDirectory::list(&dir).unwrap();
    println!("\n/virtual directory:");
    for e in &entries {
        println!("  {} ({:?})", e.name, e.file_type);
    }

    // ── Phase 5: Persistence ──
    println!("\n┌─────────────────────────────────────────────────────────┐");
    println!("│ Phase 5: Persistence — SQLite DB on disk                │");
    println!("└─────────────────────────────────────────────────────────┘\n");

    if db_path.exists() {
        let size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
        let display = if size > 1024 {
            format!("{:.1} KB", size as f64 / 1024.0)
        } else {
            format!("{size} bytes")
        };
        println!("  {display}  {db_path:?}");
    }

    // ── Cleanup ──
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_dir_all(&base_dir);

    println!("\n=== How to use LibsqlDelta ===");
    println!();
    println!("  use foundation_nativeapis::shared::vfs::{{");
    println!("      libsql_delta::LibsqlDelta,");
    println!("      overlay_fs::OverlayFileSystem,");
    println!("      native_fs::NativeFs,");
    println!("      VfsFileSystem,");
    println!("  }};");
    println!();
    println!("  let base = NativeFs::new(\"/path/to/project\").unwrap();");
    println!("  let delta = LibsqlDelta::new(\"/path/to/vfs.db\").unwrap();");
    println!("  let overlay = OverlayFileSystem::new(base, delta);");
    println!();
    println!("  // All writes persist to SQLite");
    println!("  overlay.write_file(\"/config\", b\"...\").unwrap();");
}

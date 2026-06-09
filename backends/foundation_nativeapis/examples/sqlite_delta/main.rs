use foundation_core::valtron::initialize_pool;
use foundation_nativeapis::shared::vfs::{
    DeltaStore, LibsqlDelta, SyncLibsqlDelta, VfsFile, VfsFileSystem,
};

fn main() {
    println!("=== SQLite DeltaStore Example ===\n");

    let _guard = initialize_pool(42, Some(3));
    let db_path = std::env::temp_dir().join("foundation_sqlite_delta_example.db");

    // Clean up from previous runs
    let _ = std::fs::remove_file(&db_path);

    // Create a LibsqlDelta — async-first implementation
    let async_delta = LibsqlDelta::new(&db_path).unwrap();
    println!("Created LibsqlDelta at {:?}", db_path);

    // Wrap in SyncLibsqlDelta for sync API
    let delta = SyncLibsqlDelta::new(async_delta);

    // Create files — stored as chunked blobs in SQLite
    delta.mkdir("/config").unwrap();
    let file = delta.create("/config/app.toml", 0o644).unwrap();
    file.write_at(b"[server]\nport = 8080\nhost = \"0.0.0.0\"\n", 0).unwrap();
    println!("Created /config/app.toml ({} bytes)", file.size().unwrap());

    // Write a larger file
    let payload = "line\n".repeat(200);
    delta.write_file("/config/data.csv", payload.as_bytes()).unwrap();
    println!("Wrote /config/data.csv ({} bytes)", payload.len());

    // Read back
    let data = delta.read_file("/config/app.toml").unwrap();
    println!("Read /config/app.toml:\n{}", String::from_utf8_lossy(&data));

    // Stat
    let meta = delta.stat("/config/data.csv").unwrap();
    println!("stat data.csv: size={}, inode={}", meta.size, meta.inode);

    // DeltaStore whiteout operations
    delta.add_whiteout("/deleted_file.txt", 1).unwrap();
    println!("Added whiteout for /deleted_file.txt");

    let wo = delta.is_whiteout("/deleted_file.txt").unwrap();
    println!("is_whiteout: {:?}", wo);

    let whiteouts = delta.list_whiteouts("/").unwrap();
    println!("Whiteouts in /: {:?}", whiteouts);

    delta.remove_whiteout("/deleted_file.txt").unwrap();
    println!("Removed whiteout");

    // Flush persists to disk
    delta.flush().unwrap();
    println!("\nFlushed to disk");

    // Reset clears all delta state
    delta.reset().unwrap();
    println!("Reset — all data cleared");
    assert!(!delta.exists("/config/app.toml").unwrap());

    // Clean up
    let _ = std::fs::remove_file(&db_path);
    println!("\n=== Done ===");
}

use std::path::PathBuf;

use foundation_nativeapis::native::vfs::native_fs::NativeFs;
use foundation_nativeapis::shared::vfs::{
    OpenMode, VfsDirectory, VfsFile, VfsFileSystem, VfsFileType,
};

fn main() {
    println!("=== NativeFs Example ===\n");

    // Create a temp directory to use as the root
    let tmp = std::env::temp_dir().join("foundation_native_fs_example");
    std::fs::create_dir_all(&tmp).unwrap();

    let fs = NativeFs::new(&tmp).unwrap();
    println!("NativeFs root: {:?}", fs.root());

    // Create directories and files through the VFS API
    fs.mkdir("/src").unwrap();
    fs.mkdir("/src/models").unwrap();
    println!("Created /src and /src/models");

    let file = fs.create("/src/main.rs", 0o644).unwrap();
    file.write_at(b"fn main() {\n    println!(\"hello\");\n}\n", 0).unwrap();
    println!("Created /src/main.rs ({} bytes)", file.size().unwrap());

    // Read it back
    let data = fs.read_file("/src/main.rs").unwrap();
    println!("Contents:\n{}", String::from_utf8_lossy(&data));

    // Stat shows real OS inode
    let meta = fs.stat("/src/main.rs").unwrap();
    println!(
        "stat: inode={}, size={}, type={:?}, perms={:o}",
        meta.inode, meta.size, meta.file_type, meta.permissions
    );

    // Directory listing
    let dir = fs.open_directory("/src").unwrap();
    let entries = dir.list().unwrap();
    println!("/src contains:");
    for entry in &entries {
        println!("  {:?} {:?} (inode {})", entry.file_type, entry.name, entry.inode);
    }

    // Write another file and verify it's on disk
    fs.write_file("/src/models/user.rs", b"pub struct User { pub name: String }").unwrap();
    let disk_path: PathBuf = [tmp.to_str().unwrap(), "src", "models", "user.rs"].iter().collect();
    let on_disk = std::fs::read_to_string(&disk_path).unwrap();
    println!("\nVerify on disk at {:?}: {:?}", disk_path, on_disk);

    // Rename
    fs.rename("/src/models/user.rs", "/src/models/account.rs").unwrap();
    assert!(fs.exists("/src/models/account.rs").unwrap());
    assert!(!fs.exists("/src/models/user.rs").unwrap());
    println!("Renamed user.rs → account.rs");

    // Clean up
    fs.remove_all("/src").unwrap();
    println!("Removed /src tree");
    std::fs::remove_dir_all(&tmp).ok();

    println!("\n=== Done ===");
}

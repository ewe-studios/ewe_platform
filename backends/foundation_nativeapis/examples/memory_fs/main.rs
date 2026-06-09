use std::io::SeekFrom;

use foundation_nativeapis::shared::vfs::{
    MemoryFs, OpenMode, SeekableVfsFile, VfsDirectory, VfsFile, VfsFileSystem,
};

fn main() {
    println!("=== MemoryFs Example ===\n");

    let fs = MemoryFs::new();

    // Create directories
    fs.mkdir("/docs").unwrap();
    fs.mkdir("/docs/notes").unwrap();
    println!("Created /docs and /docs/notes");

    // Create and write a file
    let file = fs.create("/docs/hello.txt", 0o644).unwrap();
    file.write_at(b"Hello from MemoryFs!", 0).unwrap();
    println!("Created /docs/hello.txt ({} bytes)", file.size().unwrap());

    // Read it back
    let data = fs.read_file("/docs/hello.txt").unwrap();
    println!("Read: {:?}", String::from_utf8_lossy(&data));

    // Seekable file I/O
    let mut seekable = fs.open_seekable("/docs/hello.txt", OpenMode::ReadWrite).unwrap();
    seekable.seek(SeekFrom::Start(6)).unwrap();
    let mut buf = [0u8; 4];
    seekable.read(&mut buf).unwrap();
    println!("Read at offset 6: {:?}", String::from_utf8_lossy(&buf));

    // Stat
    let meta = fs.stat("/docs/hello.txt").unwrap();
    println!(
        "stat: inode={}, size={}, type={:?}, perms={:o}",
        meta.inode, meta.size, meta.file_type, meta.permissions
    );

    // Inode queries
    let ino = fs.inode("/docs/hello.txt").unwrap();
    let path = fs.path_by_inode(ino).unwrap();
    println!("inode {} → path {:?}", ino, path);

    // Directory listing
    let dir = fs.open_directory("/docs").unwrap();
    let entries = dir.list().unwrap();
    println!("\n/docs contains:");
    for entry in &entries {
        println!("  {:?} {:?} (inode {})", entry.file_type, entry.name, entry.inode);
    }

    // Rename
    fs.rename("/docs/hello.txt", "/docs/greeting.txt").unwrap();
    println!("\nRenamed hello.txt → greeting.txt");
    assert!(fs.exists("/docs/greeting.txt").unwrap());
    assert!(!fs.exists("/docs/hello.txt").unwrap());

    // Rename preserves inode
    let ino_after = fs.inode("/docs/greeting.txt").unwrap();
    println!("Inode preserved after rename: {} == {}", ino, ino_after);
    assert_eq!(ino, ino_after);

    // Write another file, then remove it
    fs.write_file("/docs/notes/tmp.txt", b"temporary").unwrap();
    fs.remove("/docs/notes/tmp.txt").unwrap();
    println!("Created and removed /docs/notes/tmp.txt");

    // Symlinks
    fs.symlink("/docs/greeting.txt", "/docs/link.txt").unwrap();
    let target = fs.readlink("/docs/link.txt").unwrap();
    println!("Symlink /docs/link.txt → {}", target);
    let link_data = fs.read_file("/docs/link.txt").unwrap();
    assert_eq!(link_data, fs.read_file("/docs/greeting.txt").unwrap());
    println!("Symlink read matches original");

    // remove_all on a directory tree — remove symlink first to avoid circular resolution
    fs.remove("/docs/link.txt").unwrap();
    fs.remove_all("/docs").unwrap();
    assert!(!fs.exists("/docs").unwrap());
    println!("\nremove_all /docs — gone");

    println!("\n=== Done ===");
}

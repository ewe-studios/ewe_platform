use foundation_nativeapis::shared::vfs::{MemoryFs, VfsFile, VfsFileSystem, VfsFileType};

fn main() {
    println!("=== Inode-Native VFS Example ===\n");

    let fs = MemoryFs::new();

    // Root directory always has inode 1
    let root_ino = fs.inode("/").unwrap();
    println!("Root inode: {}", root_ino);

    // Create files — each gets a unique inode
    fs.mkdir("/projects").unwrap();
    let file = fs.create("/projects/app.rs", 0o644).unwrap();
    file.write_at(b"fn main() {}", 0).unwrap();

    fs.create("/projects/lib.rs", 0o644).unwrap();

    let app_ino = fs.inode("/projects/app.rs").unwrap();
    let lib_ino = fs.inode("/projects/lib.rs").unwrap();
    let dir_ino = fs.inode("/projects").unwrap();
    println!("Inodes: /projects={}, app.rs={}, lib.rs={}", dir_ino, app_ino, lib_ino);
    assert_ne!(app_ino, lib_ino);

    // Reverse lookup: inode → path
    let path = fs.path_by_inode(app_ino).unwrap();
    println!("path_by_inode({}) = {:?}", app_ino, path);
    assert_eq!(path, "/projects/app.rs");

    // stat_by_inode — same result as stat by path
    let meta_by_path = fs.stat("/projects/app.rs").unwrap();
    let meta_by_ino = fs.stat_by_inode(app_ino).unwrap();
    assert_eq!(meta_by_path.size, meta_by_ino.size);
    assert_eq!(meta_by_path.file_type, meta_by_ino.file_type);
    println!(
        "stat_by_inode: size={}, type={:?}",
        meta_by_ino.size, meta_by_ino.file_type
    );

    // Rename preserves inode
    fs.rename("/projects/app.rs", "/projects/main.rs").unwrap();
    let ino_after = fs.inode("/projects/main.rs").unwrap();
    println!("\nRename /projects/app.rs → /projects/main.rs");
    println!("Inode before: {}, after: {} (preserved: {})", app_ino, ino_after, app_ino == ino_after);
    assert_eq!(app_ino, ino_after);

    // Reverse lookup updates too
    let new_path = fs.path_by_inode(app_ino).unwrap();
    println!("path_by_inode({}) now = {:?}", app_ino, new_path);
    assert_eq!(new_path, "/projects/main.rs");

    // Directory listing shows inodes
    let dir = fs.open_directory("/projects").unwrap();
    use foundation_nativeapis::shared::vfs::VfsDirectory;
    let entries = dir.list().unwrap();
    println!("\n/projects entries:");
    for e in &entries {
        println!("  {:?} {:?} inode={}", e.file_type, e.name, e.inode);
    }

    println!("\n=== Done ===");
}

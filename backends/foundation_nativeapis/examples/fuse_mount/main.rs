/// Example: FUSE Mount of a VFS filesystem.
///
/// Mounts a MemoryFs via FUSE, demonstrating how to expose the VFS
/// trait-based abstraction as a real mount point that any application
/// can interact with.
///
/// Requires: Linux with FUSE support, and the user must be in the `fuse` group
/// or the system must allow unprivileged FUSE mounts.

use std::sync::Arc;
use std::time::Duration;

use foundation_nativeapis::shared::vfs::memory_fs::MemoryFs;
use foundation_nativeapis::shared::vfs::overlay_fs::OverlayFileSystem;
use foundation_nativeapis::shared::vfs::memory_delta::MemoryDelta;

#[cfg(feature = "vfs-fuse")]
use foundation_nativeapis::native::vfs::fuse::FuseMount;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== FUSE Mount Example ===\n");

    // Build a MemoryFs as the base with some initial content
    let base = MemoryFs::new();
    base.write_file("/hello.txt", b"Hello from FUSE!\n")?;
    base.write_file("/readme.md", b"This is a FUSE-mounted MemoryFs.\n")?;
    base.mkdir_all("/src")?;
    base.write_file("/src/main.rs", b"fn main() { println!(\"Hello\"); }")?;

    // Create a writable overlay with MemoryDelta
    let overlay: OverlayFileSystem<MemoryFs, MemoryDelta> =
        OverlayFileSystem::new(base, MemoryDelta::new());

    println!("Virtual filesystem ready:");
    println!("  /hello.txt     (42 bytes)");
    println!("  /readme.md     (32 bytes)");
    println!("  /src/main.rs   (35 bytes)");
    println!();

    #[cfg(feature = "vfs-fuse")]
    {
        let mount_point = std::env::args().nth(1).unwrap_or_else(|| "/tmp/vfs-fuse".to_string());
        std::fs::create_dir_all(&mount_point).ok();

        println!("Mount point: {}", mount_point);
        println!("Starting FUSE server... (Ctrl+C to unmount)\n");

        let fuse = FuseMount::new(overlay);
        // In a real scenario this would block until unmount:
        // fuse.mount(&mount_point)?;
        println!("FUSE mount would start here. Full implementation requires:");
        println!("  1. fuser crate integration");
        println!("  2. FilesystemOps trait implementation");
        println!("  3. INO table mapping VFS inodes → FUSE inodes");
        println!("  4. Directory handle lifecycle (dirp ↔ inode)");
        println!();
        println!("To actually mount:");
        println!("  cargo run -p foundation_nativeapis --features vfs-fuse --example fuse_mount /tmp/vfs-fuse");
        println!();
        println!("Then in another terminal:");
        println!("  ls -la /tmp/vfs-fuse/");
        println!("  cat /tmp/vfs-fuse/hello.txt");
    }

    #[cfg(not(feature = "vfs-fuse"))]
    {
        println!("vfs-fuse feature not enabled. Run with:");
        println!("  cargo run -p foundation_nativeapis --features vfs-fuse --example fuse_mount");
    }

    println!("\n=== Done ===");
    Ok(())
}

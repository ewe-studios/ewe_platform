use std::sync::Arc;

use foundation_core::valtron::{initialize_pool, PoolGuard};
use foundation_nativeapis::shared::vfs::{
    AsyncVfsFile, AsyncVfsFileSystem, MemoryFs, SyncFs, VfsFile, VfsFileSystem, exec_async,
};

fn main() {
    println!("=== Sync/Async Bridge Example ===\n");

    // exec_async and SyncFs need valtron's thread pool
    let _guard: PoolGuard = initialize_pool(42, Some(3));

    // MemoryFs implements both sync and async VfsFileSystem traits.
    // SyncFs<A> wraps any AsyncVfsFileSystem and bridges it to the sync API
    // using valtron's exec_async (no tokio needed).

    let async_fs = Arc::new(MemoryFs::new());

    // Direct async usage via exec_async
    println!("--- Direct async via exec_async ---");
    let fs_clone = Arc::clone(&async_fs);
    exec_async(async move {
        fs_clone.mkdir_async("/async_dir".into()).await?;
        let file = fs_clone.create_async("/async_dir/data.bin".into(), 0o644).await?;
        file.write_at_async(b"async bytes".to_vec(), 0).await?;
        println!("Wrote via async API");
        Ok(())
    }).unwrap();

    // Read back via sync API to prove it's the same filesystem
    let data = async_fs.read_file("/async_dir/data.bin").unwrap();
    println!("Read via sync API: {:?}", String::from_utf8_lossy(&data));

    // SyncFs bridge: wrap the async impl for purely sync usage
    println!("\n--- SyncFs bridge ---");
    let sync_fs = SyncFs::new(MemoryFs::new());

    // All operations go through exec_async internally
    sync_fs.mkdir("/bridged").unwrap();
    let file = sync_fs.create("/bridged/report.txt", 0o644).unwrap();
    file.write_at(b"written through SyncFs bridge", 0).unwrap();

    let read_back = sync_fs.read_file("/bridged/report.txt").unwrap();
    println!("SyncFs read: {:?}", String::from_utf8_lossy(&read_back));

    let meta = sync_fs.stat("/bridged/report.txt").unwrap();
    println!("SyncFs stat: size={}, inode={}", meta.size, meta.inode);

    println!("\n=== Done ===");
}

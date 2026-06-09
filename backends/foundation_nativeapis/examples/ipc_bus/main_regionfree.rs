/// Demonstrates MemoryRegistry pooling with free callbacks.
///
/// Allocates a region with a free callback, drops it, allocates again
/// (reuses the cached entry), then lets the entry expire.
///
/// Run: cargo run -p foundation_nativeapis --features ipc --example ipc_region_free
#[cfg(all(target_os = "linux", feature = "ipc"))]
fn main() {
    use foundation_nativeapis::ipc::MemoryRegistry;

    let mut registry = MemoryRegistry::default();

    println!("Allocating with free callback...");
    let region = registry.alloc_with_free(64, None, || {
        println!("  [callback] Region freed from cache.");
    });
    println!("  Allocated: size={}", region.as_ref().map(|r| r.buffer_size()).unwrap_or(0));

    println!("Dropping region (returns to cache, ref_count=1)...");
    drop(region);

    println!("Re-allocating (should reuse cached entry)...");
    let _region = registry.alloc(64, None);
    println!("  Re-allocated successfully.");

    println!("Dropping and maintaining (triggers eviction after TTL)...");
    drop(_region);
    std::thread::sleep(std::time::Duration::from_secs(6));
    registry.maintain();
    println!("Done.");
}

#[cfg(not(all(target_os = "linux", feature = "ipc")))]
fn main() {
    eprintln!("This example requires Linux and the 'ipc' feature.");
}

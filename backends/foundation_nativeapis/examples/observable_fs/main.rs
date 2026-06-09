use foundation_nativeapis::shared::vfs::{
    MemoryFs, ObservableFs, VfsFile, VfsFileSystem,
};

fn main() {
    println!("=== ObservableFs Example ===\n");

    let inner = MemoryFs::new();
    let fs = ObservableFs::new(inner);

    // Subscribe to events before performing operations
    let rx = fs.subscribe();

    // Perform some operations
    fs.mkdir("/data").unwrap();
    println!("Created /data");

    let file = fs.create("/data/log.txt", 0o644).unwrap();
    file.write_at(b"first line\n", 0).unwrap();
    println!("Created and wrote to /data/log.txt");

    fs.write_file("/data/config.json", b"{\"key\": \"value\"}").unwrap();
    println!("Wrote /data/config.json");

    let _ = fs.stat("/data/log.txt").unwrap();
    println!("Stat'd /data/log.txt");

    fs.rename("/data/config.json", "/data/settings.json").unwrap();
    println!("Renamed config.json → settings.json");

    // Drain all events from the subscriber
    println!("\n--- Events received ---");
    let mut count = 0;
    while let Ok(event) = rx.recv() {
        println!("  {:?}", event);
        count += 1;
    }
    println!("\nTotal events: {}", count);

    println!("\n=== Done ===");
}

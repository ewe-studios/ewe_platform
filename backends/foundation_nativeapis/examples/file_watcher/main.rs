//! Watch a directory and print file changes using the native file watcher.
//!
//! Usage: `cargo run --example file_watcher --features watcher-linux -- path/to/watch`

use foundation_nativeapis::{native_watcher, WatchEventKind};
use std::time::Duration;

fn main() {
    let path = std::env::args().nth(1).expect("usage: file_watcher <path>");
    let mut watcher = native_watcher().expect("failed to create watcher");
    watcher
        .watch(path.as_ref(), true)
        .expect("failed to watch path");

    println!("Watching {} for changes...", path);
    println!("Events: Created, Modified, Removed, Renamed");
    println!("(Ctrl+C to exit)");

    loop {
        let events = watcher
            .poll(Duration::from_millis(100))
            .expect("poll failed");
        for event in events {
            match event.kind {
                WatchEventKind::Created => println!("  + {:?}", event.path),
                WatchEventKind::Modified => println!("  ~ {:?}", event.path),
                WatchEventKind::Removed => println!("  - {:?}", event.path),
                WatchEventKind::Renamed { from, to } => {
                    println!("  {} -> {}", from.display(), to.display())
                }
            }
        }
    }
}

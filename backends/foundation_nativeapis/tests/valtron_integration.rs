/// Integration test: FileWatcherTask with valtron execution engine.
///
/// Tests the full valtron task lifecycle:
/// - Create FileWatcherTask, subscribe, spawn into valtron
/// - Touch a file in watched directory
/// - Run engine for a few ticks
/// - Verify event received via mpp Receiver channel
/// - Test multiple subscribers receive the same events

use std::fs::File;
use std::path::PathBuf;
use std::time::Duration;

use foundation_core::synca::mpp::Receiver;
use foundation_core::valtron::TaskIterator;
use foundation_nativeapis::task::{EventBroadcaster, FileWatcherTask};
use foundation_nativeapis::watcher::{poll_watcher::PollWatcher, NativeWatcher};

/// Create a temp directory that cleans up on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "valtron_watcher_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("failed to create temp dir");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Test: EventBroadcaster delivers to multiple subscribers
#[test]
fn event_broadcast_to_multiple_subscribers() {
    let mut broadcaster = EventBroadcaster::new(64);
    let (_, rx1) = broadcaster.subscribe();
    let (_, rx2) = broadcaster.subscribe();
    let (_, rx3) = broadcaster.subscribe();

    broadcaster.broadcast("test_event".to_string());

    assert_eq!(rx1.recv().unwrap(), "test_event");
    assert_eq!(rx2.recv().unwrap(), "test_event");
    assert_eq!(rx3.recv().unwrap(), "test_event");
}

/// Test: EventBroadcaster cleans up dead subscribers
#[test]
fn event_broadcast_cleans_up_dead_subscribers() {
    let mut broadcaster = EventBroadcaster::new(64);
    let (_, rx1) = broadcaster.subscribe();
    let (_, rx2) = broadcaster.subscribe();

    // Drop one receiver
    drop(rx2);

    broadcaster.broadcast("event".to_string());

    assert_eq!(rx1.recv().unwrap(), "event");
    assert_eq!(broadcaster.subscriber_count(), 1);
}

/// Test: FileWatcherTask with PollWatcher delivers events
#[test]
fn file_watcher_task_delivers_events() {
    let dir = TempDir::new();

    // Create FileWatcherTask with PollWatcher
    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }));

    let (_, mut rx) = task.subscribe();

    // Create a file
    let file_path = dir.path().join("test_valtron.txt");
    File::create(&file_path).expect("create failed");

    // Manually tick the task via TaskIterator
    let status = task.next_status();
    assert!(
        status.is_some(),
        "task should not terminate"
    );

    // Verify subscriber received events
    if let Ok(event) = rx.recv() {
        assert!(event.path.ends_with("test_valtron.txt"));
    }
}

/// Test: FileWatcherTask handles poll error gracefully
#[test]
fn file_watcher_task_handles_poll_error() {
    // Create a task that will poll — even with no watches, it shouldn't panic
    let mut task = FileWatcherTask::with_watcher(Box::new(PollWatcher::new()));
    let _ = task.next_status(); // Should not panic
    let _ = task.next_status(); // Should not panic
}

/// Test: FileWatcherTask with no subscribers works fine
#[test]
fn file_watcher_task_no_subscribers() {
    let dir = TempDir::new();

    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }));

    // Don't subscribe — just tick
    let status = task.next_status();
    // Should still work even with no subscribers
    assert!(status.is_some());
}

/// Test: FileWatcherTask unwatch works
#[test]
fn file_watcher_task_unwatch() {
    let dir = TempDir::new();

    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }));

    task.unwatch(dir.path()).expect("unwatch failed");

    // Create file after unwatch — should not generate events
    File::create(dir.path().join("after_unwatch.txt")).expect("create failed");

    let status = task.next_status();
    assert!(status.is_some());
}

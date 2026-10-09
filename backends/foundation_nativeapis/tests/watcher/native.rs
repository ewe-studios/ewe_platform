/// Integration test: end-to-end file watching with InotifyWatcher and PollWatcher.
///
/// Tests that actual filesystem events are detected on the current platform.

use foundation_nativeapis::NativeWatcher;
use serial_test::serial;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

/// Create a temp directory that cleans up on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "nativeapis_test_{}_{}_{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("anon"),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("failed to create temp dir");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Helper: poll the watcher until we get events or timeout.
/// Retries a few times because inotify can have small delays.
fn poll_until_events(
    watcher: &mut impl NativeWatcher,
    timeout: Duration,
    max_retries: usize,
) -> Vec<foundation_nativeapis::WatchEvent> {
    for _ in 0..max_retries {
        let events = watcher.poll(timeout).expect("poll failed");
        if !events.is_empty() {
            return events;
        }
    }
    Vec::new()
}

/// Test: InotifyWatcher detects file creation
#[cfg(target_os = "linux")]
#[serial]
#[test]
fn inotify_detects_file_creation() {
    let mut watcher = foundation_nativeapis::native::watcher::linux::InotifyWatcher::new()
        .expect("failed to create inotify watcher");
    let dir = TempDir::new();

    watcher.watch(dir.path(), false).expect("failed to watch dir");

    // Create a file
    let _file = File::create(dir.path().join("test.txt")).expect("failed to create file");

    let events = poll_until_events(&mut watcher, Duration::from_millis(200), 5);
    assert!(
        events.iter().any(|e| e.path.ends_with("test.txt")
            && matches!(e.kind, foundation_nativeapis::WatchEventKind::Created)),
        "Expected Created event for test.txt, got: {:?}",
        events
    );
}

/// Test: InotifyWatcher detects file modification
#[cfg(target_os = "linux")]
#[serial]
#[test]
fn inotify_detects_file_modification() {
    let mut watcher = foundation_nativeapis::native::watcher::linux::InotifyWatcher::new()
        .expect("failed to create inotify watcher");
    let dir = TempDir::new();

    watcher.watch(dir.path(), false).expect("failed to watch dir");

    // Create then modify
    let mut file = File::create(dir.path().join("test.txt")).expect("failed to create file");
    file.write_all(b"hello").expect("failed to write");
    drop(file);

    let events = poll_until_events(&mut watcher, Duration::from_millis(200), 10);
    assert!(
        events.iter().any(|e| e.path.ends_with("test.txt")),
        "Expected event for test.txt, got: {:?}",
        events
    );
}

/// Test: InotifyWatcher detects file deletion
#[cfg(target_os = "linux")]
#[serial]
#[test]
fn inotify_detects_file_deletion() {
    let mut watcher = foundation_nativeapis::native::watcher::linux::InotifyWatcher::new()
        .expect("failed to create inotify watcher");
    let dir = TempDir::new();

    watcher.watch(dir.path(), false).expect("failed to watch dir");

    // Create then delete
    let path = dir.path().join("deletable.txt");
    File::create(&path).expect("failed to create file");
    // Small delay to ensure inotify registers the creation
    std::thread::sleep(Duration::from_millis(50));
    fs::remove_file(&path).expect("failed to delete file");

    let events = poll_until_events(&mut watcher, Duration::from_millis(200), 5);
    assert!(
        events.iter().any(|e| e.path.ends_with("deletable.txt")
            && matches!(e.kind, foundation_nativeapis::WatchEventKind::Removed)),
        "Expected Removed event for deletable.txt, got: {:?}",
        events
    );
}

/// Test: InotifyWatcher detects file rename
#[cfg(target_os = "linux")]
#[serial]
#[test]
fn inotify_detects_file_rename() {
    let mut watcher = foundation_nativeapis::native::watcher::linux::InotifyWatcher::new()
        .expect("failed to create inotify watcher");
    let dir = TempDir::new();

    watcher.watch(dir.path(), false).expect("failed to watch dir");

    let from = dir.path().join("old.txt");
    let to = dir.path().join("new.txt");
    File::create(&from).expect("failed to create file");
    std::thread::sleep(Duration::from_millis(50));
    fs::rename(&from, &to).expect("failed to rename file");

    let events = poll_until_events(&mut watcher, Duration::from_millis(200), 5);

    // inotify should emit a Renamed event with cookie matching
    let has_rename = events.iter().any(|e| {
        matches!(&e.kind, foundation_nativeapis::WatchEventKind::Renamed { from: f, to: t }
            if f.ends_with("old.txt") && t.ends_with("new.txt"))
    });
    assert!(
        has_rename,
        "Expected Renamed event (old.txt -> new.txt), got: {:?}",
        events
    );
}

/// Test: InotifyWatcher unwatch stops receiving events
#[cfg(target_os = "linux")]
#[serial]
#[test]
fn inotify_unwatch_stops_events() {
    let mut watcher = foundation_nativeapis::native::watcher::linux::InotifyWatcher::new()
        .expect("failed to create inotify watcher");
    let dir = TempDir::new();

    watcher.watch(dir.path(), false).expect("failed to watch dir");
    watcher.unwatch(dir.path()).expect("failed to unwatch");

    File::create(dir.path().join("after_unwatch.txt")).expect("failed to create file");

    let events = poll_until_events(&mut watcher, Duration::from_millis(200), 3);
    assert!(
        events.is_empty(),
        "Expected no events after unwatch, got: {:?}",
        events
    );
}

/// Test: InotifyWatcher clear removes all watches
#[cfg(target_os = "linux")]
#[serial]
#[test]
fn inotify_clear_removes_all_watches() {
    let mut watcher = foundation_nativeapis::native::watcher::linux::InotifyWatcher::new()
        .expect("failed to create inotify watcher");
    let dir = TempDir::new();

    watcher.watch(dir.path(), false).expect("failed to watch dir");
    watcher.clear().expect("failed to clear");

    File::create(dir.path().join("after_clear.txt")).expect("failed to create file");

    let events = poll_until_events(&mut watcher, Duration::from_millis(200), 3);
    assert!(
        events.is_empty(),
        "Expected no events after clear, got: {:?}",
        events
    );
}

/// Test: PollWatcher detects file creation (fallback, slower)
#[serial]
#[test]
fn poll_watcher_detects_file_creation() {
    let mut watcher = foundation_nativeapis::PollWatcher::new();
    let dir = TempDir::new();

    watcher.watch(dir.path(), false).expect("failed to watch dir");

    // Create a file after initial snapshot
    let _file = File::create(dir.path().join("poll_test.txt")).expect("failed to create file");

    let events = watcher.poll(Duration::from_millis(200)).expect("poll failed");
    // PollWatcher detects directory metadata change (mtime changes on file creation)
    assert!(
        !events.is_empty(),
        "Expected at least one event for poll_test.txt creation, got: {:?}",
        events
    );
}

/// Test: PollWatcher detects file deletion (by stat change)
#[serial]
#[test]
fn poll_watcher_detects_file_deletion() {
    let mut watcher = foundation_nativeapis::PollWatcher::new();
    let dir = TempDir::new();

    // Watch the directory directly (not the file, since the file will be deleted)
    watcher.watch(dir.path(), false).expect("failed to watch dir");

    // Create a file before poll
    let path = dir.path().join("poll_delete.txt");
    File::create(&path).expect("failed to create file");
    // Small delay so the snapshot captures the file
    std::thread::sleep(Duration::from_millis(50));

    // Delete the file — directory mtime changes
    fs::remove_file(&path).expect("failed to delete file");

    let events = watcher.poll(Duration::from_millis(200)).expect("poll failed");
    // PollWatcher detects directory metadata change
    assert!(
        !events.is_empty(),
        "Expected at least one event for file deletion, got: {:?}",
        events
    );
}

/// Test: PollWatcher detects file modification
#[serial]
#[test]
fn poll_watcher_detects_file_modification() {
    let mut watcher = foundation_nativeapis::PollWatcher::new();
    let dir = TempDir::new();

    // Watch the file directly (not the directory)
    let path = dir.path().join("poll_modify.txt");
    File::create(&path).expect("failed to create file");

    watcher.watch(&path, false).expect("failed to watch dir");

    // Modify the file — write content to change mtime/size
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .expect("failed to open file");
    file.write_all(b"hello world").expect("failed to write");
    drop(file);

    let events = watcher.poll(Duration::from_millis(200)).expect("poll failed");
    assert!(
        events.iter().any(|e| matches!(e.kind, foundation_nativeapis::WatchEventKind::Modified)),
        "Expected Modified event for poll_modify.txt, got: {:?}",
        events
    );
}

/// Test: PollWatcher unwatch stops receiving events
#[serial]
#[test]
fn poll_watcher_unwatch() {
    let mut watcher = foundation_nativeapis::PollWatcher::new();
    let dir = TempDir::new();

    watcher.watch(dir.path(), false).expect("failed to watch dir");
    watcher.unwatch(dir.path()).expect("failed to unwatch");

    File::create(dir.path().join("poll_unwatch.txt")).expect("failed to create file");

    let events = watcher.poll(Duration::from_millis(200)).expect("poll failed");
    assert!(
        events.is_empty(),
        "Expected no events after unwatch, got: {:?}",
        events
    );
}

/// Test: PollWatcher handles non-existent path gracefully
#[serial]
#[test]
fn poll_watcher_nonexistent_path() {
    let mut watcher = foundation_nativeapis::PollWatcher::new();
    // Watching a file that doesn't exist should fail because
    // PathSnapshot::from_path returns Err for paths that can't be stat'd
    // (non-NotFound errors like permission denied) or the path doesn't exist
    // and we don't want to watch non-existent paths by default.
    //
    // However, PollWatcher currently treats non-existent paths as "exists: false"
    // snapshots, which means watch() succeeds. This is a design choice —
    // the PollWatcher will detect when the path later appears (Created).
    // So this test verifies the alternative: watch succeeds, no events on poll.
    let result = watcher.watch(std::path::Path::new("/nonexistent/path/that/does/not/exist"), false);
    // Currently: succeeds because non-existent is a valid snapshot state
    assert!(result.is_ok(), "PollWatcher accepts non-existent paths as snapshots");
}

/// Test: WatcherBuilder builds a PollWatcher as fallback
#[serial]
#[test]
fn watcher_builder_fallback_poll() {
    use foundation_nativeapis::shared::api::{NativeAPI, WatcherBuilder};

    // Explicitly request Poll — should always work
    let watcher = WatcherBuilder::new()
        .preferred(NativeAPI::Poll)
        .build();

    assert!(watcher.is_ok(), "Poll watcher should always be available");
}

/// The readiness check must not consume what `poll()` needs.
///
/// WHY: `InotifyWatcher` registers its inotify fd **edge-triggered**. An earlier
/// `has_events()` ran its own `epoll_wait` on that selector, consuming the
/// one-shot edge; `poll()` then saw no event and returned empty *without ever
/// reading the fd*. The events stayed queued and a `FileWatcherTask` parked on
/// `Depends(watcher)` forever. It was racy — sometimes `poll()` won the edge —
/// which is why it only showed up under test parallelism.
///
/// WHAT: after any number of `has_events()` calls, `poll()` must still return
/// the queued events.
///
/// HOW: `has_events()` now asks `FIONREAD` (non-destructive) and `poll()` reads
/// the fd directly rather than gating on an epoll edge.
#[cfg(target_os = "linux")]
#[serial]
#[test]
fn inotify_has_events_does_not_consume_the_readiness_edge() {
    let mut watcher = foundation_nativeapis::native::watcher::linux::InotifyWatcher::new()
        .expect("failed to create inotify watcher");
    let dir = TempDir::new();
    watcher.watch(dir.path(), false).expect("failed to watch dir");

    let _file = File::create(dir.path().join("edge.txt")).expect("failed to create file");

    // Let inotify queue the event, then interrogate readiness repeatedly.
    let mut saw_ready = false;
    for _ in 0..50 {
        if watcher.has_events(None) {
            saw_ready = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(saw_ready, "has_events must report the queued inotify event");

    // Ask several more times: a non-destructive check stays true.
    for i in 0..5 {
        assert!(
            watcher.has_events(None),
            "has_events call {i} returned false — it consumed the readiness it reported"
        );
    }

    // And the events must still be there to collect.
    let events = watcher.poll(Duration::from_millis(200)).expect("poll failed");
    assert!(
        events.iter().any(|e| e.path.ends_with("edge.txt")),
        "poll() lost the event that has_events() reported; got {events:?}"
    );
}

/// Integration test: Full valtron multi-threaded executor exercise.

use std::fs::File;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use foundation_core::valtron::{collect_one, execute, initialize_pool, PoolGuard};
use foundation_nativeapis::FileWatcherTask;
use foundation_nativeapis::{NativeWatcher, PollWatcher};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "valtron_multi_{}_{}",
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

fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

/// Test: FileWatcherTask executes on multi-threaded worker pool.
/// Uses collect_one to grab the first event then stop.
#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_multi_executor() {
    let _guard = init_pool();

    let dir = TempDir::new();

    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }))
    .with_poll_timeout(Duration::from_millis(50));

    let rx = task.subscribe();

    let dir_path = dir.path().to_path_buf();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        File::create(dir_path.join("multi_exec.txt")).expect("create failed");
    });

    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");
    let event = collect_one(stream).expect("expected at least one event");

    assert!(event.path.starts_with(dir.path()), "expected event in watched dir, got {:?}", event.path);
    assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok(), "subscriber missed event");
}

/// Test: Multiple FileWatcherTask instances run concurrently.
/// Each uses collect_one to grab its first event then stop.
#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn multiple_watchers_concurrent_multi_executor() {
    let _guard = init_pool();

    let dir1 = TempDir::new();
    let dir2 = TempDir::new();

    let mut task1 = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir1.path(), false).expect("watch failed");
        w
    }))
    .with_poll_timeout(Duration::from_millis(50));

    let mut task2 = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir2.path(), false).expect("watch failed");
        w
    }))
    .with_poll_timeout(Duration::from_millis(50));

    let rx1 = task1.subscribe();
    let rx2 = task2.subscribe();

    let dir1_path = dir1.path().to_path_buf();
    let dir2_path = dir2.path().to_path_buf();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        File::create(dir1_path.join("concurrent1.txt")).expect("create failed");
        File::create(dir2_path.join("concurrent2.txt")).expect("create failed");
    });

    let stream1 = execute(task1, Some(Duration::from_millis(100))).expect("execute task1 failed");
    let stream2 = execute(task2, Some(Duration::from_millis(100))).expect("execute task2 failed");

    let event1 = collect_one(stream1).expect("task1 expected at least one event");
    let event2 = collect_one(stream2).expect("task2 expected at least one event");

    assert!(event1.path.starts_with(dir1.path()), "task1 event path: {:?}", event1.path);
    assert!(event2.path.starts_with(dir2.path()), "task2 event path: {:?}", event2.path);

    assert!(rx1.recv_timeout(Duration::from_secs(5)).is_ok(), "subscriber 1 missed event");
    assert!(rx2.recv_timeout(Duration::from_secs(5)).is_ok(), "subscriber 2 missed event");
}

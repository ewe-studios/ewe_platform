/// Integration test: Full valtron multi-threaded executor exercise.
///
/// Tests that FileWatcherTask runs correctly on the multi-worker pool
/// (multi=on feature flag), producing Stream::Next values that are collected
/// at the sync boundary via collect_result().
use std::fs::File;
use std::path::PathBuf;
use std::time::Duration;

use foundation_core::valtron::{collect_result, execute, initialize_pool, PoolGuard};
use foundation_nativeapis::task::FileWatcherTask;
use foundation_nativeapis::watcher::{poll_watcher::PollWatcher, NativeWatcher};

/// Create a temp directory that cleans up on drop.
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

/// Initialize the valtron thread pool.
fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

/// Test: FileWatcherTask executes on multi-threaded worker pool,
/// produces Stream::Next events that are collected at sync boundary.
#[test]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_multi_executor() {
    let _guard = init_pool();

    let dir = TempDir::new();

    // Create a file BEFORE executing (so poll detects it on first tick)
    File::create(dir.path().join("multi_exec.txt")).expect("create failed");

    // Create FileWatcherTask with PollWatcher
    let task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }));

    // Execute through valtron multi-threaded executor
    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");

    // Collect results — the executor runs the task on a worker thread,
    // and collect_result drains the stream at the sync boundary
    let results: Vec<_> = collect_result(stream);

    assert!(
        !results.is_empty(),
        "expected at least one event from multi-threaded valtron execution, got none"
    );
}

/// Test: Multiple FileWatcherTask instances run concurrently on the worker pool,
/// each producing events independently, and all events are collected correctly.
#[test]
#[serial_test::serial]
#[tracing_test::traced_test]
fn multiple_watchers_concurrent_multi_executor() {
    let _guard = init_pool();

    let dir1 = TempDir::new();
    let dir2 = TempDir::new();

    // Create files BEFORE executing
    File::create(dir1.path().join("concurrent1.txt")).expect("create failed");
    File::create(dir2.path().join("concurrent2.txt")).expect("create failed");

    // Create FileWatcherTask for dir1
    let task1 = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir1.path(), false).expect("watch failed");
        w
    }));

    // Create FileWatcherTask for dir2
    let task2 = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir2.path(), false).expect("watch failed");
        w
    }));

    // Execute both tasks concurrently on the worker pool
    let stream1 = execute(task1, Some(Duration::from_millis(100))).expect("execute task1 failed");
    let stream2 = execute(task2, Some(Duration::from_millis(100))).expect("execute task2 failed");

    // Collect results from both streams
    let results1: Vec<_> = collect_result(stream1);
    let results2: Vec<_> = collect_result(stream2);

    // Both tasks should have produced at least one event
    assert!(
        !results1.is_empty(),
        "task1 expected at least one event from multi-threaded execution"
    );
    assert!(
        !results2.is_empty(),
        "task2 expected at least one event from multi-threaded execution"
    );
}

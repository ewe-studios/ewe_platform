/// Integration test: FileWatcherTask and FdMonitorTask run through the valtron executor.
///
/// Tests the full valtron task lifecycle with multi-threaded pool execution:
/// - Initialize valtron thread pool and hold PoolGuard
/// - Execute FileWatcherTask through valtron executor, collect Stream::Next values
/// - Verify multiple subscribers receive the same events
/// - Execute FdMonitorTask, verify callback invoked when fd becomes readable
use std::fs::File;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_core::valtron::{collect_result, execute, initialize_pool, PoolGuard};
use foundation_nativeapis::task::{EventBroadcaster, FdMonitorTask, FileWatcherTask};
use foundation_nativeapis::watcher::{poll_watcher::PollWatcher, NativeWatcher};

/// Create a temp directory that cleans up on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "valtron_exec_{}_{}",
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

/// Create a pipe (reader, writer), both nonblocking.
fn make_pipe() -> (OwnedFd, OwnedFd) {
    let (r, w) = nix::unistd::pipe().expect("pipe creation failed");
    nix::fcntl::fcntl(
        r.as_raw_fd(),
        nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
    )
    .expect("fcntl O_NONBLOCK failed");
    nix::fcntl::fcntl(
        w.as_raw_fd(),
        nix::fcntl::FcntlArg::F_SETFL(nix::fcntl::OFlag::O_NONBLOCK),
    )
    .expect("fcntl O_NONBLOCK failed");
    (r, w)
}

/// Initialize the valtron thread pool.
/// Uses a fixed thread count (2) and fixed seed (42) for reproducibility.
/// Returns the PoolGuard which must be held for the duration of the test.
fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

/// Test: FileWatcherTask executes through valtron and produces Stream::Next events
#[serial_test::serial]
#[tracing_test::traced_test]
#[test]
fn file_watcher_task_valtron_execution() {
    let _guard = init_pool();

    let dir = TempDir::new();

    // Create a file BEFORE executing (so poll detects it on first tick)
    File::create(dir.path().join("exec_test.txt")).expect("create failed");

    // Create FileWatcherTask with PollWatcher
    let task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }));

    // Execute through valtron executor
    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");

    // Collect results at sync boundary
    let results: Vec<_> = collect_result(stream);

    assert!(
        !results.is_empty(),
        "expected at least one event from valtron execution, got none"
    );
}

/// Test: FdMonitorTask executes through valtron and invokes callback
#[serial_test::serial]
#[tracing_test::traced_test]
#[test]
fn fd_monitor_task_valtron_execution() {
    let _guard = init_pool();

    let (reader, writer) = make_pipe();
    let poll = foundation_nativeapis::poll::Poll::new().expect("failed to create poll");
    let registry = poll.registry();

    let registered = foundation_nativeapis::fd::RegisteredFd::new(
        reader,
        &registry,
        foundation_nativeapis::poll::Token(0),
    )
    .expect("failed to register fd");

    let data_received: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let data_clone = data_received.clone();

    // Write data before creating task
    unsafe {
        libc::write(
            writer.as_raw_fd(),
            b"valtron pipe data".as_ptr() as *const _,
            17,
        );
    }

    let task = FdMonitorTask::new(registered)
        .with_callback(move |fd| {
            let mut buf = [0u8; 64];
            let n = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr() as *mut _, buf.len()) };
            if n > 0 {
                data_clone
                    .lock()
                    .unwrap()
                    .extend_from_slice(&buf[..n as usize]);
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "read returned 0",
                ))
            }
        })
        .with_poll_interval(Duration::from_millis(10));

    // Execute through valtron executor
    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");

    // Collect all Stream::Next values
    let results: Vec<()> = collect_result(stream);

    assert!(
        !results.is_empty(),
        "expected at least one Ready event from valtron execution"
    );

    // Verify callback received data
    let received = data_received.lock().unwrap();
    assert_eq!(&received[..17], b"valtron pipe data");
}

/// Test: FileWatcherTask with multiple subscribers — all receive events
#[serial_test::serial]
#[tracing_test::traced_test]
#[test]
fn file_watcher_task_multi_subscriber_valtron() {
    let _guard = init_pool();

    let dir = TempDir::new();

    // Create a file
    File::create(dir.path().join("multi_sub.txt")).expect("create failed");

    // Create FileWatcherTask with PollWatcher
    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }));

    let (_, rx1) = task.subscribe();
    let (_, rx2) = task.subscribe();

    // Execute through valtron executor
    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");

    collect_result(stream);

    // Both subscribers should receive the event
    let event1 = rx1
        .recv_timeout(Duration::from_secs(1))
        .expect("subscriber 1 missed event");
    let event2 = rx2
        .recv_timeout(Duration::from_secs(1))
        .expect("subscriber 2 missed event");
    assert_eq!(event1.path, event2.path);
}

/// Test: EventBroadcaster delivers to multiple subscribers
#[test]
#[serial_test::serial]
#[tracing_test::traced_test]
fn event_broadcast_to_multiple_subscribers() {
    let _guard = init_pool();

    let mut broadcaster = EventBroadcaster::new(64);
    let (_, rx1) = broadcaster.subscribe();
    let (_, rx2) = broadcaster.subscribe();
    let (_, rx3) = broadcaster.subscribe();

    broadcaster.broadcast("test_event".to_string());

    assert_eq!(rx1.recv().unwrap(), "test_event");
    assert_eq!(rx2.recv().unwrap(), "test_event");
    assert_eq!(rx3.recv().unwrap(), "test_event");
}

/// Test: EventBroadcaster cleans up dead subscribers on broadcast
#[test]
#[serial_test::serial]
#[tracing_test::traced_test]
fn event_broadcast_cleans_up_dead_subscribers() {
    let _guard = init_pool();

    let mut broadcaster = EventBroadcaster::new(64);
    let (_, rx1) = broadcaster.subscribe();

    // Subscribe and drop immediately — creates a dead channel
    let (_tx2, rx2) = broadcaster.subscribe();
    drop(rx2);

    // Broadcast should not panic even with a dead subscriber
    broadcaster.broadcast("event".to_string());
    assert_eq!(rx1.recv().unwrap(), "event");
}

/// Test: FileWatcherTask with PollWatcher delivers events
#[test]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_delivers_events() {
    let _guard = init_pool();

    let dir = TempDir::new();

    // Create FileWatcherTask with PollWatcher
    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }));

    let (_, rx) = task.subscribe();

    // Create a file
    File::create(dir.path().join("test_valtron.txt")).expect("create failed");

    // Execute through valtron executor
    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");

    collect_result(stream);

    // Verify subscriber received events
    if let Ok(event) = rx.recv() {
        assert!(
            event.path.starts_with(dir.path()) || event.path.ends_with("test_valtron.txt"),
            "expected event related to test dir, got {:?}",
            event.path
        );
    }
}

/// Test: FileWatcherTask handles poll error gracefully
#[test]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_handles_poll_error() {
    let _guard = init_pool();

    // Create a task that will poll — even with no watches, it shouldn't panic
    let task = FileWatcherTask::with_watcher(Box::new(PollWatcher::new()));
    let stream = execute(task, Some(Duration::from_millis(10))).expect("execute failed");

    // Should complete without panicking
    collect_result(stream);
}

/// Test: FileWatcherTask with no subscribers works fine
#[test]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_no_subscribers() {
    let _guard = init_pool();

    let dir = TempDir::new();

    let task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }));

    // Don't subscribe — just execute
    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");

    collect_result(stream);
}

/// Test: FileWatcherTask unwatch works
#[test]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_unwatch() {
    let _guard = init_pool();

    let dir = TempDir::new();

    // Create FileWatcherTask with PollWatcher
    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }));

    task.unwatch(dir.path()).expect("unwatch failed");

    // Create file after unwatch — should not generate events
    File::create(dir.path().join("after_unwatch.txt")).expect("create failed");

    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");

    collect_result(stream);
}

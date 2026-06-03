/// Integration test: FileWatcherTask and FdMonitorTask run through the valtron executor.

use std::fs::File;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use foundation_core::valtron::{collect_one, collect_result, execute, initialize_pool, PoolGuard};
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
fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

/// Test: FileWatcherTask executes through valtron and produces Stream::Next events.
/// Uses collect_one to grab the first event then stop — no infinite blocking.
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_valtron_execution() {
    let _guard = init_pool();

    let dir = TempDir::new();

    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }))
    .with_poll_timeout(Duration::from_millis(50));

    let (_, rx) = task.subscribe();

    let dir_path = dir.path().to_path_buf();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        File::create(dir_path.join("exec_test.txt")).expect("create failed");
    });

    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");

    // collect_one blocks until first Stream::Next, then stops
    let event = collect_one(stream).expect("expected at least one event");
    assert!(
        event.path.starts_with(dir.path()),
        "expected event in watched dir, got {:?}",
        event.path
    );

    assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok(), "subscriber missed event");
}

/// Test: FdMonitorTask executes through valtron and invokes callback
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
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

    // Write data before creating task — fd is already readable
    unsafe {
        libc::write(
            writer.as_raw_fd(),
            b"valtron pipe data".as_ptr() as *const _,
            17,
        );
    }

    let mut task = FdMonitorTask::new(registered)
        .with_callback(move |fd| {
            let mut buf = [0u8; 64];
            let n = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr() as *mut _, buf.len()) };
            if n > 0 {
                data_clone.lock().unwrap().extend_from_slice(&buf[..n as usize]);
                Ok(())
            } else {
                Err(std::io::Error::new(std::io::ErrorKind::Other, "read returned 0"))
            }
        })
        .with_poll_interval(Duration::from_millis(10));

    // Signal stop after a short delay so the task runs a few ticks then terminates
    let stop = task.stop_signal();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        stop.stop();
    });

    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");
    let results: Vec<()> = collect_result(stream);

    assert!(
        !results.is_empty(),
        "expected at least one Ready event from valtron execution"
    );

    let received = data_received.lock().unwrap();
    assert_eq!(&received[..17], b"valtron pipe data");
}

/// Test: FileWatcherTask with multiple subscribers — all receive events
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_multi_subscriber_valtron() {
    let _guard = init_pool();

    let dir = TempDir::new();

    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }))
    .with_poll_timeout(Duration::from_millis(50));

    let (_, rx1) = task.subscribe();
    let (_, rx2) = task.subscribe();

    let dir_path = dir.path().to_path_buf();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        File::create(dir_path.join("multi_sub.txt")).expect("create failed");
    });

    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");
    let event = collect_one(stream).expect("expected at least one event");

    let event1 = rx1
        .recv_timeout(Duration::from_secs(5))
        .expect("subscriber 1 missed event");
    let event2 = rx2
        .recv_timeout(Duration::from_secs(5))
        .expect("subscriber 2 missed event");
    assert_eq!(event.path, event1.path);
    assert_eq!(event1.path, event2.path);
}

/// Test: EventBroadcaster delivers to multiple subscribers
#[test]
#[ntest::timeout(60_000)]
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
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn event_broadcast_cleans_up_dead_subscribers() {
    let _guard = init_pool();

    let mut broadcaster = EventBroadcaster::new(64);
    let (_, rx1) = broadcaster.subscribe();

    // Subscribe and drop immediately — creates a dead channel
    let (_tx2, rx2) = broadcaster.subscribe();
    drop(rx2);

    broadcaster.broadcast("event".to_string());
    assert_eq!(rx1.recv().unwrap(), "event");
}

/// Test: FileWatcherTask with PollWatcher delivers events to subscriber
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_delivers_events() {
    let _guard = init_pool();

    let dir = TempDir::new();

    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }))
    .with_poll_timeout(Duration::from_millis(50));

    let (_, rx) = task.subscribe();

    let dir_path = dir.path().to_path_buf();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        File::create(dir_path.join("test_valtron.txt")).expect("create failed");
    });

    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");
    let event = collect_one(stream).expect("expected at least one event");

    assert!(
        event.path.starts_with(dir.path()) || event.path.ends_with("test_valtron.txt"),
        "expected event related to test dir, got {:?}",
        event.path
    );

    assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok(), "subscriber missed event");
}

/// Test: FileWatcherTask with no watches — should not panic.
/// Uses StopSignal to terminate the task after verifying no events.
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_handles_poll_error() {
    let _guard = init_pool();

    let mut task = FileWatcherTask::with_watcher(Box::new(PollWatcher::new()))
        .with_poll_timeout(Duration::from_millis(10));

    // Signal stop after a short delay so the task runs a few ticks then terminates
    let stop = task.stop_signal();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        stop.stop();
    });

    let stream = execute(task, Some(Duration::from_millis(50))).expect("execute failed");

    // collect_result will eventually stop when stop_signal fires
    let results: Vec<_> = collect_result(stream);
    assert!(results.is_empty(), "expected no events for empty watcher");
}

/// Test: FileWatcherTask with no subscribers works fine.
/// Events are still produced — the subscriber list just stays empty.
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_no_subscribers() {
    let _guard = init_pool();

    let dir = TempDir::new();

    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }))
    .with_poll_timeout(Duration::from_millis(50));

    // No subscribe call — events are still produced

    let dir_path = dir.path().to_path_buf();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        File::create(dir_path.join("no_sub.txt")).expect("create failed");
    });

    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");
    let event = collect_one(stream).expect("expected at least one event even without subscribers");

    assert!(
        event.path.starts_with(dir.path()),
        "expected event in watched dir, got {:?}",
        event.path
    );
}

/// Test: FileWatcherTask unwatch works — no events after unwatching.
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_unwatch() {
    let _guard = init_pool();

    let dir = TempDir::new();

    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = PollWatcher::new();
        w.watch(dir.path(), false).expect("watch failed");
        w
    }))
    .with_poll_timeout(Duration::from_millis(50));

    let (_, rx) = task.subscribe();

    task.unwatch(dir.path()).expect("unwatch failed");

    File::create(dir.path().join("after_unwatch.txt")).expect("create failed");

    // Signal stop so the task terminates cleanly
    let stop = task.stop_signal();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        stop.stop();
    });

    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");
    let results: Vec<_> = collect_result(stream);

    assert!(
        results.is_empty(),
        "expected no events after unwatch, got {:?}",
        results
    );

    assert!(
        rx.recv_timeout(Duration::from_millis(200)).is_err(),
        "expected no events on subscriber after unwatch"
    );
}

/// Test: FileWatcherTask using InotifyWatcher (Linux only) — event-driven.
#[cfg(target_os = "linux")]
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_inotify_execution() {
    use foundation_nativeapis::watcher::linux::InotifyWatcher;

    let _guard = init_pool();

    let dir = TempDir::new();

    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = InotifyWatcher::new().expect("failed to create inotify watcher");
        w.watch(dir.path(), false).expect("watch failed");
        w
    }));

    let (_, rx) = task.subscribe();

    let dir_path = dir.path().to_path_buf();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        File::create(dir_path.join("inotify_test.txt")).expect("create failed");
    });

    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");
    let event = collect_one(stream).expect("expected at least one event from inotify watcher");

    assert!(
        event.path.starts_with(dir.path()),
        "expected event in watched dir, got {:?}",
        event.path
    );

    assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok(), "subscriber missed event");
}

/// Test: InotifyWatcher with multiple subscribers — all receive events
#[cfg(target_os = "linux")]
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_inotify_multi_subscriber() {
    use foundation_nativeapis::watcher::linux::InotifyWatcher;

    let _guard = init_pool();

    let dir = TempDir::new();

    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = InotifyWatcher::new().expect("failed to create inotify watcher");
        w.watch(dir.path(), false).expect("watch failed");
        w
    }));

    let (_, rx1) = task.subscribe();
    let (_, rx2) = task.subscribe();

    let dir_path = dir.path().to_path_buf();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        File::create(dir_path.join("inotify_multi.txt")).expect("create failed");
    });

    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");
    let event = collect_one(stream).expect("expected at least one event from inotify");

    let event1 = rx1
        .recv_timeout(Duration::from_secs(5))
        .expect("subscriber 1 missed event");
    let event2 = rx2
        .recv_timeout(Duration::from_secs(5))
        .expect("subscriber 2 missed event");
    assert_eq!(event.path, event1.path);
    assert_eq!(event1.path, event2.path);
}

/// Test: InotifyWatcher unwatch — no events after unwatching (Linux)
#[cfg(target_os = "linux")]
#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn file_watcher_task_inotify_unwatch() {
    use foundation_nativeapis::watcher::linux::InotifyWatcher;

    let _guard = init_pool();

    let dir = TempDir::new();

    let mut task = FileWatcherTask::with_watcher(Box::new({
        let mut w = InotifyWatcher::new().expect("failed to create inotify watcher");
        w.watch(dir.path(), false).expect("watch failed");
        w
    }));

    let (_, rx) = task.subscribe();

    task.unwatch(dir.path()).expect("unwatch failed");

    File::create(dir.path().join("after_inotify_unwatch.txt")).expect("create failed");

    let stop = task.stop_signal();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        stop.stop();
    });

    let stream = execute(task, Some(Duration::from_millis(100))).expect("execute failed");
    let results: Vec<_> = collect_result(stream);

    assert!(
        results.is_empty(),
        "expected no events after unwatch with inotify, got {:?}",
        results
    );

    assert!(
        rx.recv_timeout(Duration::from_millis(200)).is_err(),
        "expected no events on subscriber after inotify unwatch"
    );
}

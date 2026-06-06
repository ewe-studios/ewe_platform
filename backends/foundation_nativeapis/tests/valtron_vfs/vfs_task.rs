//! Integration tests: VfsTask through valtron executor.

#![cfg(feature = "vfs")]

use foundation_core::valtron::{collect_one, initialize_pool, PoolGuard};
use foundation_nativeapis::shared::vfs::{MemoryFs, ObservableFs};
use foundation_nativeapis::{VfsEvent, VfsTask};

fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn vfs_task_receives_write_events() {
    let _guard = init_pool();

    let mut fs = ObservableFs::new(MemoryFs::new());
    fs.write_file("/hello.txt", b"world").unwrap();

    let task = VfsTask::from_observable(&mut fs);
    let stop = task.stop_signal();

    let mut stream = foundation_core::valtron::execute(task, None).unwrap();

    // collect_one returns Option<VfsEvent> directly
    if let Some(event) = collect_one(&mut stream) {
        match event {
            VfsEvent::FileOpened { path, .. } => {
                assert_eq!(path, "/hello.txt");
            }
            VfsEvent::FileCreated { path, .. } => {
                assert_eq!(path, "/hello.txt");
            }
            other => panic!("expected FileOpened or FileCreated, got {other:?}"),
        }
    }

    stop.stop();
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn vfs_task_receives_multiple_events() {
    let _guard = init_pool();

    let mut fs = ObservableFs::new(MemoryFs::new());

    // Generate events
    fs.mkdir("/src").unwrap();
    fs.mkdir("/lib").unwrap();
    fs.write_file("/main.rs", b"fn main() {}").unwrap();

    let task = VfsTask::from_observable(&mut fs);

    let mut stream = foundation_core::valtron::execute(task, None).unwrap();

    let mut received = Vec::new();
    for _ in 0..3 {
        if let Some(event) = collect_one(&mut stream) {
            received.push(event);
        }
    }

    assert_eq!(received.len(), 3, "expected 3 events, got {}", received.len());
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn vfs_task_stop_signal_terminates() {
    let _guard = init_pool();

    let mut fs = ObservableFs::new(MemoryFs::new());
    let task = VfsTask::from_observable(&mut fs);
    let stop = task.stop_signal();

    // Signal stop before collecting
    stop.stop();

    let mut stream = foundation_core::valtron::execute(task, None).unwrap();

    // With stop signaled, next_status should return None
    let result = collect_one(&mut stream);
    assert!(result.is_none(), "expected None when stopped");
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn vfs_task_subscribe_for_downstream() {
    let _guard = init_pool();

    let mut fs = ObservableFs::new(MemoryFs::new());
    let mut task = VfsTask::from_observable(&mut fs);

    // Subscribe to re-broadcast events
    let rx = task.subscribe();
    assert_eq!(task.subscriber_count(), 1);

    // Generate an event
    fs.write_file("/test.txt", b"content").unwrap();

    // Consume from the task
    let mut stream = foundation_core::valtron::execute(task, None).unwrap();
    let _ = collect_one(&mut stream);

    // Check the downstream subscriber also received it
    let downstream_event = rx.recv().ok();
    assert!(downstream_event.is_some(), "downstream subscriber should have received event");
}

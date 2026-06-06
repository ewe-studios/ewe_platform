//! Integration tests: VfsTask through valtron executor.

#![cfg(feature = "vfs")]

use foundation_core::valtron::{collect_one, initialize_pool, PoolGuard};
use foundation_nativeapis::shared::vfs::{MemoryFs, ObservableFs, VfsFileSystem};
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
    let task = VfsTask::from_observable(&mut fs);
    let stop = task.stop_signal();

    // create emits a single FileCreated event (no intermediate OperationFailed)
    fs.create("/hello.txt", 0o644).unwrap();

    let mut stream = foundation_core::valtron::execute(task, None).unwrap();

    let event = collect_one(&mut stream);
    stop.stop();

    let event = event.expect("expected at least one event");
    match event {
        VfsEvent::FileCreated { path, .. } => {
            assert_eq!(path, "/hello.txt");
        }
        other => panic!("expected FileCreated, got {other:?}"),
    }
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn vfs_task_receives_multiple_events() {
    let _guard = init_pool();

    let mut fs = ObservableFs::new(MemoryFs::new());
    let task = VfsTask::from_observable(&mut fs);
    let stop = task.stop_signal();

    // Each operation emits one event
    fs.mkdir("/src").unwrap();
    fs.mkdir("/lib").unwrap();
    fs.create("/main.rs", 0o644).unwrap();

    let mut stream = foundation_core::valtron::execute(task, None).unwrap();

    let mut received = Vec::new();
    for _ in 0..3 {
        if let Some(event) = collect_one(&mut stream) {
            received.push(event);
        }
    }

    stop.stop();

    assert!(
        received.len() >= 3,
        "expected at least 3 events, got {}",
        received.len()
    );
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

    stop.stop();

    let mut stream = foundation_core::valtron::execute(task, None).unwrap();

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

    let rx = task.subscribe();
    assert_eq!(task.subscriber_count(), 1);

    // create emits a single clean event
    fs.create("/test.txt", 0o644).unwrap();

    let stop = task.stop_signal();

    let mut stream = foundation_core::valtron::execute(task, None).unwrap();
    let _ = collect_one(&mut stream);

    stop.stop();

    let downstream_event = rx.recv().ok();
    assert!(
        downstream_event.is_some(),
        "downstream subscriber should have received event"
    );
}

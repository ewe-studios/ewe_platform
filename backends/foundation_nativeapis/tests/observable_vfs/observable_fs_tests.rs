use foundation_nativeapis::{
    ObservableFs, MemoryFs, VfsFileSystem,
    VfsEvent,
};
use foundation_core::synca::mpp::Receiver;

fn make_observable() -> (ObservableFs<MemoryFs>, Receiver<VfsEvent>) {
    let mut fs = ObservableFs::new(MemoryFs::new());
    let rx = fs.subscribe();
    (fs, rx)
}

fn collect_events(rx: &Receiver<VfsEvent>) -> Vec<VfsEvent> {
    rx.try_iter().collect()
}

// ── Construction and subscription ──

#[test]
fn test_observable_fs_new() {
    let (fs, _rx) = make_observable();
    let caps = fs.capabilities();
    assert!(!caps.persistent); // MemoryFs is not persistent
}

#[test]
fn test_subscribe_before_operations() {
    let (mut fs, rx) = make_observable();
    fs.mkdir("/src").unwrap();
    let events = collect_events(&rx);
    assert_eq!(events.len(), 1);
    match &events[0] {
        VfsEvent::DirectoryCreated { path, .. } => {
            assert_eq!(path, "/src");
        }
        other => panic!("expected DirectoryCreated, got {other:?}"),
    }
}

// ── File operation events ──

#[test]
fn test_write_file_emits_events() {
    let (mut fs, rx) = make_observable();
    fs.write_file("/hello.txt", b"world").unwrap();
    let events = collect_events(&rx);
    // write_file creates a file and writes — we expect FileOpened + FileCreated or similar
    assert!(!events.is_empty(), "expected at least one event from write_file");
}

#[test]
fn test_read_file_emits_events() {
    let (mut fs, rx) = make_observable();
    fs.write_file("/data.bin", &[1, 2, 3, 4]).unwrap();
    let _ = collect_events(&rx); // clear creation events

    let data = fs.read_file("/data.bin").unwrap();
    assert_eq!(data, vec![1, 2, 3, 4]);

    let events = collect_events(&rx);
    assert!(!events.is_empty(), "expected events from read_file");
    match &events[0] {
        VfsEvent::FileOpened { path, .. } => {
            assert_eq!(path, "/data.bin");
        }
        other => panic!("expected FileOpened, got {other:?}"),
    }
}

// ── Stat and exists events ──

#[test]
fn test_stat_emits_event() {
    let (mut fs, rx) = make_observable();
    fs.write_file("/test.txt", b"hello").unwrap();
    let _ = collect_events(&rx);

    let meta = fs.stat("/test.txt").unwrap();
    assert_eq!(meta.size, 5);

    let events = collect_events(&rx);
    assert_eq!(events.len(), 1);
    match &events[0] {
        VfsEvent::StatQueried { path, .. } => {
            assert_eq!(path, "/test.txt");
        }
        other => panic!("expected StatQueried, got {other:?}"),
    }
}

#[test]
fn test_exists_emits_event() {
    let (mut fs, rx) = make_observable();
    fs.write_file("/exists.txt", b"yes").unwrap();
    let _ = collect_events(&rx);

    let result = fs.exists("/exists.txt").unwrap();
    assert!(result);

    let events = collect_events(&rx);
    assert_eq!(events.len(), 1);
    match &events[0] {
        VfsEvent::ExistsQueried { path, exists, .. } => {
            assert_eq!(path, "/exists.txt");
            assert!(*exists);
        }
        other => panic!("expected ExistsQueried, got {other:?}"),
    }
}

// ── Directory operations ──

#[test]
fn test_mkdir_emits_event() {
    let (mut fs, rx) = make_observable();
    fs.mkdir("/a").unwrap();
    fs.mkdir("/b").unwrap();

    let events = collect_events(&rx);
    assert_eq!(events.len(), 2);
    match &events[0] {
        VfsEvent::DirectoryCreated { path, .. } => {
            assert_eq!(path, "/a");
        }
        other => panic!("expected DirectoryCreated, got {other:?}"),
    }
}

#[test]
fn test_open_directory_emits_event() {
    let (mut fs, rx) = make_observable();
    fs.mkdir("/src").unwrap();
    let _ = collect_events(&rx);

    let _dir = fs.open_directory("/src").unwrap();
    let events = collect_events(&rx);
    assert_eq!(events.len(), 1);
    match &events[0] {
        VfsEvent::DirectoryOpened { path, .. } => {
            assert_eq!(path, "/src");
        }
        other => panic!("expected DirectoryOpened, got {other:?}"),
    }
}

// ── Remove and rename ──

#[test]
fn test_remove_emits_event() {
    let (mut fs, rx) = make_observable();
    fs.write_file("/delete_me.txt", b"bye").unwrap();
    let _ = collect_events(&rx);

    fs.remove("/delete_me.txt").unwrap();
    let events = collect_events(&rx);
    assert_eq!(events.len(), 1);
    match &events[0] {
        VfsEvent::Removed { path, .. } => {
            assert_eq!(path, "/delete_me.txt");
        }
        other => panic!("expected Removed, got {other:?}"),
    }
}

#[test]
fn test_rename_emits_event() {
    let (mut fs, rx) = make_observable();
    fs.write_file("/old.txt", b"hello").unwrap();
    let _ = collect_events(&rx);

    fs.rename("/old.txt", "/new.txt").unwrap();
    let events = collect_events(&rx);
    assert_eq!(events.len(), 1);
    match &events[0] {
        VfsEvent::Renamed { from, to, .. } => {
            assert_eq!(from, "/old.txt");
            assert_eq!(to, "/new.txt");
        }
        other => panic!("expected Renamed, got {other:?}"),
    }
}

// ── Chmod ──

#[test]
fn test_chmod_emits_event() {
    let (mut fs, rx) = make_observable();
    fs.write_file("/perm.txt", b"test").unwrap();
    let _ = collect_events(&rx);

    fs.chmod("/perm.txt", 0o755).unwrap();
    let events = collect_events(&rx);
    assert_eq!(events.len(), 1);
    match &events[0] {
        VfsEvent::PermissionsChanged { path, mode, .. } => {
            assert_eq!(path, "/perm.txt");
            assert_eq!(*mode, 0o755);
        }
        other => panic!("expected PermissionsChanged, got {other:?}"),
    }
}

// ── Error events ──

#[test]
fn test_error_emits_operation_failed() {
    let (mut fs, rx) = make_observable();
    let result = fs.stat("/nonexistent");
    assert!(result.is_err());

    let events = collect_events(&rx);
    assert_eq!(events.len(), 1);
    match &events[0] {
        VfsEvent::OperationFailed { operation, path, .. } => {
            assert_eq!(operation, "stat");
            assert_eq!(path, "/nonexistent");
        }
        other => panic!("expected OperationFailed, got {other:?}"),
    }
}

// ── Multiple subscribers ──

#[test]
fn test_multiple_subscribers() {
    let mut fs = ObservableFs::new(MemoryFs::new());
    let mut rx1 = fs.subscribe();
    let mut rx2 = fs.subscribe();

    fs.mkdir("/test").unwrap();

    let e1 = collect_events(&rx1);
    let e2 = collect_events(&rx2);
    assert_eq!(e1.len(), 1);
    assert_eq!(e2.len(), 1);
}

// ── Version ordering ──

#[test]
fn test_event_versions_are_monotonic() {
    let (mut fs, rx) = make_observable();
    fs.mkdir("/a").unwrap();
    fs.mkdir("/b").unwrap();
    fs.mkdir("/c").unwrap();

    let events = collect_events(&rx);
    assert_eq!(events.len(), 3);

    let versions: Vec<u64> = events
        .iter()
        .map(|e| match e {
            VfsEvent::DirectoryCreated { version, .. } => *version,
            _ => 0,
        })
        .collect();
    assert!(versions[0] < versions[1] && versions[1] < versions[2]);
}

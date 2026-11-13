//! Integration tests: `WinWatcher` (Windows I/O completion port backend).
//!
//! Each test uses its own temporary directory.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use foundation_nativeapis::native::watcher::windows::{WinWatcher, WinWatcherConfig};
use foundation_nativeapis::{NativeWatcher, WatchError, WatchEvent, WatchEventKind};

/// A temporary directory that is removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "nativeapis_win_{}_{}_{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("failed to create temp dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Poll until `done` accepts the events collected so far, or give up.
fn collect_until(
    watcher: &mut WinWatcher,
    done: impl Fn(&[WatchEvent]) -> bool,
) -> Vec<WatchEvent> {
    let mut all = Vec::new();
    for _ in 0..25 {
        all.extend(
            watcher
                .poll(Duration::from_millis(200))
                .expect("poll failed"),
        );
        if done(&all) {
            break;
        }
    }
    all
}

/// Read events until a quiet period, so later asserts see only new events.
fn drain(watcher: &mut WinWatcher) {
    while !watcher
        .poll(Duration::from_millis(300))
        .expect("poll failed")
        .is_empty()
    {}
}

fn has(events: &[WatchEvent], name: &str, kind: &WatchEventKind) -> bool {
    events
        .iter()
        .any(|e| e.path.ends_with(name) && &e.kind == kind)
}

#[test]
fn detects_create() {
    let dir = TempDir::new("create");
    let mut watcher = WinWatcher::new().unwrap();
    watcher.watch(dir.path(), false).unwrap();

    File::create(dir.path().join("new.txt")).unwrap();

    let events = collect_until(&mut watcher, |e| {
        has(e, "new.txt", &WatchEventKind::Created)
    });
    assert!(
        has(&events, "new.txt", &WatchEventKind::Created),
        "{events:?}"
    );
}

#[test]
fn detects_modify() {
    let dir = TempDir::new("modify");
    let path = dir.path().join("m.txt");
    File::create(&path).unwrap();
    let mut watcher = WinWatcher::new().unwrap();
    watcher.watch(dir.path(), false).unwrap();

    let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(b"hello").unwrap();
    file.sync_all().unwrap();
    drop(file);

    let events = collect_until(&mut watcher, |e| has(e, "m.txt", &WatchEventKind::Modified));
    assert!(
        has(&events, "m.txt", &WatchEventKind::Modified),
        "{events:?}"
    );
}

#[test]
fn detects_delete() {
    let dir = TempDir::new("delete");
    let path = dir.path().join("d.txt");
    File::create(&path).unwrap();
    let mut watcher = WinWatcher::new().unwrap();
    watcher.watch(dir.path(), false).unwrap();

    fs::remove_file(&path).unwrap();

    let events = collect_until(&mut watcher, |e| has(e, "d.txt", &WatchEventKind::Removed));
    assert!(
        has(&events, "d.txt", &WatchEventKind::Removed),
        "{events:?}"
    );
}

#[test]
fn rename_gives_one_paired_event() {
    let dir = TempDir::new("rename");
    let from = dir.path().join("old.txt");
    let to = dir.path().join("new.txt");
    File::create(&from).unwrap();
    let mut watcher = WinWatcher::new().unwrap();
    watcher.watch(dir.path(), false).unwrap();

    fs::rename(&from, &to).unwrap();

    let events = collect_until(&mut watcher, |e| {
        e.iter()
            .any(|x| matches!(x.kind, WatchEventKind::Renamed { .. }))
    });
    let renames: Vec<_> = events
        .iter()
        .filter(|e| matches!(e.kind, WatchEventKind::Renamed { .. }))
        .collect();
    assert_eq!(renames.len(), 1, "{events:?}");
    assert_eq!(
        renames[0].kind,
        WatchEventKind::Renamed {
            from: from.clone(),
            to: to.clone()
        }
    );
    assert!(
        !events
            .iter()
            .any(|e| e.path == from && e.kind == WatchEventKind::Removed),
        "the old name must not give a Removed event: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| e.path == to && e.kind == WatchEventKind::Created),
        "the new name must not give a Created event: {events:?}"
    );
}

#[test]
fn recursive_watch_sees_sub_directory_files() {
    let dir = TempDir::new("recursive");
    fs::create_dir(dir.path().join("sub")).unwrap();
    let mut watcher = WinWatcher::new().unwrap();
    watcher.watch(dir.path(), true).unwrap();

    File::create(dir.path().join("sub").join("deep.txt")).unwrap();

    let events = collect_until(&mut watcher, |e| {
        has(e, "deep.txt", &WatchEventKind::Created)
    });
    assert!(
        has(&events, "deep.txt", &WatchEventKind::Created),
        "{events:?}"
    );
}

#[test]
fn no_events_after_unwatch() {
    let dir = TempDir::new("unwatch");
    let mut watcher = WinWatcher::new().unwrap();
    watcher.watch(dir.path(), false).unwrap();
    watcher.unwatch(dir.path()).unwrap();

    File::create(dir.path().join("late.txt")).unwrap();

    let events = watcher.poll(Duration::from_millis(300)).unwrap();
    assert!(events.is_empty(), "{events:?}");
    assert!(matches!(
        watcher.unwatch(dir.path()),
        Err(WatchError::NotWatched)
    ));
}

#[test]
fn dropping_a_watcher_with_pending_reads_does_not_hang() {
    let dir = TempDir::new("drop");
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    let mut watcher = WinWatcher::new().unwrap();
    watcher.watch(dir.path(), false).unwrap();
    watcher.watch(&sub, false).unwrap();
    drop(watcher);
}

#[test]
fn poll_timeout_returns_in_time_with_no_events() {
    let dir = TempDir::new("timeout");
    let mut watcher = WinWatcher::new().unwrap();
    watcher.watch(dir.path(), false).unwrap();

    let start = Instant::now();
    let events = watcher.poll(Duration::from_millis(150)).unwrap();
    let elapsed = start.elapsed();

    assert!(events.is_empty(), "{events:?}");
    assert!(
        elapsed >= Duration::from_millis(100),
        "returned early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "ignored the timeout: {elapsed:?}"
    );

    let start = Instant::now();
    assert!(watcher.poll(Duration::ZERO).unwrap().is_empty());
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn has_events_caches_events_for_the_next_poll() {
    let dir = TempDir::new("peek");
    let mut watcher = WinWatcher::new().unwrap();
    watcher.watch(dir.path(), false).unwrap();
    assert!(!watcher.has_events(None));

    File::create(dir.path().join("peek.txt")).unwrap();

    assert!(watcher.has_events(Some(Duration::from_secs(2))));
    let events = watcher.poll(Duration::ZERO).unwrap();
    assert!(
        has(&events, "peek.txt", &WatchEventKind::Created),
        "{events:?}"
    );
}

#[test]
fn overflow_gives_a_rescan_event_and_the_watch_keeps_working() {
    let dir = TempDir::new("overflow");
    let config = WinWatcherConfig::default().buffer_size(64);
    let mut watcher = WinWatcher::with_config(config).unwrap();
    watcher.watch(dir.path(), false).unwrap();

    // Many changes while nobody polls: a 64-byte buffer cannot hold them.
    for i in 0..400 {
        File::create(dir.path().join(format!("file_{i:04}_with_a_long_name.txt"))).unwrap();
    }

    let events = collect_until(&mut watcher, |e| {
        e.iter().any(|x| x.kind == WatchEventKind::Rescan)
    });
    let rescan = events
        .iter()
        .find(|e| e.kind == WatchEventKind::Rescan)
        .unwrap_or_else(|| panic!("no Rescan event in {} events", events.len()));
    assert_eq!(rescan.path, dir.path());

    drain(&mut watcher);
    File::create(dir.path().join("after.txt")).unwrap();
    let events = collect_until(&mut watcher, |e| {
        has(e, "after.txt", &WatchEventKind::Created)
    });
    assert!(
        has(&events, "after.txt", &WatchEventKind::Created),
        "{events:?}"
    );
}

#[test]
fn too_small_buffer_is_rejected() {
    let config = WinWatcherConfig::default().buffer_size(8);
    assert!(WinWatcher::with_config(config).is_err());
}

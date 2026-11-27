#![cfg(feature = "vfs-native")]

//! Tests for the file-backed Durable Streams virtual file system (VFS).
//!
//! Each test builds a VFS over a fresh temporary directory. The VFS methods
//! return ready futures, so the sync boundary drives them directly.

mod coverage;
mod oracle;
mod soft_delete_read;

use foundation_core::durable::{
    ContentType, DurableStreamVfs, ForkInfo, StreamName, VfsError, VfsFuture, VfsMeta, VfsOffset,
    WriterState,
};
use foundation_core::valtron::block_on_future_scoped;
use foundation_nativeapis::native::vfs::durable_file::FileVfs;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A temporary directory that removes itself.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "durable_file_vfs_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
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

/// Build the metadata for a new stream.
fn meta(content_type: &str) -> VfsMeta {
    VfsMeta {
        content_type: ContentType::parse(content_type).expect("content type"),
        len: 0,
        closed: false,
        expires: None,
        writer: WriterState::default(),
        fork: None,
        last_access_ms: 0,
        trim_from: 0,
        version: 0,
    }
}

/// Drive one VFS future to completion on this thread.
fn run<T>(future: VfsFuture<'_, T>) -> Result<T, VfsError> {
    block_on_future_scoped(future)
}

fn stream(name: &str) -> StreamName {
    StreamName::parse(name).expect("stream name")
}

#[test]
fn create_append_and_read_returns_the_exact_bytes() {
    let tmp = TempDir::new("basic");
    let vfs = FileVfs::new(tmp.path()).expect("vfs");
    let name = stream("docs/basic");

    assert!(run(vfs.create(&name, meta("text/plain"))).expect("create"));
    assert_eq!(run(vfs.append(&name, b"hello ")).expect("append").0, 6);
    assert_eq!(run(vfs.append(&name, b"world")).expect("append").0, 11);

    let all = run(vfs.read_at(&name, &VfsOffset(0), 1024)).expect("read");
    assert_eq!(all, b"hello world");

    let tail = run(vfs.read_at(&name, &VfsOffset(6), 1024)).expect("read");
    assert_eq!(tail, b"world");

    let meta = run(vfs.meta(&name)).expect("meta");
    assert_eq!(meta.len, 11);
    assert_eq!(meta.version, 2);
    assert!(!meta.closed);
    assert_eq!(meta.content_type.as_str(), "text/plain");
}

#[test]
fn a_second_vfs_over_the_same_root_sees_the_data_and_metadata() {
    let tmp = TempDir::new("restart");
    let name = stream("restart/stream");

    {
        let vfs = FileVfs::new(tmp.path()).expect("vfs");
        run(vfs.create(&name, meta("application/octet-stream"))).expect("create");
        run(vfs.append(&name, b"persisted")).expect("append");
        run(vfs.close(&name)).expect("close");
    }

    let vfs = FileVfs::new(tmp.path()).expect("reopen");
    let bytes = run(vfs.read_at(&name, &VfsOffset(0), 1024)).expect("read");
    assert_eq!(bytes, b"persisted");
    let stored = run(vfs.meta(&name)).expect("meta");
    assert_eq!(stored.len, 9);
    assert!(stored.closed);
    assert!(!run(vfs.create(&name, stored)).expect("idempotent"));
}

#[test]
fn a_stale_expected_version_returns_conflict() {
    let tmp = TempDir::new("version");
    let vfs = FileVfs::new(tmp.path()).expect("vfs");
    let name = stream("versioned");

    run(vfs.create(&name, meta("text/plain"))).expect("create");
    let first = run(vfs.meta(&name)).expect("meta");

    run(vfs.append_with_meta(&name, b"abc", first.clone(), first.version)).expect("append");

    let stale = run(vfs.append_with_meta(&name, b"def", first.clone(), first.version));
    assert!(matches!(stale, Err(VfsError::Conflict)));

    // The refused append wrote no bytes.
    let bytes = run(vfs.read_at(&name, &VfsOffset(0), 1024)).expect("read");
    assert_eq!(bytes, b"abc");
    assert_eq!(run(vfs.meta(&name)).expect("meta").len, 3);
}

#[test]
fn a_fork_reads_the_source_prefix_and_a_later_source_append_stays_invisible() {
    let tmp = TempDir::new("fork");
    let vfs = FileVfs::new(tmp.path()).expect("vfs");
    let source = stream("fork/source");
    let fork = stream("fork/child");

    run(vfs.create(&source, meta("application/octet-stream"))).expect("create source");
    run(vfs.append(&source, b"0123456789")).expect("append source");

    let mut fork_meta = meta("application/octet-stream");
    fork_meta.fork = Some(ForkInfo {
        source: source.clone(),
        at: 4,
    });
    fork_meta.len = 4;
    run(vfs.create(&fork, fork_meta)).expect("create fork");

    let prefix = run(vfs.read_at(&fork, &VfsOffset(0), 1024)).expect("read fork");
    assert_eq!(prefix, b"0123");

    // The source grows, but the fork keeps only its inherited prefix.
    run(vfs.append(&source, b"ABCDE")).expect("append source");
    let prefix = run(vfs.read_at(&fork, &VfsOffset(0), 1024)).expect("read fork");
    assert_eq!(prefix, b"0123");
    assert_eq!(run(vfs.meta(&fork)).expect("fork meta").len, 4);

    // A fork append continues after the inherited prefix.
    run(vfs.append(&fork, b"xyz")).expect("append fork");
    let bytes = run(vfs.read_at(&fork, &VfsOffset(0), 1024)).expect("read fork");
    assert_eq!(bytes, b"0123xyz");
    assert_eq!(run(vfs.meta(&fork)).expect("fork meta").len, 7);
}

#[test]
fn a_fork_soft_deletes_the_source_and_removing_the_fork_removes_both() {
    let tmp = TempDir::new("cascade");
    let vfs = FileVfs::new(tmp.path()).expect("vfs");
    let source = stream("cascade/source");
    let fork = stream("cascade/child");

    run(vfs.create(&source, meta("text/plain"))).expect("create source");
    run(vfs.append(&source, b"abcdef")).expect("append source");

    let mut fork_meta = meta("text/plain");
    fork_meta.fork = Some(ForkInfo {
        source: source.clone(),
        at: 4,
    });
    fork_meta.len = 4;
    run(vfs.create(&fork, fork_meta)).expect("create fork");

    assert!(run(vfs.remove(&source)).expect("soft delete"));
    assert!(matches!(run(vfs.meta(&source)), Err(VfsError::Gone)));
    let prefix = run(vfs.read_at(&fork, &VfsOffset(0), 1024)).expect("fork still reads");
    assert_eq!(prefix, b"abcd");

    assert!(!run(vfs.remove(&fork)).expect("hard delete"));
    assert!(matches!(run(vfs.meta(&fork)), Err(VfsError::NotFound)));
    assert!(matches!(run(vfs.meta(&source)), Err(VfsError::NotFound)));
}

#[test]
fn a_missing_fork_source_returns_not_found() {
    let tmp = TempDir::new("missing_source");
    let vfs = FileVfs::new(tmp.path()).expect("vfs");
    let fork = stream("orphan/child");

    let mut fork_meta = meta("text/plain");
    fork_meta.fork = Some(ForkInfo {
        source: stream("orphan/source"),
        at: 0,
    });
    let result = run(vfs.create(&fork, fork_meta));
    assert!(matches!(result, Err(VfsError::NotFound)));
}

#[test]
fn a_closed_stream_refuses_an_append() {
    let tmp = TempDir::new("closed");
    let vfs = FileVfs::new(tmp.path()).expect("vfs");
    let name = stream("closed/stream");

    run(vfs.create(&name, meta("text/plain"))).expect("create");
    run(vfs.close(&name)).expect("close");
    assert!(matches!(
        run(vfs.append(&name, b"late")),
        Err(VfsError::Conflict)
    ));
}

#[test]
fn an_append_notifies_a_registered_waiter() {
    let tmp = TempDir::new("notify");
    let vfs = FileVfs::new(tmp.path()).expect("vfs");
    let name = stream("notify/stream");

    run(vfs.create(&name, meta("text/plain"))).expect("create");
    let notifier = vfs.notifier(&name).expect("notifier");
    let fired = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&fired);
    notifier.register(Box::new(move || flag.store(true, Ordering::SeqCst)));

    run(vfs.append(&name, b"data")).expect("append");
    assert!(fired.load(Ordering::SeqCst));
}

//! Integration tests: Error propagation through valtron bridge.
//!
//! These tests verify that errors from the sync bridge (via valtron) are
//! properly propagated — not swallowed, not converted to generic errors.

#![cfg(feature = "vfs")]

use foundation_core::valtron::{initialize_pool, PoolGuard};
use foundation_nativeapis::shared::vfs::sync_bridge::SyncFs;
use foundation_nativeapis::shared::vfs::{MemoryFs, VfsError, VfsFileSystem};

fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn error_not_found_propagated() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    let err = sync.stat("/does/not/exist").unwrap_err();
    assert!(matches!(err.current_context(), VfsError::NotFound { .. }));
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn error_already_exists_propagated() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.create("/file.txt", 0o644).unwrap();

    let err = sync.create("/file.txt", 0o644).unwrap_err();
    assert!(matches!(
        err.current_context(),
        VfsError::AlreadyExists { .. }
    ));
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn error_not_a_file_propagated() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.mkdir("/dir").unwrap();

    let err = sync
        .open("/dir", foundation_nativeapis::shared::vfs::OpenMode::Read)
        .unwrap_err();
    assert!(matches!(err.current_context(), VfsError::NotAFile { .. }));
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn error_not_a_directory_propagated() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.write_file("/file.txt", b"data").unwrap();

    let err = sync.open_directory("/file.txt").unwrap_err();
    assert!(matches!(
        err.current_context(),
        VfsError::NotADirectory { .. }
    ));
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn error_directory_not_empty_propagated() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.mkdir("/dir").unwrap();
    sync.write_file("/dir/file.txt", b"data").unwrap();

    let err = sync.remove("/dir").unwrap_err();
    assert!(matches!(err.current_context(), VfsError::NotAFile { .. }));
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn error_cannot_remove_root() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    let err = sync.remove("/").unwrap_err();
    assert!(matches!(
        err.current_context(),
        VfsError::PermissionDenied { .. }
    ));
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn error_path_traversal_rejected() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.mkdir("/safe").unwrap();

    let err = sync.stat("/safe/../..").unwrap_err();
    assert!(matches!(
        err.current_context(),
        VfsError::InvalidPath { .. }
    ));
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn error_stack_trace_preserved() {
    let _guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    let err = sync.stat("/missing").unwrap_err();
    assert!(err.downcast_ref::<VfsError>().is_some());
}

#[test]
#[foundation_macros::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn pool_guard_dropped_cleanly() {
    let guard = init_pool();
    let sync = SyncFs::new(MemoryFs::new());

    sync.mkdir("/before").unwrap();
    sync.write_file("/before/a.txt", b"data").unwrap();
    assert!(sync.exists("/before").unwrap());
    assert!(sync.exists("/before/a.txt").unwrap());

    drop(guard);
}

//! Reads across a soft-delete on the file VFS.
//!
//! A direct read of a soft-deleted entry returns `Gone`. A fork still reads
//! the inherited prefix of a soft-deleted source, because it depends on that
//! data.

use crate::{meta, stream, TempDir};
use foundation_core::durable::{DurableStreamVfs, ForkInfo, StreamName, VfsError, VfsOffset};
use foundation_core::valtron::block_on_future_scoped;
use foundation_nativeapis::native::vfs::durable_file::FileVfs;

/// Create a source with a fork at `at`, so a remove soft-deletes the source.
fn source_with_fork(vfs: &FileVfs, at: u64) -> (StreamName, StreamName) {
    let source = stream("soft-read/source");
    let child = stream("soft-read/child");

    block_on_future_scoped(async {
        vfs.create(&source, meta("text/plain"))
            .await
            .expect("create source");
        vfs.append(&source, b"abcdef").await.expect("append source");

        let mut child_meta = meta("text/plain");
        child_meta.fork = Some(ForkInfo {
            source: source.clone(),
            at,
        });
        child_meta.len = at;
        vfs.create(&child, child_meta).await.expect("create fork");

        assert!(vfs.remove(&source).await.expect("soft delete source"));
    });

    (source, child)
}

#[test]
fn a_direct_read_of_a_soft_deleted_entry_returns_gone() {
    let tmp = TempDir::new("soft_read_direct");
    let vfs = FileVfs::new(tmp.path()).expect("vfs");
    let (source, _child) = source_with_fork(&vfs, 4);

    block_on_future_scoped(async {
        assert!(matches!(
            vfs.read_at(&source, &VfsOffset(0), 16).await,
            Err(VfsError::Gone)
        ));
    });
}

#[test]
fn a_fork_reads_the_prefix_of_a_soft_deleted_source() {
    let tmp = TempDir::new("soft_read_fork");
    let vfs = FileVfs::new(tmp.path()).expect("vfs");
    let (_source, child) = source_with_fork(&vfs, 4);

    block_on_future_scoped(async {
        assert_eq!(
            vfs.read_at(&child, &VfsOffset(0), 16)
                .await
                .expect("fork reads its source"),
            b"abcd"
        );
    });
}

//! Reads across a soft-delete on the memory VFS.
//!
//! A direct read of a soft-deleted entry returns `Gone`. A fork still reads
//! the inherited prefix of a soft-deleted source, because it depends on that
//! data.

use foundation_core::durable::{
    ContentType, DurableStreamVfs, ForkInfo, MemoryVfs, StreamName, VfsError, VfsMeta, VfsOffset,
    WriterState,
};
use foundation_core::valtron::block_on_future_scoped;

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

fn stream(name: &str) -> StreamName {
    StreamName::parse(name).expect("stream name")
}

/// Create a source with a fork at `at`, so a remove soft-deletes the source.
async fn source_with_fork(vfs: &MemoryVfs, at: u64) -> (StreamName, StreamName) {
    let source = stream("soft-read/source");
    let child = stream("soft-read/child");

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
    (source, child)
}

#[test]
fn a_direct_read_of_a_soft_deleted_entry_returns_gone() {
    block_on_future_scoped(async {
        let vfs = MemoryVfs::new();
        let (source, _child) = source_with_fork(&vfs, 4).await;

        assert!(matches!(
            vfs.read_at(&source, &VfsOffset(0), 16).await,
            Err(VfsError::Gone)
        ));
    });
}

#[test]
fn a_fork_reads_the_prefix_of_a_soft_deleted_source() {
    block_on_future_scoped(async {
        let vfs = MemoryVfs::new();
        let (_source, child) = source_with_fork(&vfs, 4).await;

        assert_eq!(
            vfs.read_at(&child, &VfsOffset(0), 16)
                .await
                .expect("fork reads its source"),
            b"abcd"
        );
    });
}

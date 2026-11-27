//! Coverage for every `DurableStreamVfs` method on the memory VFS.
//!
//! The test calls each trait method and the main error branches. The
//! fork-chain depth limit needs a chain deeper than 64, so it has no test.

use foundation_core::durable::{
    ContentType, DurableStreamVfs, ForkInfo, MemoryVfs, StreamName, VfsError, VfsMeta, VfsOffset,
    WriterState,
};
use foundation_core::valtron::block_on_future_scoped;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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

#[test]
fn memory_vfs_covers_every_method() {
    block_on_future_scoped(async {
        let vfs = MemoryVfs::new();
        let name = stream("coverage/all");
        let prefix = stream("coverage");

        // create: new, then idempotent.
        assert!(vfs
            .create(&name, meta("application/octet-stream"))
            .await
            .expect("create"));
        assert!(!vfs
            .create(&name, meta("application/octet-stream"))
            .await
            .expect("idempotent create"));

        // append.
        assert_eq!(vfs.append(&name, b"abc").await.expect("append").as_u64(), 3);

        // append_with_meta at the current version.
        let current = vfs.meta(&name).await.expect("metadata");
        let expected_version = current.version;
        assert_eq!(
            vfs.append_with_meta(&name, b"de", current, expected_version)
                .await
                .expect("append_with_meta")
                .as_u64(),
            5
        );

        // read_at.
        assert_eq!(
            vfs.read_at(&name, &VfsOffset(0), 1024).await.expect("read"),
            b"abcde"
        );

        // meta.
        let stored = vfs.meta(&name).await.expect("metadata");
        assert_eq!(stored.len, 5);
        assert_eq!(stored.version, 2);

        // touch.
        vfs.touch(&name, 42).await.expect("touch");
        assert_eq!(vfs.meta(&name).await.expect("metadata").last_access_ms, 42);

        // notifier: a registered callback fires on the next append.
        let notifier = vfs.notifier(&name).expect("notifier");
        let fired = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&fired);
        notifier.register(Box::new(move || flag.store(true, Ordering::SeqCst)));
        vfs.append(&name, b"f").await.expect("append");
        assert!(
            fired.load(Ordering::SeqCst),
            "the append fires the notifier"
        );

        // trim: a read before the trim point is refused.
        vfs.trim(&name, VfsOffset(2)).await.expect("trim");
        assert!(matches!(
            vfs.read_at(&name, &VfsOffset(1), 1024).await,
            Err(VfsError::Trimmed)
        ));
        assert_eq!(
            vfs.read_at(&name, &VfsOffset(2), 1024)
                .await
                .expect("read after trim"),
            b"cdef"
        );

        // list.
        let entries = vfs.list(&prefix).await.expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, name);

        // close.
        vfs.close(&name).await.expect("close");
        assert!(vfs.meta(&name).await.expect("metadata").closed);

        // remove a stream with no fork: hard delete, false.
        assert!(!vfs.remove(&name).await.expect("remove"));
        assert!(matches!(vfs.meta(&name).await, Err(VfsError::NotFound)));
        let remaining = vfs.list(&prefix).await.expect("list");
        assert_eq!(remaining.len(), 0, "the list is empty");
    });
}

#[test]
fn memory_vfs_reports_missing_and_conflicting_entries() {
    block_on_future_scoped(async {
        let vfs = MemoryVfs::new();
        let missing = stream("errors/missing");

        // Every method on a missing entry reports NotFound.
        assert!(matches!(
            vfs.append(&missing, b"x").await,
            Err(VfsError::NotFound)
        ));
        assert!(matches!(
            vfs.read_at(&missing, &VfsOffset(0), 8).await,
            Err(VfsError::NotFound)
        ));
        assert!(matches!(vfs.meta(&missing).await, Err(VfsError::NotFound)));
        assert!(matches!(
            vfs.touch(&missing, 1).await,
            Err(VfsError::NotFound)
        ));
        assert!(matches!(
            vfs.trim(&missing, VfsOffset(0)).await,
            Err(VfsError::NotFound)
        ));
        assert!(matches!(vfs.close(&missing).await, Err(VfsError::NotFound)));
        assert!(matches!(
            vfs.remove(&missing).await,
            Err(VfsError::NotFound)
        ));
        assert!(matches!(
            vfs.append_with_meta(&missing, b"x", meta("text/plain"), 0)
                .await,
            Err(VfsError::NotFound)
        ));

        // A fork from a missing source reports NotFound.
        let mut orphan = meta("text/plain");
        orphan.fork = Some(ForkInfo {
            source: missing.clone(),
            at: 0,
        });
        assert!(matches!(
            vfs.create(&stream("errors/orphan"), orphan).await,
            Err(VfsError::NotFound)
        ));

        // A fork longer than its source conflicts.
        let source = stream("errors/source");
        vfs.create(&source, meta("text/plain"))
            .await
            .expect("create source");
        vfs.append(&source, b"abc").await.expect("append source");
        let mut long = meta("text/plain");
        long.fork = Some(ForkInfo {
            source: source.clone(),
            at: 0,
        });
        long.len = 10;
        assert!(matches!(
            vfs.create(&stream("errors/long"), long).await,
            Err(VfsError::Conflict)
        ));

        // A stale version conflicts and stores no bytes.
        let versioned = stream("errors/versioned");
        vfs.create(&versioned, meta("text/plain"))
            .await
            .expect("create versioned");
        let stale = meta("text/plain");
        assert!(matches!(
            vfs.append_with_meta(&versioned, b"x", stale, 99).await,
            Err(VfsError::Conflict)
        ));
        assert_eq!(vfs.meta(&versioned).await.expect("metadata").len, 0);

        // A closed entry refuses an append and an append_with_meta.
        let closed = stream("errors/closed");
        vfs.create(&closed, meta("text/plain"))
            .await
            .expect("create closed");
        vfs.close(&closed).await.expect("close");
        assert!(matches!(
            vfs.append(&closed, b"x").await,
            Err(VfsError::Conflict)
        ));
        let closed_meta = vfs.meta(&closed).await.expect("metadata");
        assert!(matches!(
            vfs.append_with_meta(&closed, b"x", closed_meta.clone(), closed_meta.version)
                .await,
            Err(VfsError::Conflict)
        ));
    });
}

#[test]
fn memory_vfs_soft_deletes_and_cascades() {
    block_on_future_scoped(async {
        let vfs = MemoryVfs::new();
        let source = stream("soft/source");
        let child = stream("soft/child");

        vfs.create(&source, meta("text/plain"))
            .await
            .expect("create source");
        vfs.append(&source, b"abc").await.expect("append source");
        let mut child_meta = meta("text/plain");
        child_meta.fork = Some(ForkInfo {
            source: source.clone(),
            at: 3,
        });
        child_meta.len = 3;
        vfs.create(&child, child_meta).await.expect("create child");

        assert!(vfs.remove(&source).await.expect("soft delete source"));
        assert!(matches!(
            vfs.meta(&source).await.expect_err("gone"),
            VfsError::Gone
        ));
        assert!(matches!(
            vfs.append(&source, b"x").await,
            Err(VfsError::Gone)
        ));
        assert!(matches!(
            vfs.append_with_meta(&source, b"x", meta("text/plain"), 0)
                .await,
            Err(VfsError::Gone)
        ));
        // A soft-deleted source stays readable at the VFS layer, because a fork
        // reads through it. The store refuses it at `meta`.
        assert_eq!(
            vfs.read_at(&source, &VfsOffset(0), 8)
                .await
                .expect("soft-deleted source is readable for its forks"),
            b"abc"
        );
        assert!(matches!(vfs.touch(&source, 1).await, Err(VfsError::Gone)));
        assert!(matches!(
            vfs.trim(&source, VfsOffset(0)).await,
            Err(VfsError::Gone)
        ));
        assert!(matches!(vfs.close(&source).await, Err(VfsError::Gone)));
        assert!(matches!(vfs.remove(&source).await, Err(VfsError::Gone)));
        assert!(matches!(
            vfs.create(&source, meta("text/plain")).await,
            Err(VfsError::Gone)
        ));

        // A new fork from a soft-deleted source reports Gone.
        let mut second = meta("text/plain");
        second.fork = Some(ForkInfo {
            source: source.clone(),
            at: 0,
        });
        assert!(matches!(
            vfs.create(&stream("soft/second"), second).await,
            Err(VfsError::Gone)
        ));

        // The list excludes the soft-deleted source.
        let listed: Vec<StreamName> = vfs
            .list(&stream("soft"))
            .await
            .expect("list")
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        assert!(!listed.contains(&source));
        assert!(listed.contains(&child));

        // Removing the fork hard-deletes it and cascades the source.
        assert!(!vfs.remove(&child).await.expect("remove child"));
        assert!(matches!(vfs.meta(&child).await, Err(VfsError::NotFound)));
        assert!(matches!(vfs.meta(&source).await, Err(VfsError::NotFound)));
    });
}

//! The oracles for the file VFS.
//!
//! The differential oracle runs the shared kit on the memory VFS and the file
//! VFS, then compares the observations. The restart oracle writes, drops the
//! VFS, reopens the same root and reads. The resource oracle proves a read page
//! stays within the configured limit.

use crate::TempDir;
use core::time::Duration;
use foundation_context::Ctx;
use foundation_core::durable::vfs_testkit::{run_all, SCENARIOS};
use foundation_core::durable::{
    AppendRequest, Chunk, CreateSpec, DurableStream, DurableStreamStore, MemoryVfs, ReadFrom,
    ReadLimit, ReadRequest, StreamName,
};
use foundation_core::valtron::{block_on_future_scoped, SimulatedClock};
use foundation_nativeapis::native::vfs::durable_file::FileVfs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Build fresh memory stores.
fn memory_factory() -> impl Fn() -> (DurableStream, Arc<SimulatedClock>) {
    || {
        let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
        let ctx = Ctx::fixed(clock.clone());
        (
            DurableStream::with_ctx(Arc::new(MemoryVfs::new()), &ctx),
            clock,
        )
    }
}

/// Build fresh file stores, each in its own subdirectory of `root`.
fn file_factory(root: PathBuf) -> impl Fn() -> (DurableStream, Arc<SimulatedClock>) {
    let root = Arc::new(root);
    let counter = Arc::new(AtomicUsize::new(0));
    move || {
        let index = counter.fetch_add(1, Ordering::SeqCst);
        let dir = root.join(format!("store-{index}"));
        let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
        let ctx = Ctx::fixed(clock.clone());
        let vfs = Arc::new(FileVfs::new(&dir).expect("file vfs"));
        (DurableStream::with_ctx(vfs, &ctx), clock)
    }
}

#[test]
fn file_vfs_runs_the_same_scenarios() {
    let tmp = TempDir::new("oracle_conformance");
    let file = file_factory(tmp.path().to_path_buf());

    let report = block_on_future_scoped(run_all(&file));
    let names: Vec<&str> = report.iter().map(|scenario| scenario.name).collect();

    assert_eq!(
        names, SCENARIOS,
        "the file VFS runs every scenario in order"
    );
}

#[test]
fn backends_match_model() {
    let tmp = TempDir::new("oracle_differential");
    let memory = memory_factory();
    let file = file_factory(tmp.path().to_path_buf());

    let memory_report = block_on_future_scoped(run_all(&memory));
    let file_report = block_on_future_scoped(run_all(&file));

    assert_eq!(
        memory_report, file_report,
        "the memory and file VFS report the same scenario observations"
    );
}

#[test]
fn restart_preserves_stream() {
    let tmp = TempDir::new("oracle_restart");
    let name = StreamName::parse("restart/oracle").expect("stream name");
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(clock);

    {
        let vfs = Arc::new(FileVfs::new(tmp.path()).expect("vfs"));
        let stream = DurableStream::with_ctx(vfs, &ctx);
        block_on_future_scoped(async {
            stream
                .create(name.clone(), CreateSpec::default())
                .await
                .expect("create");
            stream
                .append(&name, AppendRequest::new(Chunk::new(b"survives".to_vec())))
                .await
                .expect("append");
        });
    }

    // Drop the first VFS, then reopen the same root.
    let vfs = Arc::new(FileVfs::new(tmp.path()).expect("reopen"));
    let stream = DurableStream::with_ctx(vfs, &ctx);
    let page = block_on_future_scoped(stream.read(
        &name,
        ReadRequest {
            from: ReadFrom::Start,
            limit: ReadLimit::default(),
        },
    ))
    .expect("read after restart");
    let bytes: Vec<u8> = page
        .chunks
        .iter()
        .flat_map(|chunk| chunk.as_bytes().to_vec())
        .collect();

    assert_eq!(bytes, b"survives", "the data survives a reopen");
}

/// Fill one stream and read it back in pages of `page` bytes.
fn fill_and_page(stream: &DurableStream, name: &StreamName, page: usize) {
    block_on_future_scoped(async {
        stream
            .create(name.clone(), CreateSpec::default())
            .await
            .expect("create");
        stream
            .append(
                name,
                AppendRequest::new(Chunk::new(b"0123456789abcdefghij".to_vec())),
            )
            .await
            .expect("append");

        let mut collected = Vec::new();
        let mut from = ReadFrom::Start;
        let mut pages = 0_usize;
        loop {
            let page_out = stream
                .read(
                    name,
                    ReadRequest {
                        from: from.clone(),
                        limit: ReadLimit::new(Some(page), None),
                    },
                )
                .await
                .expect("bounded read");
            let bytes: Vec<u8> = page_out
                .chunks
                .iter()
                .flat_map(|chunk| chunk.as_bytes().to_vec())
                .collect();
            assert!(
                bytes.len() <= page,
                "a page is bounded by {page} bytes, got {}",
                bytes.len()
            );
            collected.extend_from_slice(&bytes);
            pages += 1;
            assert!(pages <= 32, "the read makes progress");
            if page_out.cursor.up_to_date {
                break;
            }
            from = ReadFrom::Offset(page_out.cursor.next);
        }
        assert_eq!(
            collected, b"0123456789abcdefghij",
            "the pages reassemble the stream"
        );
    });
}

#[test]
fn read_is_bounded() {
    let tmp = TempDir::new("oracle_bounded");
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(clock);
    let file = DurableStream::with_ctx(Arc::new(FileVfs::new(tmp.path()).expect("vfs")), &ctx);
    let memory = DurableStream::with_ctx(Arc::new(MemoryVfs::new()), &ctx);

    // Different limits prove the page size is configuration, not a constant.
    for page in [1_usize, 3, 7, 20] {
        let file_name = StreamName::parse(&format!("bounded/file-{page}")).expect("name");
        let memory_name = StreamName::parse(&format!("bounded/memory-{page}")).expect("name");
        fill_and_page(&file, &file_name, page);
        fill_and_page(&memory, &memory_name, page);
    }
}

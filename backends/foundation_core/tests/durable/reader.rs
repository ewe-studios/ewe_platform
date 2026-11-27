//! BDD scenarios for the core reader state rules.

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, Chunk, CloseRequest, ContentType, CreateSpec, DurableStream, DurableStreamVfs,
    DurableStreams, MemoryVfs, ReadFrom, ReadLimit, ReadRequest, StreamName, StreamOffset, VfsMeta,
    WriterState,
};
use proptest::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn streams() -> DurableStreams<DurableStream> {
    DurableStreams::new(DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host()))
}

fn bytes(page: &foundation_core::durable::ReadPage) -> Vec<u8> {
    page.chunks
        .iter()
        .flat_map(|chunk| chunk.as_bytes().iter().copied())
        .collect()
}

fn offset(value: u64) -> StreamOffset {
    StreamOffset::from_bytes(value.to_be_bytes())
}

#[test]
fn given_a_stream_when_read_from_an_offset_then_the_suffix_is_returned() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("reader/suffix").expect("valid");
        let stream = streams.stream(name);

        stream.create(CreateSpec::default()).await.expect("create");
        stream
            .append(AppendRequest::new(Chunk::new(b"abcde".to_vec())))
            .await
            .expect("append");

        let page = stream
            .read(ReadRequest {
                from: ReadFrom::Offset(offset(2)),
                limit: ReadLimit::default(),
            })
            .await
            .expect("read");

        assert_eq!(bytes(&page), b"cde");
        assert!(page.cursor.up_to_date);
    });
}

#[test]
fn given_a_closed_stream_when_read_from_start_then_the_page_is_closed() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("reader/closed").expect("valid");
        let stream = streams.stream(name);

        stream.create(CreateSpec::default()).await.expect("create");
        stream
            .append(AppendRequest::new(Chunk::new(b"data".to_vec())))
            .await
            .expect("append");
        stream
            .close(CloseRequest {
                final_chunk: None,
                producer: None,
            })
            .await
            .expect("close");

        let page = stream
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("read");

        assert_eq!(bytes(&page), b"data");
        assert!(page.cursor.up_to_date);
        assert!(page.cursor.closed);
    });
}

#[test]
fn given_a_closed_stream_when_read_a_partial_page_then_it_is_not_closed() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("reader/partial").expect("valid");
        let stream = streams.stream(name);

        stream.create(CreateSpec::default()).await.expect("create");
        stream
            .append(AppendRequest::new(Chunk::new(b"abcdef".to_vec())))
            .await
            .expect("append");
        stream
            .close(CloseRequest {
                final_chunk: None,
                producer: None,
            })
            .await
            .expect("close");

        let page = stream
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::new(Some(3), None),
            })
            .await
            .expect("read");

        assert_eq!(bytes(&page), b"abc");
        assert!(!page.cursor.up_to_date);
        assert!(!page.cursor.closed);

        let next = stream
            .read(ReadRequest {
                from: ReadFrom::Offset(page.cursor.next),
                limit: ReadLimit::default(),
            })
            .await
            .expect("read");

        assert_eq!(bytes(&next), b"def");
        assert!(next.cursor.up_to_date);
        assert!(next.cursor.closed);
    });
}

#[test]
fn given_a_stream_when_it_appends_then_the_notifier_fires() {
    let vfs = Arc::new(MemoryVfs::new());
    let name = StreamName::parse("reader/notifier").expect("valid");
    let meta = VfsMeta {
        content_type: ContentType::default(),
        len: 0,
        closed: false,
        expires: None,
        writer: WriterState::default(),
        fork: None,
        last_access_ms: 0,
        trim_from: 0,
        version: 0,
    };
    foundation_core::valtron::block_on_future_scoped(async {
        vfs.create(&name, meta).await.expect("create")
    });

    let fired = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&fired);
    let notifier = vfs.notifier(&name).expect("notifier");
    notifier.register(Box::new(move || flag.store(true, Ordering::SeqCst)));

    foundation_core::valtron::block_on_future_scoped(async {
        vfs.append(&name, b"x").await.expect("append")
    });

    assert!(fired.load(Ordering::SeqCst));
}

#[test]
fn now_catchup_empty() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("reader/tail").expect("valid");
        let stream = streams.stream(name);

        stream.create(CreateSpec::default()).await.expect("create");
        stream
            .append(AppendRequest::new(Chunk::new(b"data".to_vec())))
            .await
            .expect("append");

        let page = stream
            .read(ReadRequest {
                from: ReadFrom::Tail,
                limit: ReadLimit::default(),
            })
            .await
            .expect("read");

        assert_eq!(page.chunks, [] as [foundation_core::durable::Chunk; 0]);
        assert_eq!(page.cursor.next, offset(4));
        assert!(page.cursor.up_to_date);
        assert!(!page.cursor.closed);
    });
}

#[test]
fn now_closed_returns_immediately() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("reader/tail-closed").expect("valid");
        let stream = streams.stream(name);

        stream.create(CreateSpec::default()).await.expect("create");
        stream
            .append(AppendRequest::new(Chunk::new(b"data".to_vec())))
            .await
            .expect("append");
        stream
            .close(CloseRequest {
                final_chunk: None,
                producer: None,
            })
            .await
            .expect("close");

        let page = stream
            .read(ReadRequest {
                from: ReadFrom::Tail,
                limit: ReadLimit::default(),
            })
            .await
            .expect("read");

        assert_eq!(page.chunks, [] as [foundation_core::durable::Chunk; 0]);
        assert_eq!(page.cursor.next, offset(4));
        assert!(page.cursor.up_to_date);
        assert!(page.cursor.closed);
    });
}

proptest! {
    #[test]
    fn read_from_offset_is_suffix(
        data in prop::collection::vec(any::<u8>(), 1..64),
        start in 0_usize..64,
    ) {
        let start = start.min(data.len());
        let (full, suffix) = foundation_core::valtron::block_on_future_scoped(async {
            let streams = streams();
            let name = StreamName::parse("reader/suffix-oracle").expect("valid");
            let stream = streams.stream(name);
            stream.create(CreateSpec::default()).await.expect("create");
            stream
                .append(AppendRequest::new(Chunk::new(data.clone())))
                .await
                .expect("append");

            let full = stream
                .read(ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::default(),
                })
                .await
                .expect("full read");
            let suffix = stream
                .read(ReadRequest {
                    from: ReadFrom::Offset(offset(start as u64)),
                    limit: ReadLimit::default(),
                })
                .await
                .expect("suffix read");
            (bytes(&full), bytes(&suffix))
        });

        prop_assert_eq!(full, data.clone());
        prop_assert_eq!(suffix, data[start..].to_vec());
    }
}

proptest! {
    #[test]
    fn model_matches_driver(
        chunks in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..20), 0..8),
        close in any::<bool>(),
        page in 1_usize..16,
    ) {
        let model: Vec<u8> = chunks.iter().flatten().copied().collect();
        let pages = foundation_core::valtron::block_on_future_scoped(async {
            let streams = streams();
            let name = StreamName::parse("reader/model").expect("valid");
            let stream = streams.stream(name);
            stream.create(CreateSpec::default()).await.expect("create");

            for chunk in &chunks {
                stream
                    .append(AppendRequest::new(Chunk::new(chunk.clone())))
                    .await
                    .expect("append");
            }
            if close {
                stream
                    .close(CloseRequest {
                        final_chunk: None,
                        producer: None,
                    })
                    .await
                    .expect("close");
            }

            let mut collected = Vec::new();
            let mut from = ReadFrom::Start;
            let mut pages = Vec::new();
            loop {
                let page_out = stream
                    .read(ReadRequest {
                        from: from.clone(),
                        limit: ReadLimit::new(Some(page), None),
                    })
                    .await
                    .expect("read");
                let page_bytes = bytes(&page_out);
                pages.push((page_bytes.clone(), page_out.cursor.up_to_date, page_out.cursor.closed));
                collected.extend_from_slice(&page_bytes);
                if page_out.cursor.up_to_date {
                    break;
                }
                from = ReadFrom::Offset(page_out.cursor.next);
            }
            (pages, collected)
        });
        let (pages, collected) = pages;

        prop_assert_eq!(&collected, &model);
        let (_, up_to_date, closed) = pages.last().expect("at least one page");
        prop_assert!(*up_to_date);
        prop_assert_eq!(*closed, close);
        for (page_bytes, _, _) in &pages[..pages.len() - 1] {
            prop_assert!(page_bytes.len() <= page);
        }
    }
}

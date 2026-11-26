//! BDD scenarios for fork and soft-delete.

use foundation_core::durable::{
    AppendRequest, BadRequestKind, Chunk, ContentType, CreateSpec, DurableStream, DurableStreams,
    ForkSpec, MemoryVfs, ReadFrom, ReadLimit, ReadRequest, StoreError, StreamName, StreamOffset,
};
use std::sync::Arc;

fn streams() -> DurableStreams<DurableStream> {
    DurableStreams::new(DurableStream::new(Arc::new(MemoryVfs::new())))
}

async fn create(streams: &DurableStreams<DurableStream>, name: &StreamName) {
    streams
        .stream(name.clone())
        .create(CreateSpec::default())
        .await
        .expect("create");
}

async fn append(streams: &DurableStreams<DurableStream>, name: &StreamName, data: &'static [u8]) {
    streams
        .stream(name.clone())
        .append(AppendRequest::new(Chunk::new(data.to_vec())))
        .await
        .expect("append");
}

async fn read_all(streams: &DurableStreams<DurableStream>, name: &StreamName) -> Vec<u8> {
    let page = streams
        .stream(name.clone())
        .read(ReadRequest {
            from: ReadFrom::Start,
            limit: ReadLimit::default(),
        })
        .await
        .expect("read");
    page.chunks
        .iter()
        .flat_map(|chunk| chunk.as_bytes().iter().copied())
        .collect()
}

fn offset(value: u64) -> StreamOffset {
    StreamOffset::from_bytes(value.to_be_bytes())
}

#[test]
fn given_a_source_when_a_fork_reads_then_it_spans_the_boundary() {
    smol::block_on(async {
        let streams = streams();
        let source = StreamName::parse("fork/source").expect("valid");
        let fork = StreamName::parse("fork/target").expect("valid");
        create(&streams, &source).await;
        append(&streams, &source, b"abc").await;

        streams
            .stream(fork.clone())
            .fork(ForkSpec {
                source: source.clone(),
                at: Some(offset(2)),
                sub_offset: 0,
            })
            .await
            .expect("fork");

        assert_eq!(read_all(&streams, &fork).await, b"ab");

        append(&streams, &fork, b"X").await;
        assert_eq!(read_all(&streams, &fork).await, b"abX");
        assert_eq!(read_all(&streams, &source).await, b"abc");
    });
}

#[test]
fn given_a_source_when_it_appends_after_the_fork_then_the_fork_does_not_see_it() {
    smol::block_on(async {
        let streams = streams();
        let source = StreamName::parse("fork/hidden-source").expect("valid");
        let fork = StreamName::parse("fork/hidden-target").expect("valid");
        create(&streams, &source).await;
        append(&streams, &source, b"abc").await;

        streams
            .stream(fork.clone())
            .fork(ForkSpec {
                source: source.clone(),
                at: Some(offset(2)),
                sub_offset: 0,
            })
            .await
            .expect("fork");

        append(&streams, &source, b"d").await;
        assert_eq!(read_all(&streams, &fork).await, b"ab");
    });
}

#[test]
fn given_a_fork_when_the_source_is_deleted_then_the_fork_still_reads() {
    smol::block_on(async {
        let streams = streams();
        let source = StreamName::parse("fork/soft-source").expect("valid");
        let fork = StreamName::parse("fork/soft-target").expect("valid");
        create(&streams, &source).await;
        append(&streams, &source, b"abc").await;

        streams
            .stream(fork.clone())
            .fork(ForkSpec {
                source: source.clone(),
                at: Some(offset(2)),
                sub_offset: 0,
            })
            .await
            .expect("fork");

        let deleted = streams
            .stream(source.clone())
            .delete()
            .await
            .expect("delete");
        assert!(deleted.soft_deleted);
        assert!(matches!(
            streams.stream(source.clone()).head().await,
            Err(StoreError::Gone)
        ));
        assert_eq!(read_all(&streams, &fork).await, b"ab");
    });
}

#[test]
fn given_the_last_fork_when_it_is_deleted_then_the_source_is_cleaned_up() {
    smol::block_on(async {
        let streams = streams();
        let source = StreamName::parse("fork/cascade-source").expect("valid");
        let fork = StreamName::parse("fork/cascade-target").expect("valid");
        create(&streams, &source).await;
        append(&streams, &source, b"abc").await;

        streams
            .stream(fork.clone())
            .fork(ForkSpec {
                source: source.clone(),
                at: Some(offset(2)),
                sub_offset: 0,
            })
            .await
            .expect("fork");

        streams
            .stream(source.clone())
            .delete()
            .await
            .expect("soft delete");
        streams
            .stream(fork.clone())
            .delete()
            .await
            .expect("delete fork");

        assert!(matches!(
            streams.stream(source).head().await,
            Err(StoreError::NotFound)
        ));
    });
}

#[test]
fn given_a_binary_source_when_a_fork_uses_sub_offset_then_it_skips_bytes() {
    smol::block_on(async {
        let streams = streams();
        let source = StreamName::parse("fork/binary-sub-source").expect("valid");
        let fork = StreamName::parse("fork/binary-sub-target").expect("valid");
        create(&streams, &source).await;
        append(&streams, &source, b"abcde").await;

        streams
            .stream(fork.clone())
            .fork(ForkSpec {
                source,
                at: Some(offset(1)),
                sub_offset: 2,
            })
            .await
            .expect("fork");

        // The sub-offset moves the divergence point: the fork inherits source bytes 0..3.
        assert_eq!(read_all(&streams, &fork).await, b"abc");
    });
}

#[test]
fn given_a_json_source_when_a_fork_uses_sub_offset_then_it_skips_messages() {
    smol::block_on(async {
        let streams = streams();
        let source = StreamName::parse("fork/json-sub-source").expect("valid");
        let fork = StreamName::parse("fork/json-sub-target").expect("valid");
        streams
            .stream(source.clone())
            .create(CreateSpec {
                content_type: Some(ContentType::parse("application/json").expect("valid")),
                initial: Some(Chunk::new(br#"[{"a":1},{"b":2},{"c":3}]"#.to_vec())),
                ..CreateSpec::default()
            })
            .await
            .expect("create");

        streams
            .stream(fork.clone())
            .fork(ForkSpec {
                source,
                at: Some(offset(0)),
                sub_offset: 2,
            })
            .await
            .expect("fork");

        // The sub-offset moves the divergence point: the fork inherits the first two messages.
        assert_eq!(read_all(&streams, &fork).await, br#"{"a":1}{"b":2}"#);
    });
}

#[test]
fn given_a_sub_offset_past_the_end_then_it_is_refused() {
    smol::block_on(async {
        let streams = streams();
        let source = StreamName::parse("fork/sub-overshoot-source").expect("valid");
        create(&streams, &source).await;
        append(&streams, &source, b"abc").await;

        let error = streams
            .stream(StreamName::parse("fork/sub-overshoot-target").expect("valid"))
            .fork(ForkSpec {
                source,
                at: Some(offset(1)),
                sub_offset: 99,
            })
            .await
            .expect_err("overshoot");

        assert!(matches!(
            error,
            StoreError::BadRequest(BadRequestKind::Fork)
        ));
    });
}

#[test]
fn given_a_fork_when_it_is_created_twice_then_the_second_call_is_idempotent() {
    smol::block_on(async {
        let streams = streams();
        let source = StreamName::parse("fork/idempotent-source").expect("valid");
        let fork = StreamName::parse("fork/idempotent-target").expect("valid");
        create(&streams, &source).await;
        append(&streams, &source, b"abc").await;

        let spec = ForkSpec {
            source: source.clone(),
            at: Some(offset(2)),
            sub_offset: 0,
        };
        let first = streams
            .stream(fork.clone())
            .fork(spec.clone())
            .await
            .expect("first fork");
        assert!(first.created);

        let second = streams.stream(fork).fork(spec).await.expect("second fork");
        assert!(!second.created);
    });
}

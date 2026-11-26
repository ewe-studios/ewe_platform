//! Regression scenarios from the reference server conformance suite.
//!
//! Each test names one wire rule the suite checks. The suite runs over HTTP;
//! these tests pin the same rule at the core.

use foundation_core::durable::{
    AppendKind, AppendRequest, Chunk, CloseRequest, ContentType, CreateSpec, DurableStream,
    DurableStreams, ForkSpec, MemoryVfs, Producer, ProducerId, ReadFrom, ReadLimit, ReadRequest,
    StoreError, StreamName,
};
use std::sync::Arc;

fn streams() -> DurableStreams<DurableStream> {
    DurableStreams::new(DurableStream::new(Arc::new(MemoryVfs::new())))
}

fn name(value: &str) -> StreamName {
    StreamName::parse(value).expect("valid name")
}

async fn create_text(streams: &DurableStreams<DurableStream>, stream: &StreamName) {
    streams
        .stream(stream.clone())
        .create(CreateSpec {
            content_type: Some(ContentType::parse("text/plain").expect("type")),
            ..CreateSpec::default()
        })
        .await
        .expect("create");
}

async fn read_all(streams: &DurableStreams<DurableStream>, stream: &StreamName) -> Vec<u8> {
    let page = streams
        .stream(stream.clone())
        .read(ReadRequest {
            from: ReadFrom::Start,
            limit: ReadLimit::default(),
        })
        .await
        .expect("read");
    page.chunks
        .into_iter()
        .flat_map(|chunk| chunk.as_bytes().to_vec())
        .collect()
}

#[test]
fn given_a_create_with_a_body_and_closed_when_it_lands_then_the_body_is_readable_and_closed() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let stream = name("closed-with-body");
        let outcome = streams
            .stream(stream.clone())
            .create(CreateSpec {
                content_type: Some(ContentType::parse("text/plain").expect("type")),
                closed: true,
                initial: Some(Chunk::new(b"initial".to_vec())),
                ..CreateSpec::default()
            })
            .await
            .expect("create");

        assert!(outcome.created);
        assert!(outcome.head.closed);
        assert_eq!(read_all(&streams, &stream).await, b"initial");
    });
}

#[test]
fn given_a_fork_without_a_content_type_when_it_lands_then_it_inherits_the_source_type() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let source = name("fork-inherit-src");
        let fork = name("fork-inherit");
        create_text(&streams, &source).await;

        let outcome = streams
            .stream(fork)
            .create(CreateSpec {
                fork: Some(ForkSpec {
                    source,
                    at: None,
                    sub_offset: 0,
                }),
                ..CreateSpec::default()
            })
            .await
            .expect("fork");

        assert_eq!(outcome.head.content_type.as_str(), "text/plain");
    });
}

#[test]
fn given_a_fork_with_a_different_content_type_when_it_lands_then_it_is_a_mismatch() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let source = name("fork-mismatch-src");
        let fork = name("fork-mismatch");
        create_text(&streams, &source).await;

        let error = streams
            .stream(fork)
            .create(CreateSpec {
                content_type: Some(ContentType::parse("application/json").expect("type")),
                fork: Some(ForkSpec {
                    source,
                    at: None,
                    sub_offset: 0,
                }),
                ..CreateSpec::default()
            })
            .await
            .expect_err("mismatch");

        assert!(matches!(error, StoreError::ContentTypeMismatch { .. }));
    });
}

#[test]
fn given_a_soft_deleted_stream_when_it_is_re_created_or_forked_then_it_is_a_config_conflict() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let source = name("gone-src");
        let fork = name("gone-fork");
        let tail = name("gone-tail");
        create_text(&streams, &source).await;
        streams
            .stream(tail.clone())
            .create(CreateSpec {
                fork: Some(ForkSpec {
                    source: source.clone(),
                    at: None,
                    sub_offset: 0,
                }),
                ..CreateSpec::default()
            })
            .await
            .expect("fork");

        streams
            .stream(source.clone())
            .delete()
            .await
            .expect("delete");

        let recreate = streams
            .stream(source.clone())
            .create(CreateSpec {
                content_type: Some(ContentType::parse("text/plain").expect("type")),
                ..CreateSpec::default()
            })
            .await
            .expect_err("recreate");
        assert!(matches!(recreate, StoreError::ConfigConflict));

        let refork = streams
            .stream(fork)
            .create(CreateSpec {
                fork: Some(ForkSpec {
                    source,
                    at: None,
                    sub_offset: 0,
                }),
                ..CreateSpec::default()
            })
            .await
            .expect_err("refork");
        assert!(matches!(refork, StoreError::ConfigConflict));
    });
}

#[test]
fn given_a_producer_close_when_it_is_retried_then_only_the_same_tuple_is_a_duplicate() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let stream = name("producer-close");
        create_text(&streams, &stream).await;
        let producer = Producer {
            id: ProducerId::parse("p1").expect("id"),
            epoch: 0,
            seq: 0,
        };

        let first = streams
            .stream(stream.clone())
            .close(CloseRequest {
                final_chunk: Some(Chunk::new(b"final".to_vec())),
                producer: Some(producer.clone()),
            })
            .await
            .expect("close");
        assert!(matches!(first.kind, AppendKind::Created));

        let duplicate = streams
            .stream(stream.clone())
            .close(CloseRequest {
                final_chunk: Some(Chunk::new(b"other".to_vec())),
                producer: Some(producer),
            })
            .await
            .expect("duplicate");
        assert!(matches!(duplicate.kind, AppendKind::Duplicate));
        assert_eq!(read_all(&streams, &stream).await, b"final");

        let different_seq = streams
            .stream(stream.clone())
            .close(CloseRequest {
                final_chunk: None,
                producer: Some(Producer {
                    id: ProducerId::parse("p1").expect("id"),
                    epoch: 0,
                    seq: 1,
                }),
            })
            .await
            .expect_err("conflict");
        assert!(matches!(different_seq, StoreError::Closed { .. }));
    });
}

#[test]
fn given_a_three_level_fork_chain_when_both_parents_are_deleted_then_the_chain_still_reads() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let level0 = name("chain-l0");
        let level1 = name("chain-l1");
        let level2 = name("chain-l2");
        create_text(&streams, &level0).await;
        streams
            .stream(level0.clone())
            .append(AppendRequest::new(Chunk::new(b"A".to_vec())))
            .await
            .expect("append A");
        for (child, source) in [(&level1, &level0)] {
            streams
                .stream(child.clone())
                .create(CreateSpec {
                    fork: Some(ForkSpec {
                        source: source.clone(),
                        at: None,
                        sub_offset: 0,
                    }),
                    ..CreateSpec::default()
                })
                .await
                .expect("fork");
        }
        // Append before the next fork, so the deeper fork inherits it.
        streams
            .stream(level1.clone())
            .append(AppendRequest::new(Chunk::new(b"B".to_vec())))
            .await
            .expect("append B");
        streams
            .stream(level2.clone())
            .create(CreateSpec {
                fork: Some(ForkSpec {
                    source: level1.clone(),
                    at: None,
                    sub_offset: 0,
                }),
                ..CreateSpec::default()
            })
            .await
            .expect("fork level2");
        streams
            .stream(level2.clone())
            .append(AppendRequest::new(Chunk::new(b"C".to_vec())))
            .await
            .expect("append C");

        streams
            .stream(level0.clone())
            .delete()
            .await
            .expect("delete l0");
        streams
            .stream(level1.clone())
            .delete()
            .await
            .expect("delete l1");

        assert!(matches!(
            streams.stream(level0.clone()).head().await,
            Err(StoreError::Gone)
        ));
        assert_eq!(read_all(&streams, &level2).await, b"ABC");

        streams
            .stream(level2)
            .delete()
            .await
            .expect("delete l2");
        assert!(matches!(
            streams.stream(level1).head().await,
            Err(StoreError::NotFound)
        ));
        assert!(matches!(
            streams.stream(level0).head().await,
            Err(StoreError::NotFound)
        ));
    });
}

#[test]
fn given_a_create_with_a_body_when_it_runs_twice_then_the_body_is_stored_once() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let stream = name("create-once");
        let spec = CreateSpec {
            content_type: Some(ContentType::parse("text/plain").expect("type")),
            initial: Some(Chunk::new(b"abc".to_vec())),
            ..CreateSpec::default()
        };
        let first = streams
            .stream(stream.clone())
            .create(spec.clone())
            .await
            .expect("create");
        assert!(first.created);

        let second = streams
            .stream(stream.clone())
            .create(spec)
            .await
            .expect("repeat");
        assert!(!second.created);

        assert_eq!(read_all(&streams, &stream).await, b"abc");
    });
}

//! Property and model-oracle tests for append and read.

use foundation_core::durable::{
    AppendRequest, Chunk, CreateSpec, DurableStream, DurableStreams, MemoryVfs, ReadFrom,
    ReadLimit, ReadRequest, StreamName,
};
use proptest::prelude::*;
use std::sync::Arc;

proptest! {
    #[test]
    fn append_then_read_is_byte_exact(
        chunks in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..64), 1..20),
    ) {
        let (bytes, model) = smol::block_on(async {
            let streams = DurableStreams::new(
                DurableStream::new(Arc::new(MemoryVfs::new())),
            );
            let name = StreamName::parse("events/property").expect("valid name");
            let stream = streams.stream(name);

            stream.create(CreateSpec::default()).await.expect("create");

            let mut model = Vec::new();
            for chunk in &chunks {
                stream
                    .append(AppendRequest::new(Chunk::new(chunk.clone())))
                    .await
                    .expect("append");
                model.extend_from_slice(chunk);
            }

            let page = stream
                .read(ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::default(),
                })
                .await
                .expect("read");
            let bytes: Vec<u8> = page
                .chunks
                .iter()
                .flat_map(|chunk| chunk.as_bytes().iter().copied())
                .collect();

            (bytes, model)
        });

        prop_assert_eq!(bytes, model);
    }
}

proptest! {
    #[test]
    fn producer_retries_store_each_chunk_once(
        chunks in prop::collection::vec(prop::collection::vec(any::<u8>(), 1..24), 1..10),
    ) {
        let bytes = smol::block_on(async {
            let streams = DurableStreams::new(
                DurableStream::new(Arc::new(MemoryVfs::new())),
            );
            let stream = streams.stream(StreamName::parse("events/producer").expect("name"));
            stream.create(CreateSpec::default()).await.expect("create");

            let producer_id = foundation_core::durable::ProducerId::parse("p").expect("id");
            let mut model = Vec::new();
            for (seq, chunk) in chunks.iter().enumerate() {
                let request = AppendRequest {
                    chunk: Chunk::new(chunk.clone()),
                    content_type: None,
                    stream_seq: None,
                    producer: Some(foundation_core::durable::Producer {
                        id: producer_id.clone(),
                        epoch: 0,
                        seq: u64::try_from(seq).expect("seq"),
                    }),
                    close: false,
                };
                stream.append(request.clone()).await.expect("append");
                // A retry of the same sequence stores nothing new.
                stream.append(request).await.expect("retry");
                model.extend_from_slice(chunk);
            }

            let page = stream
                .read(ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::default(),
                })
                .await
                .expect("read");
            page.chunks
                .into_iter()
                .flat_map(|chunk| chunk.as_bytes().to_vec())
                .collect::<Vec<u8>>()
        });
        prop_assert_eq!(bytes, chunks.iter().flatten().copied().collect::<Vec<u8>>());
    }
}

proptest! {
    #[test]
    fn fork_reads_the_prefix_at_its_offset(
        first in prop::collection::vec(any::<u8>(), 1..32),
        second in prop::collection::vec(any::<u8>(), 1..32),
        third in prop::collection::vec(any::<u8>(), 1..32),
    ) {
        let (fork_bytes, source_bytes) = smol::block_on(async {
            let streams = DurableStreams::new(
                DurableStream::new(Arc::new(MemoryVfs::new())),
            );
            let source = streams.stream(StreamName::parse("fork/source").expect("name"));
            source.create(CreateSpec::default()).await.expect("create");
            let after_first = source
                .append(AppendRequest::new(Chunk::new(first.clone())))
                .await
                .expect("append first")
                .cursor
                .next;
            source
                .append(AppendRequest::new(Chunk::new(second.clone())))
                .await
                .expect("append second");

            let fork = streams.stream(StreamName::parse("fork/child").expect("name"));
            fork.create(CreateSpec {
                fork: Some(foundation_core::durable::ForkSpec {
                    source: StreamName::parse("fork/source").expect("source"),
                    at: Some(after_first),
                    sub_offset: 0,
                }),
                ..CreateSpec::default()
            })
            .await
            .expect("fork");

            // A source append after the fork point is invisible to the fork.
            source
                .append(AppendRequest::new(Chunk::new(third.clone())))
                .await
                .expect("append third");
            fork.append(AppendRequest::new(Chunk::new(b"!".to_vec())))
                .await
                .expect("fork append");

            let fork_page = fork
                .read(ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::default(),
                })
                .await
                .expect("fork read");
            let source_page = source
                .read(ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::default(),
                })
                .await
                .expect("source read");
            let bytes = |page: foundation_core::durable::ReadPage| {
                page.chunks
                    .into_iter()
                    .flat_map(|chunk| chunk.as_bytes().to_vec())
                    .collect::<Vec<u8>>()
            };
            (bytes(fork_page), bytes(source_page))
        });

        let mut expected_fork = first.clone();
        expected_fork.push(b'!');
        prop_assert_eq!(fork_bytes, expected_fork);

        let mut expected_source = first;
        expected_source.extend_from_slice(&second);
        expected_source.extend_from_slice(&third);
        prop_assert_eq!(source_bytes, expected_source);
    }
}

proptest! {
    #[test]
    fn json_append_then_read_returns_each_message(
        messages in prop::collection::vec((any::<i64>(), "[a-z]{0,8}"), 1..12),
    ) {
        let read_back = smol::block_on(async {
            let streams = DurableStreams::new(
                DurableStream::new(Arc::new(MemoryVfs::new())),
            );
            let stream = streams.stream(StreamName::parse("json/property").expect("name"));
            stream
                .create(CreateSpec {
                    content_type: Some(
                        foundation_core::durable::ContentType::parse("application/json")
                            .expect("type"),
                    ),
                    ..CreateSpec::default()
                })
                .await
                .expect("create");

            for (number, text) in &messages {
                let value = serde_json::json!({ "n": number, "s": text });
                stream
                    .append(AppendRequest {
                        chunk: Chunk::new(serde_json::to_vec(&value).expect("json")),
                        content_type: None,
                        stream_seq: None,
                        producer: None,
                        close: false,
                    })
                    .await
                    .expect("append");
            }

            let page = stream
                .read(ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::default(),
                })
                .await
                .expect("read");
            page.chunks
                .into_iter()
                .map(|chunk| serde_json::from_slice::<serde_json::Value>(chunk.as_bytes()).expect("json"))
                .collect::<Vec<_>>()
        });

        let expected: Vec<serde_json::Value> = messages
            .iter()
            .map(|(number, text)| serde_json::json!({ "n": number, "s": text }))
            .collect();
        prop_assert_eq!(read_back, expected);
    }
}

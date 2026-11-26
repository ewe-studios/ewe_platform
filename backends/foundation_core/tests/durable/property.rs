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

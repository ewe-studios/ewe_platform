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

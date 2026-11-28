//! BDD scenarios for the business API and the memory VFS.

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, Chunk, CloseRequest, CreateSpec, DurableStream, DurableStreams, MemoryVfs,
    ReadFrom, ReadLimit, ReadRequest, StoreError, StreamName,
};
use std::sync::Arc;

fn streams() -> DurableStreams<DurableStream> {
    DurableStreams::new(DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host()))
}

#[test]
fn given_an_empty_driver_when_the_business_api_appends_then_a_reader_sees_the_bytes() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("events/abc").expect("valid name");
        let stream = streams.stream(name);

        stream.create(CreateSpec::default()).await.expect("create");
        stream
            .append(AppendRequest::new(Chunk::new(b"hello".to_vec())))
            .await
            .expect("append");

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

        assert_eq!(bytes, b"hello");
    });
}

#[test]
fn given_a_closed_stream_when_the_business_api_appends_then_it_is_refused() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = streams();
        let name = StreamName::parse("events/closed").expect("valid name");
        let stream = streams.stream(name);

        stream.create(CreateSpec::default()).await.expect("create");
        stream
            .close(CloseRequest {
                final_chunk: None,
                producer: None,
            })
            .await
            .expect("close");

        let error = stream
            .append(AppendRequest::new(Chunk::new(b"late".to_vec())))
            .await
            .expect_err("closed stream refuses an append");

        assert!(matches!(error, StoreError::Closed { .. }));
    });
}

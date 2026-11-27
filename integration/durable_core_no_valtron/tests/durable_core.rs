//! The durable protocol core works without the valtron task engine.
//!
//! This crate depends on `foundation_core` without the `valtron` feature. Its
//! successful build proves that `foundation_core::durable` has no valtron
//! type, trait or executor. The test then uses the public business API.

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, Chunk, CreateSpec, DurableStream, DurableStreams, MemoryVfs, ReadFrom,
    ReadLimit, ReadRequest, StreamName,
};
use std::sync::Arc;

fn streams() -> DurableStreams<DurableStream> {
    DurableStreams::new(DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host()))
}

#[test]
fn given_a_memory_driver_when_create_append_and_read_run_then_the_bytes_match() {
    futures_lite::future::block_on(async {
        let streams = streams();
        let name = StreamName::parse("events/no-valtron").expect("valid name");
        let stream = streams.stream(name);

        stream.create(CreateSpec::default()).await.expect("create");
        stream
            .append(AppendRequest::new(Chunk::new(
                b"hello without valtron".to_vec(),
            )))
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

        assert_eq!(bytes, b"hello without valtron");
        assert!(page.cursor.up_to_date, "the page is at the stream tail");
    });
}

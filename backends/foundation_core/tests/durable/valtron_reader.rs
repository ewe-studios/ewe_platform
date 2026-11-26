//! Tests for the valtron reader wrapper.

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, Chunk, CloseRequest, CreateSpec, DurableStream, DurableStreams, MemoryVfs,
    ReadFrom, StreamName,
};
use foundation_core::valtron::durable::DurableStreamReader;
use foundation_core::valtron::{valtron_test, Stream, StreamSpread};
use std::sync::Arc;

#[valtron_test(timeout = 5000)]
fn given_a_closed_stream_when_the_valtron_reader_catches_up_then_it_yields_the_chunks() {
    let ctx = Ctx::host();
    let stream = DurableStream::new(Arc::new(MemoryVfs::new()));
    let name = StreamName::parse("reader/valtron").expect("valid name");

    smol::block_on(async {
        let streams = DurableStreams::new(stream.clone());
        let handle = streams.stream(name.clone());
        handle.create(CreateSpec::default()).await.expect("create");
        handle
            .append(AppendRequest::new(Chunk::new(b"hello".to_vec())))
            .await
            .expect("append");
        handle
            .close(CloseRequest {
                final_chunk: None,
                producer: None,
            })
            .await
            .expect("close");
    });

    let mut reader = DurableStreamReader::new(stream, name, ReadFrom::Start, false, ctx);
    let mut bytes = Vec::new();
    let mut steps = 0_usize;

    while let Some(item) = reader.next() {
        steps += 1;
        assert!(steps < 100, "reader must make progress");

        match item {
            Stream::Next(chunk) => bytes.extend_from_slice(chunk.as_bytes()),
            Stream::Spread(items) => {
                for item in items {
                    if let StreamSpread::Done(chunk) = item {
                        bytes.extend_from_slice(chunk.as_bytes());
                    }
                }
            }
            Stream::Pending(_)
            | Stream::Wait
            | Stream::Ignore
            | Stream::Init
            | Stream::Delayed(_) => {}
        }
    }

    assert_eq!(bytes, b"hello");
}

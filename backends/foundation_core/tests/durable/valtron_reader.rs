//! Tests for the valtron reader wrapper.

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, Chunk, CloseRequest, CreateSpec, DurableStream, DurableStreams, MemoryVfs,
    ReadFrom, StreamName,
};
use foundation_core::valtron::durable::DurableStreamReader;
use foundation_core::valtron::{
    block_on_future, valtron_test, Stream, StreamExt, StreamIteratorExt, StreamSpread,
};
use std::sync::Arc;

#[valtron_test(timeout = 5000)]
fn given_a_live_reader_when_data_appends_then_it_yields_the_data() {
    let ctx = Ctx::host();
    let stream = DurableStream::new(Arc::new(MemoryVfs::new()));
    let name = StreamName::parse("reader/live").expect("valid name");

    smol::block_on(async {
        let streams = DurableStreams::new(stream.clone());
        streams
            .stream(name.clone())
            .create(CreateSpec::default())
            .await
            .expect("create");
    });

    let mut reader =
        DurableStreamReader::new(stream.clone(), name.clone(), ReadFrom::Tail, true, ctx);

    let mut steps = 0_usize;
    loop {
        steps += 1;
        assert!(steps < 100, "reader must reach the live tail");
        match reader.next() {
            Some(Stream::Wait) => break,
            Some(Stream::Next(_)) | Some(Stream::Spread(_)) => panic!("unexpected data"),
            Some(Stream::Pending(_) | Stream::Ignore | Stream::Init | Stream::Delayed(_)) => {}
            None => panic!("live reader ended before data"),
        }
    }

    smol::block_on(async {
        let streams = DurableStreams::new(stream.clone());
        streams
            .stream(name.clone())
            .append(AppendRequest::new(Chunk::new(b"live".to_vec())))
            .await
            .expect("append");
    });

    let mut bytes = Vec::new();
    steps = 0;
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
            Stream::Wait
            | Stream::Pending(_)
            | Stream::Ignore
            | Stream::Init
            | Stream::Delayed(_) => {}
        }

        if !bytes.is_empty() {
            break;
        }
    }

    assert_eq!(bytes, b"live");
}

#[valtron_test(timeout = 5000)]
fn given_a_reader_when_it_is_bridged_with_into_next_stream_then_it_yields_chunks() {
    let ctx = Ctx::host();
    let stream = DurableStream::new(Arc::new(MemoryVfs::new()));
    let name = StreamName::parse("reader/bridge").expect("valid name");

    smol::block_on(async {
        let streams = DurableStreams::new(stream.clone());
        let handle = streams.stream(name.clone());
        handle.create(CreateSpec::default()).await.expect("create");
        handle
            .append(AppendRequest::new(Chunk::new(b"bridge".to_vec())))
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

    let mut bridge =
        DurableStreamReader::new(stream, name, ReadFrom::Start, false, ctx).into_next_stream();
    let bytes = block_on_future(async move {
        let mut bytes = Vec::new();
        while let Some(chunk) = bridge.next().await {
            bytes.extend_from_slice(chunk.as_bytes());
        }
        bytes
    });

    assert_eq!(bytes, b"bridge");
}

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

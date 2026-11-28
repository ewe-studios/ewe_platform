//! Runtime matrix: the protocol core runs in a local embedded process.
//!
//! These tests drive the core with direct async calls. They name no HTTP
//! adapter, socket or server. The other runtime rows (WebAssembly (wasm),
//! Cloudflare Worker, Durable Object (DO), WebSocket and the `foundation_ai`
//! Session) are recorded in
//! `specifications/completed/71-durable-streams-in-valtron/features/08-conformance/report.md`.

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, Chunk, CloseRequest, CreateSpec, DurableService, DurableStream, DurableStreams,
    MemoryVfs, ReadFrom, ReadLimit, ReadPage, ReadRequest, StreamName,
};
use std::sync::Arc;

/// Build one core over the memory virtual file system (VFS).
fn stream() -> DurableStream {
    DurableStream::new(Arc::new(MemoryVfs::new()), &Ctx::host())
}

/// Read the bytes of one page.
fn bytes(page: &ReadPage) -> Vec<u8> {
    page.chunks
        .iter()
        .flat_map(|chunk| chunk.as_bytes().iter().copied())
        .collect()
}

/// The core runs a full create, append, read and close sequence with no transport.
#[test]
fn embedded_core_roundtrip() {
    foundation_core::valtron::block_on_future_scoped(async {
        let streams = DurableStreams::new(stream());
        let name = StreamName::parse("runtime/embedded").expect("valid name");
        let handle = streams.stream(name);

        handle.create(CreateSpec::default()).await.expect("create");
        handle
            .append(AppendRequest::new(Chunk::new(b"first".to_vec())))
            .await
            .expect("append one");
        handle
            .append(AppendRequest::new(Chunk::new(b"second".to_vec())))
            .await
            .expect("append two");

        let page = handle
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("read");
        assert_eq!(bytes(&page), b"firstsecond");
        assert!(page.cursor.up_to_date);

        handle
            .close(CloseRequest {
                final_chunk: None,
                producer: None,
            })
            .await
            .expect("close");

        let closed = handle
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect("read after close");
        assert!(closed.cursor.closed);
    });
}

/// `DurableService` serves the protocol with no HTTP adapter in the path.
#[test]
fn service_runs_without_http() {
    foundation_core::valtron::block_on_future_scoped(async {
        let service = DurableService::new(stream());
        let name = StreamName::parse("runtime/service").expect("valid name");

        service
            .create(name.clone(), CreateSpec::default())
            .await
            .expect("create");
        service
            .append(
                &name,
                AppendRequest::new(Chunk::new(b"no transport".to_vec())),
            )
            .await
            .expect("append");

        let page = service
            .read(
                &name,
                ReadRequest {
                    from: ReadFrom::Start,
                    limit: ReadLimit::default(),
                },
            )
            .await
            .expect("read");
        assert_eq!(bytes(&page), b"no transport");
    });
}

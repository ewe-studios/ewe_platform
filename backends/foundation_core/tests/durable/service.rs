//! Tests for the transport-neutral service.

use foundation_core::durable::{
    AppendRequest, Chunk, CreateSpec, DurableLimits, DurableService, DurableStream, MemoryVfs,
    ReadFrom, ReadLimit, ReadRequest, StreamName,
};
use std::sync::Arc;

#[test]
fn given_a_service_when_the_business_api_runs_then_it_uses_the_driver() {
    smol::block_on(async {
        let service = DurableService::with_limits(
            DurableStream::new(Arc::new(MemoryVfs::new())),
            DurableLimits::default(),
        );
        let name = StreamName::parse("service/basic").expect("valid");

        service
            .create(name.clone(), CreateSpec::default())
            .await
            .expect("create");
        service
            .append(&name, AppendRequest::new(Chunk::new(b"service".to_vec())))
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
        let bytes: Vec<u8> = page
            .chunks
            .iter()
            .flat_map(|chunk| chunk.as_bytes().iter().copied())
            .collect();

        assert_eq!(bytes, b"service");
        assert_eq!(service.limits().page_bytes, 1024 * 1024);
    });
}

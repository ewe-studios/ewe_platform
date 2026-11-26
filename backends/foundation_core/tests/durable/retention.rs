//! BDD scenarios for retention trim.

use foundation_core::durable::{
    AppendRequest, Chunk, CreateSpec, DurableStream, DurableStreamVfs, DurableStreams, MemoryVfs,
    ReadFrom, ReadLimit, ReadRequest, StoreError, StreamName, StreamOffset, VfsOffset,
};
use std::sync::Arc;

#[test]
fn given_a_trimmed_stream_when_read_before_the_trim_then_it_is_refused() {
    smol::block_on(async {
        let vfs = Arc::new(MemoryVfs::new());
        let streams = DurableStreams::new(DurableStream::new(vfs.clone()));
        let name = StreamName::parse("retention/trim").expect("valid");

        streams
            .stream(name.clone())
            .create(CreateSpec::default())
            .await
            .expect("create");
        streams
            .stream(name.clone())
            .append(AppendRequest::new(Chunk::new(b"abcde".to_vec())))
            .await
            .expect("append");
        vfs.trim(&name, VfsOffset(2)).await.expect("trim");

        let error = streams
            .stream(name.clone())
            .read(ReadRequest {
                from: ReadFrom::Start,
                limit: ReadLimit::default(),
            })
            .await
            .expect_err("trimmed offset");

        assert!(matches!(error, StoreError::Trimmed));

        let page = streams
            .stream(name)
            .read(ReadRequest {
                from: ReadFrom::Offset(StreamOffset::from_bytes(2_u64.to_be_bytes())),
                limit: ReadLimit::default(),
            })
            .await
            .expect("read after trim");
        let bytes: Vec<u8> = page
            .chunks
            .iter()
            .flat_map(|chunk| chunk.as_bytes().iter().copied())
            .collect();

        assert_eq!(bytes, b"cde");
    });
}

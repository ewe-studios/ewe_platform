//! IPC streaming registry tests moved out of `src/ipc/streaming.rs`
//! (spec 70 feature 59, rule 7). Every item the tests use is public.

use foundation_platform::ipc::streaming::{
    IpcStream, IpcStreamChunk, IpcStreamReceiver, PlatformStreamRegistry, StreamingIpc,
};
use foundation_platform::PlatformSession;
use foundation_wasm::ipc::{Ipc, IpcContentType, IpcError, IpcKind, IpcRequest, IpcResponse};

struct TestStreamIpc;

impl Ipc for TestStreamIpc {
    fn name(&self) -> &'static str {
        "test_stream"
    }
    fn kind(&self) -> IpcKind {
        IpcKind::Query
    }
    fn invoke(&self, _: &IpcRequest<Vec<u8>>) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Err(IpcError::ExecutionFailed("use stream()".into()))
    }
}

impl StreamingIpc for TestStreamIpc {
    fn stream(&self, _: &PlatformSession, _: &IpcRequest<Vec<u8>>) -> Result<IpcStream, IpcError> {
        let queue = concurrent_queue::ConcurrentQueue::unbounded();
        let _ = queue.push(Ok(IpcStreamChunk::new(b"chunk1".to_vec(), 0)));
        let _ = queue.push(Ok(IpcStreamChunk::new(b"chunk2".to_vec(), 1)));
        queue.close();
        Ok(IpcStream {
            receiver: IpcStreamReceiver::Sync(queue),
            total_hint: None,
        })
    }

    fn accept_stream(
        &self,
        _: &PlatformSession,
        _: &IpcRequest<Vec<u8>>,
        _: IpcStreamReceiver,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError> {
        Ok(IpcResponse {
            payload: b"accepted".to_vec(),
            content_type: IpcContentType::Json,
        })
    }
}

#[test]
fn registry_register_and_get() {
    let reg = PlatformStreamRegistry::new();
    reg.register(TestStreamIpc);
    assert!(reg.get("test_stream").is_some());
    assert!(reg.get("nonexistent").is_none());
    assert_eq!(reg.names(), vec!["test_stream"]);
}

#[test]
fn stream_yields_chunks_in_order() {
    let reg = PlatformStreamRegistry::new();
    reg.register(TestStreamIpc);
    let session = PlatformSession::new_test(std::path::PathBuf::from("."));

    let handler = reg.get("test_stream").unwrap();
    let ipc_stream = handler
        .stream(
            &session,
            &IpcRequest {
                ipc: "test_stream".into(),
                action: "read".into(),
                payload: vec![],
                content_type: IpcContentType::Json,
                target: None,
            },
        )
        .unwrap();

    match ipc_stream.receiver {
        IpcStreamReceiver::Sync(ref queue) => {
            let c1 = queue.pop().unwrap().unwrap();
            assert_eq!(c1.data, b"chunk1");
            assert_eq!(c1.sequence, 0);

            let c2 = queue.pop().unwrap().unwrap();
            assert_eq!(c2.data, b"chunk2");
            assert_eq!(c2.sequence, 1);

            assert!(queue.pop().is_err());
        }
    }
}

//! Streaming IPC — bidirectional chunk delivery over Tauri Channels (F26).
//!
//! WHY: The `Ipc` trait (F25) is request/response. Streaming needs a different
//! contract: multiple chunks over time, bidirectional flow, and backpressure.
//! `StreamingIpc` extends `Ipc` with `stream()` and `accept_stream()`.
//!
//! WHAT: `StreamingIpc` trait, `IpcStream`, `IpcStreamReceiver`, `IpcStreamChunk`,
//! `PlatformStreamRegistry`.
//!
//! HOW: `stream()` returns an `IpcStreamReceiver` that the platform drains into
//! a Tauri Channel. Tauri handles delivery (eval for small chunks, fetch for
//! large ones). End-of-stream is signaled by Tauri Channel drop (`{ end: true }`).

use std::collections::HashMap;
use std::sync::RwLock;

use foundation_wasm::ipc::{Ipc, IpcError, IpcRequest, IpcResponse};

use crate::session::PlatformSession;

// ── StreamingIpc trait ────────────────────────────────────────────────

/// A streaming IPC — extends [`Ipc`] (F25) with bidirectional chunk delivery.
///
/// NOT an `IpcKind` variant. `Ipc::kind()` returns `IpcKind::Query` for
/// streaming IPCs — streaming is the extended trait, not the kind.
pub trait StreamingIpc: Ipc {
    /// Server → client: emit a stream of chunks to the frontend.
    ///
    /// Called once per invocation. The platform drains the returned
    /// `IpcStreamReceiver` into the Tauri Channel, delivering each
    /// chunk to JS via `onChunk`. End-of-stream is signaled when
    /// the Tauri Channel drops (sends `{ end: true }`).
    fn stream(
        &self,
        session: &PlatformSession,
        request: &IpcRequest,
    ) -> Result<IpcStream, IpcError>;

    /// Client → server: accept a stream of chunks from the frontend.
    ///
    /// The platform collects chunks from JS into an `IpcStreamReceiver`,
    /// then calls this method to process them. Returns a single `IpcResponse`
    /// when the input stream is complete.
    fn accept_stream(
        &self,
        session: &PlatformSession,
        request: &IpcRequest,
        input: IpcStreamReceiver,
    ) -> Result<IpcResponse, IpcError>;
}

// ── Stream types ───────────────────────────────────────────────────────

/// A stream of chunks in either direction. Platform-agnostic.
pub struct IpcStream {
    pub receiver: IpcStreamReceiver,
    pub total_hint: Option<u64>,
}

/// Receiver half for streaming chunks.
///
/// Uses `concurrent_queue::ConcurrentQueue` — lock-free, wasm32-safe,
/// already a workspace dependency (used in foundation_core, foundation_ai).
pub enum IpcStreamReceiver {
    Sync(concurrent_queue::ConcurrentQueue<Result<IpcStreamChunk, IpcError>>),
}

/// A single chunk in a stream.
///
/// Does NOT have `is_last` — Tauri's Channel drop signals end-of-stream
/// via `{ end: true }` to JS. The drop happens when the Tauri command
/// that owns the Channel returns.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IpcStreamChunk {
    /// Chunk payload bytes.
    pub data: Vec<u8>,
    /// Monotonically increasing sequence number.
    pub sequence: u64,
    /// Optional progress: bytes sent so far (for progress bars).
    pub progress: Option<u64>,
}

impl IpcStreamChunk {
    /// Create a new chunk.
    #[must_use]
    pub fn new(data: Vec<u8>, sequence: u64) -> Self {
        Self { data, sequence, progress: None }
    }

    /// Create a chunk with progress info.
    #[must_use]
    pub fn with_progress(data: Vec<u8>, sequence: u64, progress: u64) -> Self {
        Self { data, sequence, progress: Some(progress) }
    }
}

// ── PlatformStreamRegistry ────────────────────────────────────────────

/// Registry of streaming IPC handlers. Separate from `IpcRegistry` (F25)
/// because streaming handlers need a different lookup that provides access
/// to the `StreamingIpc` trait methods.
pub struct PlatformStreamRegistry {
    handlers: RwLock<HashMap<String, Box<dyn StreamingIpc>>>,
}

impl PlatformStreamRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self { handlers: RwLock::new(HashMap::new()) }
    }

    /// Register a streaming IPC handler.
    pub fn register<S: StreamingIpc + 'static>(&self, ipc: S) -> Option<Box<dyn StreamingIpc>> {
        self.handlers.write().unwrap().insert(ipc.name().to_string(), Box::new(ipc))
    }

    /// Look up a streaming IPC by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn StreamingIpc> {
        let guard = self.handlers.read().unwrap();
        guard.get(name).map(|b| {
            unsafe { &*(b.as_ref() as *const dyn StreamingIpc) }
        })
    }

    /// Return all registered streaming IPC names.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.handlers.read().unwrap().keys().cloned().collect()
    }
}

impl Default for PlatformStreamRegistry {
    fn default() -> Self { Self::new() }
}

// ── Tauri command: __ewe_ipc_stream ──────────────────────────────────

// (Tauri command __ewe_ipc_stream is defined in builder.rs next to __ewe_ipc.)
// The #[tauri::command] proc macro generates macros in the defining module's
// scope — builder.rs needs them for generate_handler!.

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use foundation_wasm::ipc::{IpcContentType, IpcKind};

    struct TestStreamIpc;

    impl Ipc for TestStreamIpc {
        fn name(&self) -> &str { "test_stream" }
        fn kind(&self) -> IpcKind { IpcKind::Query }
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
            Ok(IpcStream { receiver: IpcStreamReceiver::Sync(queue), total_hint: None })
        }

        fn accept_stream(&self, _: &PlatformSession, _: &IpcRequest<Vec<u8>>, _: IpcStreamReceiver) -> Result<IpcResponse<Vec<u8>>, IpcError> {
            Ok(IpcResponse { payload: b"accepted".to_vec(), content_type: IpcContentType::Json })
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
        let ipc_stream = handler.stream(&session, &IpcRequest {
            ipc: "test_stream".into(),
            action: "read".into(),
            payload: vec![],
            content_type: IpcContentType::Json,
            target: None,
        }).unwrap();

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
}

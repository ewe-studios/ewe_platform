//! Protocol fault-injection oracle.
//!
//! A VFS wrapper fails one chosen operation and returns a typed [`VfsError`]. The test proves
//! that the typed error reaches [`DurableStream`] and that the failed operation leaves no
//! partial state behind.

use foundation_context::Ctx;
use foundation_core::durable::{
    AppendRequest, Chunk, CloseRequest, CreateSpec, DurableStream, DurableStreamVfs,
    DurableStreams, ForkSpec, MemoryVfs, NotifierRef, ReadFrom, ReadLimit, ReadRequest, StoreError,
    StreamName, VfsEntry, VfsError, VfsFuture, VfsMeta, VfsOffset,
};
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

/// One operation the wrapper can make fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FaultOp {
    /// An append of new bytes.
    Append,
    /// A read.
    Read,
    /// A `HEAD` metadata read.
    Head,
    /// A close.
    Close,
    /// A fork.
    Fork,
}

impl FaultOp {
    fn as_str(self) -> &'static str {
        match self {
            Self::Append => "append",
            Self::Read => "read",
            Self::Head => "head",
            Self::Close => "close",
            Self::Fork => "fork",
        }
    }
}

/// The typed error the wrapper injects.
#[derive(Debug)]
struct InjectedFault(FaultOp);

impl fmt::Display for InjectedFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "injected {} fault", self.0.as_str())
    }
}

impl core::error::Error for InjectedFault {}

fn fault<T: Send + 'static>(op: FaultOp) -> VfsFuture<'static, T> {
    Box::pin(async move { Err(VfsError::Io(Box::new(InjectedFault(op)))) })
}

/// A [`MemoryVfs`] wrapper that fails one armed operation.
struct FaultVfs {
    inner: MemoryVfs,
    armed: Mutex<Option<FaultOp>>,
}

impl FaultVfs {
    fn new() -> Self {
        Self {
            inner: MemoryVfs::new(),
            armed: Mutex::new(None),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<FaultOp>> {
        self.armed.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn arm(&self, op: FaultOp) {
        *self.lock() = Some(op);
    }

    fn disarm(&self) {
        *self.lock() = None;
    }

    fn is_armed(&self, op: FaultOp) -> bool {
        *self.lock() == Some(op)
    }
}

impl DurableStreamVfs for FaultVfs {
    fn create(&self, name: &StreamName, meta: VfsMeta) -> VfsFuture<'_, bool> {
        // `DurableStream::fork` creates the fork target through this method.
        if self.is_armed(FaultOp::Fork) {
            return fault(FaultOp::Fork);
        }
        self.inner.create(name, meta)
    }

    fn append(&self, name: &StreamName, bytes: &[u8]) -> VfsFuture<'_, VfsOffset> {
        if self.is_armed(FaultOp::Append) {
            return fault(FaultOp::Append);
        }
        self.inner.append(name, bytes)
    }

    fn append_with_meta(
        &self,
        name: &StreamName,
        bytes: &[u8],
        meta: VfsMeta,
        expected_version: u64,
    ) -> VfsFuture<'_, VfsOffset> {
        // An append and a close both reach the VFS through this method. The
        // core sets `meta.closed` for a close, so the flag names the caller.
        let op = if meta.closed {
            FaultOp::Close
        } else {
            FaultOp::Append
        };
        if self.is_armed(op) {
            return fault(op);
        }
        self.inner
            .append_with_meta(name, bytes, meta, expected_version)
    }

    fn read_at(&self, name: &StreamName, at: &VfsOffset, limit: usize) -> VfsFuture<'_, Vec<u8>> {
        if self.is_armed(FaultOp::Read) {
            return fault(FaultOp::Read);
        }
        self.inner.read_at(name, at, limit)
    }

    fn meta(&self, name: &StreamName) -> VfsFuture<'_, VfsMeta> {
        if self.is_armed(FaultOp::Head) {
            return fault(FaultOp::Head);
        }
        self.inner.meta(name)
    }

    fn touch(&self, name: &StreamName, now_ms: i64) -> VfsFuture<'_, ()> {
        self.inner.touch(name, now_ms)
    }

    fn trim(&self, name: &StreamName, at: VfsOffset) -> VfsFuture<'_, ()> {
        self.inner.trim(name, at)
    }

    fn close(&self, name: &StreamName) -> VfsFuture<'_, ()> {
        if self.is_armed(FaultOp::Close) {
            return fault(FaultOp::Close);
        }
        self.inner.close(name)
    }

    fn remove(&self, name: &StreamName) -> VfsFuture<'_, bool> {
        self.inner.remove(name)
    }

    fn list(&self, prefix: &StreamName) -> VfsFuture<'_, Vec<VfsEntry>> {
        self.inner.list(prefix)
    }

    fn notifier(&self, name: &StreamName) -> Option<NotifierRef> {
        self.inner.notifier(name)
    }
}

/// Assert that the store surfaced the injected fault and its operation.
fn assert_injected(error: &StoreError, op: FaultOp) {
    match error {
        StoreError::Storage(inner) => {
            let fault = inner
                .downcast_ref::<InjectedFault>()
                .expect("the injected fault must survive the VFS boundary");
            assert_eq!(fault.0, op, "the wrong fault surfaced");
        }
        other => panic!("expected StoreError::Storage, got {other:?}"),
    }
}

fn name(raw: &str) -> StreamName {
    StreamName::parse(raw).expect("valid name")
}

fn request() -> ReadRequest {
    ReadRequest {
        from: ReadFrom::Start,
        limit: ReadLimit::default(),
    }
}

#[test]
fn injected_error_is_typed() {
    for op in [
        FaultOp::Append,
        FaultOp::Read,
        FaultOp::Head,
        FaultOp::Close,
        FaultOp::Fork,
    ] {
        foundation_core::valtron::block_on_future_scoped(run_case(op));
    }
}

async fn run_case(op: FaultOp) {
    let vfs = Arc::new(FaultVfs::new());
    let streams = DurableStreams::new(DurableStream::new(vfs.clone(), &Ctx::host()));
    let source = name("fault/injection");
    let stream = streams.stream(source.clone());
    stream.create(CreateSpec::default()).await.expect("create");

    match op {
        FaultOp::Append => {
            let before = stream.head().await.expect("head before");
            vfs.arm(FaultOp::Append);
            let error = stream
                .append(AppendRequest::new(Chunk::new(b"lost".to_vec())))
                .await
                .expect_err("the append must fail");
            assert_injected(&error, FaultOp::Append);
            vfs.disarm();

            let after = stream.head().await.expect("head after");
            assert_eq!(after.tail.as_bytes(), before.tail.as_bytes());
            assert!(!after.closed);
            let page = stream.read(request()).await.expect("read after");
            assert!(page.chunks.is_empty(), "a failed append left data");
        }
        FaultOp::Read => {
            stream
                .append(AppendRequest::new(Chunk::new(b"kept".to_vec())))
                .await
                .expect("append");
            vfs.arm(FaultOp::Read);
            let error = stream
                .read(request())
                .await
                .expect_err("the read must fail");
            assert_injected(&error, FaultOp::Read);
            vfs.disarm();

            let page = stream.read(request()).await.expect("read after");
            let bytes: Vec<u8> = page
                .chunks
                .iter()
                .flat_map(|chunk| chunk.as_bytes().iter().copied())
                .collect();
            assert_eq!(bytes, b"kept");
        }
        FaultOp::Head => {
            vfs.arm(FaultOp::Head);
            let error = stream.head().await.expect_err("the head must fail");
            assert_injected(&error, FaultOp::Head);
            vfs.disarm();

            let head = stream.head().await.expect("head after");
            assert_eq!(head.tail.as_bytes(), &[0; 8]);
        }
        FaultOp::Close => {
            stream
                .append(AppendRequest::new(Chunk::new(b"before close".to_vec())))
                .await
                .expect("append");
            vfs.arm(FaultOp::Close);
            let error = stream
                .close(CloseRequest {
                    final_chunk: None,
                    producer: None,
                })
                .await
                .expect_err("the close must fail");
            assert_injected(&error, FaultOp::Close);
            vfs.disarm();

            let head = stream.head().await.expect("head after");
            assert!(!head.closed, "a failed close left the stream closed");
            stream
                .append(AppendRequest::new(Chunk::new(b"after".to_vec())))
                .await
                .expect("a failed close must leave the stream open");
        }
        FaultOp::Fork => {
            stream
                .append(AppendRequest::new(Chunk::new(b"abc".to_vec())))
                .await
                .expect("append");
            let target = name("fault/injection-fork");
            vfs.arm(FaultOp::Fork);
            let error = streams
                .stream(target.clone())
                .fork(ForkSpec {
                    source: source.clone(),
                    at: None,
                    sub_offset: 0,
                })
                .await
                .expect_err("the fork must fail");
            assert_injected(&error, FaultOp::Fork);
            vfs.disarm();

            let missing = streams.stream(target).head().await;
            assert!(
                matches!(missing, Err(StoreError::NotFound)),
                "a failed fork left a target stream: {missing:?}"
            );
        }
    }
}

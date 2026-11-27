//! The `Create` response stream of the control service yields on `Wait`.
//!
//! The create progress iterator is a driving valtron stream: it runs the
//! producer inside `next()`. `Stream::Wait` means "the queue is empty; check
//! again". A synchronous loop that continues on `Wait` runs the producer again
//! at once. On a single-threaded wasm host that loop never returns to the event
//! loop, so the create stalls for ever and the Durable Object stops answering
//! (issue #1125). The service must yield to the host on `Wait`.

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::Poll;

use foundation_connectrpc::{Request, RpcCtx};
use foundation_core::valtron::Stream;
use foundation_sandbox::shared::control::service::ControlService;
use foundation_sandbox::shared::control::testing::spec;
use foundation_sandbox::shared::control::{
    ExitReport, SandboxHandle, SandboxInfo, SandboxProvider,
};
use foundation_sandbox::shared::driver::{CreateProgress, HostReport};
use foundation_sandbox::shared::error::{SandboxError, SandboxResult};
use foundation_sandbox::shared::handles::CreateStream;
use foundation_sandbox::shared::ids::SandboxId;
use foundation_sandbox::shared::limits::SandboxLimits;
use foundation_sandbox::shared::rpc::messages::{CreateEvent, CreateRequest};
use foundation_sandbox::shared::rpc::SandboxService;
use foundation_sandbox::shared::spec::SandboxSpec;
use foundation_sandbox::shared::BoxFuture;
use futures::Stream as FuturesStream;

use super::support;

/// One create iterator with a known sequence of steps.
///
/// It reports one progress value, then `Wait` (the queue is empty for now),
/// then one error result. It counts every pull, so a test can see how many
/// steps one poll of the response stream consumed.
struct ScriptedCreate {
    pulls: Arc<AtomicUsize>,
}

impl Iterator for ScriptedCreate {
    type Item = Stream<SandboxResult<SandboxHandle>, CreateProgress>;

    fn next(&mut self) -> Option<Self::Item> {
        let pull = self.pulls.fetch_add(1, Ordering::SeqCst);
        Some(match pull {
            0 => Stream::Pending(CreateProgress::Preparing),
            1 => Stream::Wait,
            _ => Stream::Next(Err(SandboxError::SlaveHandshake.into())),
        })
    }
}

/// A provider whose only real call is the progress create.
struct ScriptedProvider {
    pulls: Arc<AtomicUsize>,
}

impl ScriptedProvider {
    fn refused(name: &str) -> SandboxError {
        SandboxError::Io {
            op: name.to_string(),
            message: "the test provider does not serve this call".to_string(),
        }
    }
}

impl SandboxProvider for ScriptedProvider {
    fn probe(&self) -> BoxFuture<'_, SandboxResult<Vec<HostReport>>> {
        Box::pin(async { Err(Self::refused("probe").into()) })
    }

    fn create(&self, _spec: SandboxSpec) -> BoxFuture<'_, SandboxResult<SandboxHandle>> {
        Box::pin(async { Err(Self::refused("create").into()) })
    }

    fn create_with_progress(&self, _spec: SandboxSpec) -> CreateStream {
        CreateStream::new(Box::new(ScriptedCreate {
            pulls: Arc::clone(&self.pulls),
        }))
    }

    fn get(&self, _id: SandboxId) -> BoxFuture<'_, SandboxResult<SandboxHandle>> {
        Box::pin(async { Err(Self::refused("get").into()) })
    }

    fn list(&self) -> BoxFuture<'_, SandboxResult<Vec<SandboxInfo>>> {
        Box::pin(async { Err(Self::refused("list").into()) })
    }

    fn stop(&self, _id: SandboxId) -> BoxFuture<'_, SandboxResult<ExitReport>> {
        Box::pin(async { Err(Self::refused("stop").into()) })
    }

    fn kill(&self, _id: SandboxId) -> BoxFuture<'_, SandboxResult<ExitReport>> {
        Box::pin(async { Err(Self::refused("kill").into()) })
    }
}

/// Polls one stream one time.
fn poll_once<S: FuturesStream + Unpin>(stream: &mut S) -> Poll<Option<S::Item>> {
    let waker = futures::task::noop_waker();
    let mut context = futures::task::Context::from_waker(&waker);
    Pin::new(stream).poll_next(&mut context)
}

/// A create stream that reports `Wait` must give the host a turn.
#[test]
fn create_stream_yields_on_wait() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(80);
    let pulls = Arc::new(AtomicUsize::new(0));
    let provider: Arc<dyn SandboxProvider> = Arc::new(ScriptedProvider {
        pulls: Arc::clone(&pulls),
    });
    let service = ControlService::new(provider, SandboxLimits::default());
    let opened = support::block_on(service.create(
        RpcCtx::client(ctx),
        Request::new(CreateRequest { spec: spec() }),
    ))
    .expect("the create stream opens");
    let mut stream = Box::pin(opened);

    // The first pull gives the one progress event.
    let first = poll_once(&mut stream);
    assert!(
        matches!(
            first,
            Poll::Ready(Some(Ok(CreateEvent::Progress(CreateProgress::Preparing))))
        ),
        "the first event is the progress value"
    );

    // The next pull reaches `Wait`. The service must yield instead of pulling
    // the iterator again in the same poll: a synchronous re-pull never returns
    // to the wasm event loop, so the create cannot progress (issue #1125).
    let second = poll_once(&mut stream);
    assert!(second.is_pending(), "the stream must yield on `Wait`");
    assert_eq!(
        pulls.load(Ordering::SeqCst),
        2,
        "the stream must not pull the iterator past `Wait` in one poll"
    );

    // A later poll delivers the result that follows the wait.
    let third = poll_once(&mut stream);
    assert!(
        matches!(third, Poll::Ready(Some(Err(_)))),
        "the result after the wait arrives on the next poll"
    );
    assert_eq!(pulls.load(Ordering::SeqCst), 3);
}

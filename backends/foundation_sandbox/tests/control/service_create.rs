//! The `Create` response stream of the control service yields on `Wait`.
//!
//! The create progress iterator is a driving valtron stream: it runs the
//! producer inside `next()`. `Stream::Wait` means "the queue is empty; check
//! again". A synchronous loop that continues on `Wait` runs the producer again
//! at once. On a single-threaded wasm host that loop never returns to the event
//! loop, so the create stalls for ever and the Durable Object stops answering
//! (issue #1125). The service must yield to the host on `Wait`.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_connectrpc::{Request, RpcCtx};
use foundation_context::env::{Timer, TimerFuture};
use foundation_context::{Clock, Ctx};
use foundation_core::valtron::{SimulatedClock, Stream};
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

/// A timer that holds every wait until the test opens it.
///
/// The service parks on the injected timer when the create iterator reports
/// `Wait`. The test keeps that wait closed so it can see the park, then opens it
/// to let the create continue.
#[derive(Default)]
struct GateState {
    open: AtomicBool,
    wakers: Mutex<Vec<Waker>>,
}

/// The timer of one test, opened by [`GateTimer::release`].
#[derive(Clone, Default)]
struct GateTimer(Arc<GateState>);

impl GateTimer {
    /// Completes every held wait and every later wait at once.
    fn release(&self) {
        self.0.open.store(true, Ordering::SeqCst);
        let wakers = std::mem::take(
            &mut *self
                .0
                .wakers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for waker in wakers {
            waker.wake();
        }
    }
}

impl Timer for GateTimer {
    fn sleep_until(&self, _ctx: Ctx, _deadline: Instant) -> TimerFuture {
        let state = Arc::clone(&self.0);
        Box::pin(futures::future::poll_fn(
            move |context: &mut Context<'_>| {
                if state.open.load(Ordering::SeqCst) {
                    return Poll::Ready(());
                }
                state
                    .wakers
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(context.waker().clone());
                Poll::Pending
            },
        ))
    }
}

/// A context whose timer is the [`GateTimer`].
fn gated_ctx(seed: u64) -> (Ctx, Arc<GateTimer>) {
    support::pool();
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::with_seed(Arc::clone(&clock) as Arc<dyn Clock>, seed);
    let env = foundation_core::env::simulated(&ctx, seed).expect("the simulated Env builds");
    let gate = Arc::new(GateTimer::default());
    ctx.store(env.with_timer(Arc::clone(&gate) as Arc<dyn Timer>));
    (ctx, gate)
}

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
    let (ctx, gate) = gated_ctx(80);
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

    // The next pull reaches `Wait`. The service must park on the timer instead
    // of pulling the iterator again in the same poll: a synchronous re-pull
    // never returns to the wasm event loop, so the create cannot progress
    // (issue #1125).
    let second = poll_once(&mut stream);
    assert!(second.is_pending(), "the stream must yield on `Wait`");
    assert_eq!(
        pulls.load(Ordering::SeqCst),
        2,
        "the stream must not pull the iterator past `Wait` in one poll"
    );

    // While the timer holds, the create stays parked.
    assert!(
        poll_once(&mut stream).is_pending(),
        "the stream stays parked until the timer fires"
    );

    // The timer fires: the create continues and gives its result.
    gate.release();
    let third = poll_once(&mut stream);
    assert!(
        matches!(third, Poll::Ready(Some(Err(_)))),
        "the result after the wait arrives when the timer fires"
    );
    assert_eq!(pulls.load(Ordering::SeqCst), 3);
}

//! The readiness of the create progress stream.
//!
//! The create progress iterator is a driving valtron stream: it runs the
//! producer inside `next()`. It also carries the wake of its engine. The
//! parking adapter waits on that wake, so the stream needs no timer. These
//! tests drive the stream with no timer and count the wakes.

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use foundation_compact::SendWrapper;
use foundation_connectrpc::{Request, RpcCtx};
use foundation_core::valtron::{
    SendWakeCallback, Stream, StreamParkingFutureStream, WakeCallback, WakeResult, WakeableStream,
};
use foundation_sandbox::shared::control::service::ControlService;
use foundation_sandbox::shared::control::testing::spec;
use foundation_sandbox::shared::control::{
    ExitReport, SandboxHandle, SandboxInfo, SandboxProvider,
};
use foundation_sandbox::shared::driver::{CreateProgress, HostReport};
use foundation_sandbox::shared::error::{SandboxError, SandboxResult};
use foundation_sandbox::shared::handles::{CreateStream, NoWake};
use foundation_sandbox::shared::ids::SandboxId;
use foundation_sandbox::shared::limits::SandboxLimits;
use foundation_sandbox::shared::rpc::messages::{CreateEvent, CreateRequest};
use foundation_sandbox::shared::rpc::SandboxService;
use foundation_sandbox::shared::spec::SandboxSpec;
use foundation_sandbox::shared::BoxFuture;
use futures::Stream as FuturesStream;

use super::support;

/// A create progress iterator with two progress points and one result.
///
/// It counts the progress points it yields and the wakes that a consumer
/// registers. It reports that it drives its own producer, as the real create
/// iterator does.
struct CountingCreate {
    step: usize,
    progress_points: Arc<AtomicUsize>,
    wakes: Arc<AtomicUsize>,
}

impl Iterator for CountingCreate {
    type Item = Stream<SandboxResult<SandboxHandle>, CreateProgress>;

    fn next(&mut self) -> Option<Self::Item> {
        let step = self.step;
        self.step += 1;
        match step {
            0 => {
                self.progress_points.fetch_add(1, Ordering::SeqCst);
                Some(Stream::Pending(CreateProgress::Preparing))
            }
            1 => {
                self.progress_points.fetch_add(1, Ordering::SeqCst);
                Some(Stream::Pending(CreateProgress::SubstrateStarted))
            }
            2 => Some(Stream::Next(Err(SandboxError::SlaveHandshake.into()))),
            _ => None,
        }
    }
}

impl WakeableStream for CountingCreate {
    fn wake(&mut self, _cb: WakeCallback) -> WakeResult {
        self.wakes.fetch_add(1, Ordering::SeqCst);
        WakeResult::Successful
    }

    fn wake_send(&mut self, _cb: SendWakeCallback) -> WakeResult {
        self.wakes.fetch_add(1, Ordering::SeqCst);
        WakeResult::Successful
    }

    fn drives_its_producer(&self) -> bool {
        true
    }

    fn can_make_progress(&self) -> bool {
        true
    }
}

/// A create stream with two progress points wakes twice and gives one result.
///
/// The redrive budget is zero, so the parking adapter parks on the engine wake
/// at every progress point. The wake count is therefore exact, and no timer
/// takes part.
#[test]
fn create_stream_wakes_on_each_progress_point() {
    let progress_points = Arc::new(AtomicUsize::new(0));
    let wakes = Arc::new(AtomicUsize::new(0));
    let source = CountingCreate {
        step: 0,
        progress_points: Arc::clone(&progress_points),
        wakes: Arc::clone(&wakes),
    };
    let stream = CreateStream::new(Box::new(SendWrapper::new(source)), None);
    let mut parked = StreamParkingFutureStream::new(stream).with_wait_redrive_budget(0);
    let items = futures::executor::block_on(async move {
        let mut items = Vec::new();
        while let Some(item) = futures::StreamExt::next(&mut parked).await {
            items.push(item);
        }
        items
    });

    assert_eq!(
        progress_points.load(Ordering::SeqCst),
        2,
        "the source yields two progress points"
    );
    assert_eq!(
        wakes.load(Ordering::SeqCst),
        2,
        "the adapter wakes once for each progress point"
    );
    assert_eq!(items.len(), 1, "the create finishes with one result");
    assert!(
        matches!(items[0], Stream::Next(Err(_))),
        "the one result is the error of the create"
    );
}

/// A create stream with no wake still ends, and the adapter does not spin.
///
/// The immediate error stream of a failed start has no wake. The parking
/// adapter gives its one result and then ends. A bounded poll count proves
/// that it does not spin.
#[test]
fn stream_without_a_wake_ends_without_spinning() {
    let item: Stream<SandboxResult<SandboxHandle>, CreateProgress> =
        Stream::Next(Err(SandboxError::SlaveHandshake.into()));
    let stream = CreateStream::new(Box::new(NoWake::new(std::iter::once(item))), None);
    let mut parked = StreamParkingFutureStream::new(stream);
    let waker = futures::task::noop_waker();
    let mut context = Context::from_waker(&waker);
    let mut items = Vec::new();
    let mut polls = 0;
    loop {
        polls += 1;
        assert!(
            polls <= 8,
            "the parking adapter spun on a stream with no wake"
        );
        match Pin::new(&mut parked).poll_next(&mut context) {
            Poll::Ready(Some(item)) => items.push(item),
            Poll::Ready(None) => break,
            Poll::Pending => {}
        }
    }

    assert_eq!(
        polls, 2,
        "one pull gives the result and the next pull ends the stream"
    );
    assert_eq!(items.len(), 1, "the stream gives its one result");
    assert!(
        matches!(items[0], Stream::Next(Err(_))),
        "the result is the error of the create"
    );
}

/// A provider whose only real call is the progress create.
struct ScriptedProvider {
    stream: Mutex<Option<CreateStream>>,
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
        self.stream
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .expect("the test supplies one create stream")
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

/// The control service keeps every progress event of the create stream.
///
/// The parking adapter does not yield a raw `Stream::Pending`. The service maps
/// the progress point and the result to one step type first, so the caller
/// still receives each progress event in order, then the one result, with no
/// timer.
#[test]
fn service_create_keeps_every_progress_event() {
    let _serial = support::serial();
    let (ctx, _clock) = support::ctx(81);
    let progress_points = Arc::new(AtomicUsize::new(0));
    let wakes = Arc::new(AtomicUsize::new(0));
    let source = CountingCreate {
        step: 0,
        progress_points: Arc::clone(&progress_points),
        wakes: Arc::clone(&wakes),
    };
    let stream = CreateStream::new(Box::new(SendWrapper::new(source)), None);
    let provider: Arc<dyn SandboxProvider> = Arc::new(ScriptedProvider {
        stream: Mutex::new(Some(stream)),
    });
    let service = ControlService::new(provider, SandboxLimits::default());
    let response = support::block_on(service.create(
        RpcCtx::client(ctx),
        Request::new(CreateRequest { spec: spec() }),
    ))
    .expect("the create stream opens");
    let events = futures::executor::block_on(futures::StreamExt::collect::<Vec<_>>(response));

    assert_eq!(
        events.len(),
        3,
        "the stream gives two progress events and one result"
    );
    assert!(
        matches!(
            events[0],
            Ok(CreateEvent::Progress(CreateProgress::Preparing))
        ),
        "the first event is the first progress point"
    );
    assert!(
        matches!(
            events[1],
            Ok(CreateEvent::Progress(CreateProgress::SubstrateStarted))
        ),
        "the second event is the second progress point"
    );
    assert!(
        matches!(events[2], Err(_)),
        "the last event is the error of the create"
    );
}

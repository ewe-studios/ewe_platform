//! The readiness of the create progress stream.
//!
//! The create progress iterator is a driving valtron stream: it runs the
//! producer inside `next()`. It also carries the wake of its engine. The
//! parking adapter waits on that wake, so the stream needs no timer. These
//! tests drive the stream with no timer and count the wakes.

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use foundation_compact::SendWrapper;
use foundation_core::valtron::{
    SendWakeCallback, Stream, StreamParkingFutureStream, WakeCallback, WakeResult, WakeableStream,
};
use foundation_sandbox::shared::control::SandboxHandle;
use foundation_sandbox::shared::driver::CreateProgress;
use foundation_sandbox::shared::error::{SandboxError, SandboxResult};
use foundation_sandbox::shared::handles::{CreateStream, NoWake};
use futures::Stream as FuturesStream;

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

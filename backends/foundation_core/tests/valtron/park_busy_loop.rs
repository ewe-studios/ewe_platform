//! Regression: a task that parks forever must yield control back to the
//! caller (`Poll::Pending`), never busy-loop inside `run_until`.
//!
//! This reproduces the single-threaded symptom seen under `wrangler dev`:
//! an AgentLoop task that parks on an in-flight SSE stream (`Init` →
//! `Pending` → `Wait` / `Depends`) made `run_until` spin on
//! `CanProgress(None)` without ever yielding to JS.  The same `run_until`
//! loop drives the native single executor, so we can pin the exact park
//! behaviour here first (native, `multi` off = `single` engine) before the
//! wasm reproduction.
//!
//! A correct executor parks the task and returns `Pending` from the first
//! `poll_next`.  A busy-looping executor never returns from that poll — the
//! `#[valtron_test(timeout = ...)]` bound turns that hang into a failure.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use foundation_core::valtron::{
    execute, valtron_test, BoolSignal, Ctx, StreamIteratorExt, TaskIterator, TaskStatus,
};
use futures_core::Stream as FuturesStream;

/// A task that emits `Init`, then `Pending`, then parks on `Wait` forever —
/// the exact shape of an AgentLoop parked on a stream that never delivers.
struct ForeverWait {
    step: u8,
}

impl TaskIterator for ForeverWait {
    type Ready = u32;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let s = self.step;
        self.step = self.step.saturating_add(1);
        match s {
            0 => Some(TaskStatus::Init),
            1 => Some(TaskStatus::Pending(())),
            _ => Some(TaskStatus::Wait),
        }
    }
}

/// Same shape, but parks on `Depends` with a signal that never fires — the
/// variant the live `run_until` trace actually reported.
struct ForeverDepends {
    step: u8,
    signal: Arc<BoolSignal>,
}

impl TaskIterator for ForeverDepends {
    type Ready = u32;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let s = self.step;
        self.step = self.step.saturating_add(1);
        match s {
            0 => Some(TaskStatus::Init),
            1 => Some(TaskStatus::Pending(())),
            // Signal stays `false` forever → the task never becomes ready.
            // `Arc<BoolSignal>` coerces to `Arc<dyn EventReadiness>` at the call.
            _ => Some(TaskStatus::Depends(self.signal.clone())),
        }
    }
}

/// Poll `stream` exactly once and return the outcome.  The assertion lives in
/// the tests: `Poll::Pending` = parked correctly, `Poll::Ready` = wrong.
fn poll_once<S>(stream: &mut S) -> Poll<Option<u32>>
where
    S: FuturesStream<Item = u32> + Unpin,
{
    let waker = Waker::noop();
    let mut cx = Context::from_waker(&waker);
    Pin::new(stream).poll_next(&mut cx)
}

/// A task that parks on `Wait` forever yields `Pending` from the first poll.
#[valtron_test(threads = 1, timeout = 10_000)]
async fn forever_wait_parks_not_busy_loops() {
    let mut stream = execute(ForeverWait { step: 0 }, None)
        .expect("execute")
        .into_next_stream();

    match poll_once(&mut stream) {
        Poll::Pending => {}
        Poll::Ready(v) => panic!("parked task must yield Pending, got Ready({v:?})"),
    }
}

/// A task that parks on a never-ready `Depends` signal yields `Pending`.
#[valtron_test(threads = 1, timeout = 10_000)]
async fn forever_depends_parks_not_busy_loops() {
    let mut stream = execute(
        ForeverDepends {
            step: 0,
            signal: Arc::new(BoolSignal::new(false)),
        },
        None,
    )
    .expect("execute")
    .into_next_stream();

    match poll_once(&mut stream) {
        Poll::Pending => {}
        Poll::Ready(v) => panic!("parked task must yield Pending, got Ready({v:?})"),
    }
}

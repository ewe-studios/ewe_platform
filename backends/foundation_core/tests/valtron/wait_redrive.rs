// Tests build the effect sources on purpose (spec 84, section 6, item 2).
#![allow(clippy::disallowed_methods)]

//! Bounded re-drive for the parking stream-to-future adapters (spec 92 feature
//! 01, stages A and C).
//!
//! WHY: `Stream::Wait` means "the queue is empty; run the producer and check
//! again". `Stream::Init` and `Stream::Pending` are in-band control states too:
//! the agent loop reports progress as `Pending` between its ready records. A
//! driving stream (for example `DrivenStreamIterator`) runs the producer inside
//! `next()`. On a single-threaded engine no other thread sends the wake, so
//! parking on any of these states hangs the first turn. `Stream::Delayed` is
//! different: it asks for a timer, so the adapter still parks.
//!
//! WHAT: the adapters re-poll a driving stream on `Wait`, `Init` and `Pending`,
//! with a bounded budget, and keep the old park behaviour for a queue-backed
//! stream and for a driving stream whose engine is parked. `StreamPendingFuture`
//! returns `Pending` as its result, so it re-drives only the other states.
//!
//! HOW: fake streams prove each arm. The red evidence over the real driver
//! lives in `dst_hang_proof`.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

use foundation_core::valtron::{
    SendWakeCallback, Stream, StreamIteratorExt, WakeResult, WakeableStream,
};
use futures_core::Stream as _;
use proptest::prelude::*;

/// A waker that counts how often it is woken.
#[derive(Default)]
struct WakeCounter(AtomicUsize);

impl Wake for WakeCounter {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Clone, Default)]
struct WakeCount {
    counter: Arc<WakeCounter>,
}

impl WakeCount {
    fn count(&self) -> usize {
        self.counter.0.load(Ordering::SeqCst)
    }

    fn waker(&self) -> Waker {
        Waker::from(Arc::clone(&self.counter))
    }
}

fn lock<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ============================================================================
// wait_parks_a_non_driving_stream
// ============================================================================

/// A queue-backed stream: it reports `drives_its_producer() == false`, so a
/// `Wait` must register a producer wake and park.
struct QueueStream {
    ready: Arc<AtomicBool>,
    producer_wakes: Arc<Mutex<Vec<SendWakeCallback>>>,
    sent: bool,
}

impl Iterator for QueueStream {
    type Item = Stream<u32, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sent {
            return None;
        }
        if self.ready.load(Ordering::SeqCst) {
            self.sent = true;
            Some(Stream::Next(7))
        } else {
            Some(Stream::Wait)
        }
    }
}

impl WakeableStream for QueueStream {
    fn wake_send(&mut self, cb: SendWakeCallback) -> WakeResult {
        lock(&self.producer_wakes).push(cb);
        WakeResult::Successful
    }
}

/// A non-driving stream keeps the old register-then-recheck-then-park
/// behaviour: it returns `Pending` without a self-wake, and a producer wake
/// later completes it.
#[test]
fn wait_parks_a_non_driving_stream() {
    let ready = Arc::new(AtomicBool::new(false));
    let producer_wakes = Arc::new(Mutex::new(Vec::new()));
    let mut stream = QueueStream {
        ready: Arc::clone(&ready),
        producer_wakes: Arc::clone(&producer_wakes),
        sent: false,
    }
    .into_future_stream();
    let counter = WakeCount::default();
    let waker = counter.waker();
    let mut cx = Context::from_waker(&waker);

    assert_eq!(
        Pin::new(&mut stream).poll_next(&mut cx),
        Poll::Pending,
        "a queue-backed stream must park on Wait"
    );
    assert_eq!(
        counter.count(),
        0,
        "the adapter must not self-wake a non-driving stream"
    );
    assert_eq!(
        lock(&producer_wakes).len(),
        1,
        "the adapter must register exactly one producer wake"
    );

    // The producer fills the queue and fires the registered wake.
    ready.store(true, Ordering::SeqCst);
    let callbacks = std::mem::take(&mut *lock(&producer_wakes));
    for fire in callbacks {
        fire();
    }

    assert_eq!(
        Pin::new(&mut stream).poll_next(&mut cx),
        Poll::Ready(Some(Stream::Next(7))),
        "the producer wake must complete the parked stream"
    );
}

// ============================================================================
// wait_redrive_is_bounded
// ============================================================================

/// A driving stream that never makes progress: every `next()` returns `Wait`.
struct NeverReadyStream {
    calls: Arc<AtomicUsize>,
    producer_wakes: Arc<Mutex<Vec<SendWakeCallback>>>,
}

impl Iterator for NeverReadyStream {
    type Item = Stream<u32, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Some(Stream::Wait)
    }
}

impl WakeableStream for NeverReadyStream {
    fn wake_send(&mut self, cb: SendWakeCallback) -> WakeResult {
        lock(&self.producer_wakes).push(cb);
        WakeResult::Successful
    }

    fn drives_its_producer(&self) -> bool {
        true
    }
}

/// A driving stream that never makes progress stops after the budget: one poll
/// makes the initial call plus exactly the budget re-polls, then it fires the
/// waker and returns `Pending`.
#[test]
fn wait_redrive_is_bounded() {
    const BUDGET: usize = 5;

    let calls = Arc::new(AtomicUsize::new(0));
    let producer_wakes = Arc::new(Mutex::new(Vec::new()));
    let mut stream = NeverReadyStream {
        calls: Arc::clone(&calls),
        producer_wakes: Arc::clone(&producer_wakes),
    }
    .into_future_stream()
    .with_wait_redrive_budget(BUDGET);
    let counter = WakeCount::default();
    let waker = counter.waker();
    let mut cx = Context::from_waker(&waker);

    let poll = Pin::new(&mut stream).poll_next(&mut cx);

    assert_eq!(poll, Poll::Pending, "an exhausted budget must park");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        BUDGET + 1,
        "one poll must make the initial call plus exactly {BUDGET} re-polls"
    );
    assert_eq!(
        counter.count(),
        1,
        "the adapter must wake itself once when the budget is spent"
    );
    assert_eq!(
        lock(&producer_wakes).len(),
        1,
        "the adapter must register one fallback wake"
    );
}

// ============================================================================
// wait_redrive_resets_on_progress
// ============================================================================

/// A driving stream that yields scripted operations, then ends.
struct ScriptedStream {
    ops: VecDeque<Stream<u32, ()>>,
    calls: Arc<AtomicUsize>,
}

impl ScriptedStream {
    fn new(ops: Vec<Stream<u32, ()>>) -> (Self, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        (
            Self {
                ops: ops.into(),
                calls: Arc::clone(&calls),
            },
            calls,
        )
    }
}

impl Iterator for ScriptedStream {
    type Item = Stream<u32, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.ops.pop_front()
    }
}

impl WakeableStream for ScriptedStream {
    fn drives_its_producer(&self) -> bool {
        true
    }
}

/// `Next`, `Spread` and `Ignore` reset the budget. Two full `Wait` runs in one
/// poll therefore both re-drive, and the collector finishes in one poll.
#[test]
fn wait_redrive_resets_on_progress() {
    let (stream, calls) = ScriptedStream::new(vec![
        Stream::Wait,
        Stream::Wait,
        Stream::Next(1),
        Stream::Wait,
        Stream::Wait,
        Stream::Next(2),
    ]);
    let mut future = stream.into_collect_future().with_wait_redrive_budget(2);
    let counter = WakeCount::default();
    let waker = counter.waker();
    let mut cx = Context::from_waker(&waker);

    let poll = Pin::new(&mut future).poll(&mut cx);

    assert_eq!(
        poll,
        Poll::Ready(vec![1, 2]),
        "progress must reset the budget so both Wait runs re-drive"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        7,
        "the collector must call next() six times plus the final None"
    );
    assert_eq!(
        counter.count(),
        0,
        "the adapter must not park while it makes progress"
    );
}

// ============================================================================
// pending_and_init_redrive_a_driving_stream
// ============================================================================

/// A `Pending` item from a driving stream is a control state, not an item to
/// park on: the stream still has its ready record. One poll must drive past it.
#[test]
fn pending_redrives_a_driving_stream() {
    let (stream, calls) = ScriptedStream::new(vec![Stream::Pending(()), Stream::Next(7)]);
    let mut stream = stream.into_next_stream();
    let counter = WakeCount::default();
    let waker = counter.waker();
    let mut cx = Context::from_waker(&waker);

    let poll = Pin::new(&mut stream).poll_next(&mut cx);

    assert_eq!(
        poll,
        Poll::Ready(Some(7)),
        "a Pending item from a driving stream must be driven past in one poll"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the adapter must pull the Pending and the Next"
    );
    assert_eq!(
        counter.count(),
        0,
        "the adapter must not park while it makes progress"
    );
}

/// `StreamPendingFuture` returns `Pending` as its result. It must keep that
/// behaviour, and not re-drive it away.
#[test]
fn pending_is_the_result_of_stream_pending_future() {
    let (stream, calls) = ScriptedStream::new(vec![Stream::Pending(()), Stream::Next(7)]);
    let mut future = stream.into_pending_future();
    let counter = WakeCount::default();
    let waker = counter.waker();
    let mut cx = Context::from_waker(&waker);

    assert!(
        matches!(
            Pin::new(&mut future).poll(&mut cx),
            Poll::Ready(Some(((), _)))
        ),
        "StreamPendingFuture must return the Pending payload as its result"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "the adapter must stop at the first Pending"
    );
    assert_eq!(counter.count(), 0, "the adapter must not park");
}

/// An `Init` item from a driving stream is a control state too: the stream is
/// still starting, and its ready record follows.
#[test]
fn init_redrives_a_driving_stream() {
    let (stream, calls) = ScriptedStream::new(vec![Stream::Init, Stream::Next(7)]);
    let mut stream = stream.into_next_stream();
    let counter = WakeCount::default();
    let waker = counter.waker();
    let mut cx = Context::from_waker(&waker);

    let poll = Pin::new(&mut stream).poll_next(&mut cx);

    assert_eq!(
        poll,
        Poll::Ready(Some(7)),
        "an Init item from a driving stream must be driven past in one poll"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the adapter must pull the Init and the Next"
    );
    assert_eq!(
        counter.count(),
        0,
        "the adapter must not park while it makes progress"
    );
}

// ============================================================================
// delayed_parks_a_driving_stream
// ============================================================================

/// A driving scripted stream that can register a producer wake.
struct WakingScriptedStream {
    ops: VecDeque<Stream<u32, ()>>,
    calls: Arc<AtomicUsize>,
    producer_wakes: Arc<Mutex<Vec<SendWakeCallback>>>,
}

impl Iterator for WakingScriptedStream {
    type Item = Stream<u32, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.ops.pop_front()
    }
}

impl WakeableStream for WakingScriptedStream {
    fn wake_send(&mut self, cb: SendWakeCallback) -> WakeResult {
        lock(&self.producer_wakes).push(cb);
        WakeResult::Successful
    }

    fn drives_its_producer(&self) -> bool {
        true
    }
}

/// `Delayed` asks for a timer, so even a driving stream keeps the old
/// register-then-recheck-then-park behaviour. The budget must not be spent.
#[test]
fn delayed_parks_a_driving_stream() {
    let calls = Arc::new(AtomicUsize::new(0));
    let producer_wakes = Arc::new(Mutex::new(Vec::new()));
    let mut stream = WakingScriptedStream {
        ops: VecDeque::from(vec![
            Stream::Delayed(Duration::from_millis(10)),
            Stream::Delayed(Duration::from_millis(10)),
            Stream::Next(7),
        ]),
        calls: Arc::clone(&calls),
        producer_wakes: Arc::clone(&producer_wakes),
    }
    .into_next_stream();
    let counter = WakeCount::default();
    let waker = counter.waker();
    let mut cx = Context::from_waker(&waker);

    assert_eq!(
        Pin::new(&mut stream).poll_next(&mut cx),
        Poll::Pending,
        "a Delayed item must park"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the adapter must pull the Delayed item and recheck it once, not spend the budget"
    );
    assert_eq!(
        counter.count(),
        0,
        "the adapter must not self-wake when the stream accepts a wake"
    );
    assert_eq!(
        lock(&producer_wakes).len(),
        1,
        "the adapter must register a producer wake for the timer"
    );
}

// ============================================================================
// wait_redrive_forwards_through_a_wrapper
// ============================================================================

/// A wrapper that forwards `wake`/`wake_send` must forward both the driving
/// capability and the parked state too, or it hides the fix or the regression.
#[test]
fn wait_redrive_forwards_through_a_wrapper() {
    let (stream, _calls) = ScriptedStream::new(vec![Stream::Next(1)]);
    let wrapped = stream.map_done(|value| value);

    assert!(
        wrapped.drives_its_producer(),
        "a wrapper over a driving stream must report true"
    );
    assert!(
        wrapped.can_make_progress(),
        "a wrapper must forward can_make_progress"
    );

    let parked = ParkedDrivingStream {
        ready: Arc::new(AtomicBool::new(false)),
        producer_wakes: Arc::new(Mutex::new(Vec::new())),
        calls: Arc::new(AtomicUsize::new(0)),
        sent: false,
    };
    let wrapped = parked.map_done(|value| value);
    assert!(
        !wrapped.can_make_progress(),
        "a wrapper must forward the parked state of a driving stream"
    );
}

// ============================================================================
// wait_parks_a_parked_driving_stream
// ============================================================================

/// A driving stream whose engine is parked on a readiness signal: it reports
/// `drives_its_producer() == true` and `can_make_progress() == false` until the
/// producer is ready.
struct ParkedDrivingStream {
    ready: Arc<AtomicBool>,
    producer_wakes: Arc<Mutex<Vec<SendWakeCallback>>>,
    calls: Arc<AtomicUsize>,
    sent: bool,
}

impl Iterator for ParkedDrivingStream {
    type Item = Stream<u32, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.sent {
            return None;
        }
        if self.ready.load(Ordering::SeqCst) {
            self.sent = true;
            Some(Stream::Next(7))
        } else {
            Some(Stream::Wait)
        }
    }
}

impl WakeableStream for ParkedDrivingStream {
    fn wake_send(&mut self, cb: SendWakeCallback) -> WakeResult {
        lock(&self.producer_wakes).push(cb);
        WakeResult::Successful
    }

    fn drives_its_producer(&self) -> bool {
        true
    }

    fn can_make_progress(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }
}

/// A driving stream that is parked must park, not re-drive. A re-poll would
/// sleep out the single driver's wait budget and change nothing, so the adapter
/// uses the old register-then-recheck-then-park path.
#[test]
fn wait_parks_a_parked_driving_stream() {
    let ready = Arc::new(AtomicBool::new(false));
    let producer_wakes = Arc::new(Mutex::new(Vec::new()));
    let calls = Arc::new(AtomicUsize::new(0));
    let mut stream = ParkedDrivingStream {
        ready: Arc::clone(&ready),
        producer_wakes: Arc::clone(&producer_wakes),
        calls: Arc::clone(&calls),
        sent: false,
    }
    .into_future_stream();
    let counter = WakeCount::default();
    let waker = counter.waker();
    let mut cx = Context::from_waker(&waker);

    assert_eq!(
        Pin::new(&mut stream).poll_next(&mut cx),
        Poll::Pending,
        "a parked driving stream must park on Wait"
    );
    assert_eq!(
        counter.count(),
        0,
        "the adapter must not self-wake a parked driving stream"
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the adapter must pull once and recheck once, not re-drive the budget"
    );
    assert_eq!(
        lock(&producer_wakes).len(),
        1,
        "the adapter must register one wake"
    );

    ready.store(true, Ordering::SeqCst);
    let callbacks = std::mem::take(&mut *lock(&producer_wakes));
    for fire in callbacks {
        fire();
    }
    assert_eq!(
        Pin::new(&mut stream).poll_next(&mut cx),
        Poll::Ready(Some(Stream::Next(7))),
        "the producer wake must complete the parked driving stream"
    );
}

// ============================================================================
// wait_redrive_property
// ============================================================================

/// One scripted operation for the property test.
#[derive(Clone, Debug)]
enum Op {
    Wait,
    Ignore,
    Next(u32),
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        Just(Op::Wait),
        Just(Op::Ignore),
        (0u32..1_000).prop_map(Op::Next),
    ]
}

proptest! {
    /// Over any sequence of `Wait`, `Ignore` and `Next`, the adapter yields
    /// every `Next` payload in order and then terminates, without parking.
    #[test]
    fn wait_redrive_property(ops in prop::collection::vec(op_strategy(), 0..80)) {
        let expected: Vec<u32> = ops
            .iter()
            .filter_map(|op| match op {
                Op::Next(value) => Some(*value),
                Op::Wait | Op::Ignore => None,
            })
            .collect();
        // One poll can re-drive at most one `Wait` per operation, and each
        // `Next` ends a poll and starts a fresh budget.
        let budget = ops.len() + 1;
        let (stream, _calls) = ScriptedStream::new(Vec::from_iter(
            ops.into_iter().map(|op| match op {
                Op::Wait => Stream::Wait,
                Op::Ignore => Stream::Ignore,
                Op::Next(value) => Stream::Next(value),
            }),
        ));
        let mut stream = stream
            .into_next_stream()
            .with_wait_redrive_budget(budget);
        let counter = WakeCount::default();
        let waker = counter.waker();
        let mut cx = Context::from_waker(&waker);

        let mut got = Vec::new();
        loop {
            match Pin::new(&mut stream).poll_next(&mut cx) {
                Poll::Ready(Some(value)) => got.push(value),
                Poll::Ready(None) => break,
                Poll::Pending => prop_assert!(
                    false,
                    "the adapter parked on a driveable Wait within its budget"
                ),
            }
        }

        prop_assert_eq!(got, expected);
    }
}

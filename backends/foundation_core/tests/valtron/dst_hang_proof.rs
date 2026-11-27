// Tests build the effect sources on purpose (spec 84, section 6, item 2).
#![allow(clippy::disallowed_methods)]

//! Evidence for the deterministic simulation test (DST) turn hang (spec 92
//! feature 01, issue #908).
//!
//! WHY: on the single-threaded simulation engine a turn over the real registry
//! never finished. The `Stream::Wait` arm of the stream-to-future adapter
//! parked on a producer wake that only another thread could send.
//!
//! WHAT:
//! - [`wait_redrives_a_driving_stream`] proves the fix: a driving stream that
//!   yields `Wait` twice finishes in one poll on one thread.
//! - [`wait_arm_completes_with_a_producer_thread`] is the control: the same
//!   adapter completes when a producer thread sends the wake.
//! - [`sim_honours_depends_readiness`] probes the harness: a task parked on an
//!   injected readiness resumes when a push fires it.
//!
//! HOW: the tests run in [`SimRuntime`], the same single-threaded engine as the
//! DST. A bounded watchdog reports a hang in seconds; no test can hang the
//! continuous integration (CI) run.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

use foundation_core::valtron::sim::SimRuntime;
use foundation_core::valtron::{
    execute, EventReadinessPtr, SendWakeCallback, Stream, StreamIteratorExt, TaskIterator,
    TaskStatus, WakeCallback, WakeResult, WakeableStream,
};
use foundation_core::valtron::{Pipe, StreamNextStream};
use futures_core::Stream as _;

const START: Duration = Duration::from_secs(1_700_000_000);

/// How long a test may wait for a completion before its watchdog fails it.
const WATCHDOG: Duration = Duration::from_secs(5);

/// A waker that counts how often it is woken.
///
/// WHY: a `Wait` bug shows up as a `Pending` with no self-wake, and the count
/// reports how often the adapter woke the executor.
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

// ============================================================================
// wait_redrives_a_driving_stream
// ============================================================================

/// A task that yields `Wait` twice, then `Ready(7)`.
///
/// WHY: `execute()` wraps the task in a driving stream, whose `next()` runs the
/// single-threaded engine. The engine makes progress on each call, so a `Wait`
/// means "run the producer again", not "wait for another thread".
///
/// WHAT: the two `Wait` statuses need two more `next()` calls before the task
/// is ready.
struct WaitTwiceThenReady {
    calls: usize,
}

impl TaskIterator for WaitTwiceThenReady {
    type Ready = u32;
    type Pending = ();

    fn next_status(&mut self, _ctx: foundation_context::Ctx) -> Option<TaskStatus<u32, ()>> {
        self.calls += 1;
        match self.calls {
            1 | 2 => Some(TaskStatus::Wait),
            _ => Some(TaskStatus::Ready(7)),
        }
    }
}

/// The real driver: `execute()` wraps the task in a driving stream. One poll of
/// the parking adapter must re-drive the two `Wait` statuses and return the
/// `Next(7)` on a single-threaded engine.
#[test]
fn wait_redrives_a_driving_stream() {
    SimRuntime::run(42, START, |_sim| {
        let mut stream = execute(WaitTwiceThenReady { calls: 0 }, None)
            .expect("the engine is initialized")
            .into_future_stream();
        let counter = WakeCount::default();
        let waker = counter.waker();
        let mut cx = Context::from_waker(&waker);

        let poll = Pin::new(&mut stream).poll_next(&mut cx);

        assert!(
            matches!(poll, Poll::Ready(Some(Stream::Next(7)))),
            "a driving stream must re-drive `Wait` on one thread and finish in one poll; \
             got {poll:?} after {} self-wake(s)",
            counter.count()
        );
    });
}

// ============================================================================
// wait_arm_completes_with_a_producer_thread
// ============================================================================

/// A stream that yields `Wait` until a producer sets `ready`, then one `Next`.
struct SignalStream {
    ready: Arc<AtomicBool>,
    sent: bool,
    producer_wakes: Arc<Mutex<Vec<SendWakeCallback>>>,
}

impl Iterator for SignalStream {
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

impl WakeableStream for SignalStream {
    fn wake(&mut self, _cb: WakeCallback) -> WakeResult {
        WakeResult::NotSupported
    }

    fn wake_send(&mut self, cb: SendWakeCallback) -> WakeResult {
        self.producer_wakes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(cb);
        WakeResult::Successful
    }
}

/// CONTROL: the same adapter, the same `Wait` arm, but a producer thread fires
/// the registered wake. The future completes, so the engine (whether a
/// producer runs) is the difference, not the adapter.
#[test]
fn wait_arm_completes_with_a_producer_thread() {
    let ready = Arc::new(AtomicBool::new(false));
    let wakes = Arc::new(Mutex::new(Vec::new()));
    let stream = SignalStream {
        ready: Arc::clone(&ready),
        sent: false,
        producer_wakes: Arc::clone(&wakes),
    };

    let producer_ready = Arc::clone(&ready);
    let producer_wakes = Arc::clone(&wakes);
    let producer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        producer_ready.store(true, Ordering::SeqCst);
        let callbacks = std::mem::take(
            &mut *producer_wakes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for fire in callbacks {
            fire();
        }
    });

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        SimRuntime::run(42, START, move |sim| {
            let mut stream = stream.into_future_stream();
            let collected = sim.block_on(async move {
                let mut items = Vec::new();
                while let Some(item) =
                    std::future::poll_fn(|cx| Pin::new(&mut stream).poll_next(cx)).await
                {
                    items.push(item);
                }
                items
            });
            let _ = tx.send(collected);
        });
    });

    let collected = rx.recv_timeout(WATCHDOG).unwrap_or_else(|_| {
        panic!(
            "watchdog: wait_arm_completes_with_a_producer_thread: the producer thread fired the \
             wake, but the future did not complete within {WATCHDOG:?}"
        )
    });
    producer.join().expect("the producer thread");
    assert_eq!(
        collected,
        vec![Stream::Next(7)],
        "the producer thread must drive the Wait arm to Next"
    );
}

// ============================================================================
// sim_honours_depends_readiness
// ============================================================================

/// Parks once on an injected readiness, then yields 7.
struct ParkThenReady {
    signal: Option<EventReadinessPtr>,
}

impl TaskIterator for ParkThenReady {
    type Ready = u32;
    type Pending = ();

    fn next_status(&mut self, _ctx: foundation_context::Ctx) -> Option<TaskStatus<u32, ()>> {
        if let Some(signal) = self.signal.take() {
            return Some(TaskStatus::Depends(signal));
        }
        Some(TaskStatus::Ready(7))
    }
}

/// PROBE: a task parked on a `Depends(readiness)` must resume when a push on
/// another thread fires that readiness. If this passes, the simulation harness
/// honours readiness and the `Wait` arm is the only defect.
#[test]
fn sim_honours_depends_readiness() {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        SimRuntime::run(42, START, move |sim| {
            let (sender, receiver) = Pipe::<u32>::with_depth(4);
            let signal: EventReadinessPtr = Arc::new(receiver.readiness());

            let pusher = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                sender.try_send(7).expect("push the readiness");
            });

            let mut stream: StreamNextStream<_> = execute(
                ParkThenReady {
                    signal: Some(signal),
                },
                None,
            )
            .expect("the engine is initialized")
            .into_next_stream();

            let value = sim.block_on(async move {
                std::future::poll_fn(|cx| Pin::new(&mut stream).poll_next(cx)).await
            });
            pusher.join().expect("the pusher thread");
            let _ = tx.send(value);
        });
    });

    let value = rx.recv_timeout(WATCHDOG).unwrap_or_else(|_| {
        panic!(
            "watchdog: sim_honours_depends_readiness: the parked task did not resume within \
             {WATCHDOG:?} after a push on its readiness"
        )
    });
    assert_eq!(
        value,
        Some(7),
        "the engine must resume a task parked on Depends when a push fires it"
    );
}

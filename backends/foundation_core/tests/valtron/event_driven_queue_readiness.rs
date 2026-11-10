//! Queue readiness types are event-driven (spec 70 feature 77, E3).
//!
//! WHY: a task parked on a queue readiness was re-checked every millisecond,
//! because a push could come from code that did not wake the task. The queues of
//! valtron now wake the parked task themselves (`EventQueue`, feature 80), so
//! their readiness registers the task's waker (`register` returns `true`) and the
//! executor does not re-check it on a timer.
//!
//! WHAT: for each type, a task parks on it, a thread stores into it 200 ms
//! later, and the task runs again soon after. A counting adapter (public trait
//! only) proves the executor checked the readiness only a few times while the
//! task was parked.

#![cfg_attr(
    not(feature = "multi"),
    expect(
        clippy::arc_with_non_send_sync,
        reason = "one Arc queue serves both trees; the single-threaded task payload is not Send + Sync and never crosses a thread"
    )
)]
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{
    execute, valtron_test, EventQueue, EventReadiness, EventReadinessPtr, Pipe, ReadinessWaker,
    ResumeSignal, StreamIteratorExt, TaskIterator, TaskStatus,
};

use foundation_context::Ctx;
use futures_core::Stream as FuturesStream;

/// Counts `is_ready` calls on the readiness it wraps.
struct Counting {
    inner: EventReadinessPtr,
    checks: Arc<AtomicUsize>,
}

impl EventReadiness for Counting {
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        self.checks.fetch_add(1, Ordering::SeqCst);
        self.inner.is_ready(dur)
    }

    fn deadline(&self) -> Option<Instant> {
        self.inner.deadline()
    }

    fn clear(&self) {
        self.inner.clear();
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.inner.register(waker)
    }
}

/// Parks once on a readiness, then yields the milliseconds since creation.
struct ParkThenReport {
    created: Instant,
    signal: Option<EventReadinessPtr>,
    step: u8,
}

impl TaskIterator for ParkThenReport {
    type Ready = u64;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let step = self.step;
        self.step = self.step.saturating_add(1);
        match step {
            0 => Some(TaskStatus::Depends(self.signal.take().expect("signal"))),
            1 => Some(TaskStatus::Ready(
                u64::try_from(self.created.elapsed().as_millis()).unwrap_or(u64::MAX),
            )),
            _ => None,
        }
    }
}

/// Park a task on `signal`; run `trigger` on another thread after 200 ms. Returns
/// the milliseconds until the task ran again and the readiness checks it cost.
async fn park_and_trigger<F>(signal: EventReadinessPtr, registers: bool, trigger: F) -> (u64, usize)
where
    F: FnOnce() + Send + 'static,
{
    assert_eq!(
        signal.register(&ReadinessWaker::noop()),
        registers,
        "whether the readiness under test takes the task's waker"
    );
    let checks = Arc::new(AtomicUsize::new(0));
    let counted: EventReadinessPtr = Arc::new(Counting {
        inner: signal,
        checks: Arc::clone(&checks),
    });
    let mut stream = execute(
        ParkThenReport {
            created: Instant::now(),
            signal: Some(counted),
            step: 0,
        },
        None,
    )
    .expect("execute")
    .into_next_stream();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        trigger();
    });
    let ms = std::future::poll_fn(|cx| Pin::new(&mut stream).poll_next(cx))
        .await
        .expect("the task yields a value");
    thread.join().expect("trigger thread");
    (ms, checks.load(Ordering::SeqCst))
}

/// The most checks a 200 ms park may cost. An event-driven park checks the
/// readiness a few times (2 to 12 alone). A wake belongs to one task, so the
/// tests of this binary that run beside this one do not interrupt it. A 1 ms
/// re-check makes 170 (`multi`) to 350 or more, so the bound still tells them
/// apart.
const MAX_CHECKS: usize = 100;

fn assert_woke(label: &str, ms: u64, checks: usize) {
    eprintln!("{label}: {ms} ms, {checks} checks");
    assert!(
        (200..400).contains(&ms),
        "{label}: the task ran again after {ms} ms; the store came at 200 ms"
    );
    assert!(
        checks <= MAX_CHECKS,
        "{label}: the executor checked the parked readiness {checks} times; the bound is {MAX_CHECKS}"
    );
}

#[valtron_test(threads = 1, timeout = 10_000)]
async fn queue_readiness_wakes_on_a_push_from_another_thread() {
    let queue = EventQueue::unbounded();
    let pusher = queue.clone();
    let (ms, checks) = park_and_trigger(Arc::new(queue.readiness()), true, move || {
        pusher.push(1u32).expect("push");
    })
    .await;
    assert_woke("EventQueue::readiness", ms, checks);
}

#[valtron_test(threads = 1, timeout = 10_000)]
async fn pipe_receiver_readiness_wakes_on_try_send_from_another_thread() {
    let (tx, rx) = Pipe::<u32>::with_depth(4);
    let (ms, checks) = park_and_trigger(Arc::new(rx.readiness()), true, move || {
        tx.try_send(7).expect("send");
    })
    .await;
    assert_woke("PipeReceiver::readiness", ms, checks);
}

#[valtron_test(threads = 1, timeout = 10_000)]
async fn pipe_sender_vacancy_wakes_on_recv_from_another_thread() {
    let (tx, rx) = Pipe::<u32>::with_depth(1);
    tx.try_send(1).expect("fill the pipe");
    let (ms, checks) = park_and_trigger(Arc::new(tx.vacancy()), true, move || {
        assert_eq!(rx.try_recv().expect("recv"), 1);
    })
    .await;
    assert_woke("PipeSender::vacancy", ms, checks);
}

#[valtron_test(threads = 1, timeout = 10_000)]
async fn resume_signal_wakes_on_set_from_another_thread() {
    let signal = ResumeSignal::new(false);
    let setter = signal.clone();
    let (ms, checks) = park_and_trigger(Arc::new(signal), true, move || setter.set(true)).await;
    assert_woke("ResumeSignal", ms, checks);
}

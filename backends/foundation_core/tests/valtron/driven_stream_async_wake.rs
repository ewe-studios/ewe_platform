//! An async consumer of a single-thread `DrivenStreamIterator` is woken when a
//! parked task becomes ready, even if nothing is pushed to the output queue
//! (spec 70 feature 46).
//!
//! WHY: the iterator drives the engine inside `next`. A parked task becomes
//! runnable when a foreign thread flips its readiness, a timer passes its
//! deadline, or a raw flag changes. None of these pushes to the queue, so a
//! `Pending` consumer (`into_collect_future` under smol or tokio) had nothing
//! to wake it and hung.
//!
//! WHAT: one task parks once on a readiness, then yields 42. Each test awaits
//! it under smol or tokio with a 10 s limit. The park lasts 2 s, longer than
//! the blocking waits inside `next`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{
    execute, valtron_test, Ctx, EventReadiness, EventReadinessPtr, ReadinessWaker,
    StreamIteratorExt, TaskIterator, TaskStatus, WakeSignal,
};

/// How long each task stays parked.
const PARK: Duration = Duration::from_secs(2);

/// Parks once on `signal`, then yields 42.
struct ParkOnce {
    signal: Option<EventReadinessPtr>,
    parked: bool,
}

impl ParkOnce {
    fn new(signal: EventReadinessPtr) -> Self {
        Self {
            signal: Some(signal),
            parked: false,
        }
    }
}

impl TaskIterator for ParkOnce {
    type Ready = u32;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if let Some(signal) = self.signal.take() {
            self.parked = true;
            return Some(TaskStatus::Depends(signal));
        }
        if self.parked {
            self.parked = false;
            return Some(TaskStatus::Ready(42));
        }
        None
    }
}

/// Ready at an instant; nothing flips it. It reports the instant as its deadline
/// and is never re-checked on a timer.
struct DeadlineOnly(Instant);

impl EventReadiness for DeadlineOnly {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        Instant::now() >= self.0
    }

    fn deadline(&self) -> Option<Instant> {
        Some(self.0)
    }

    fn register(&self, _waker: &ReadinessWaker) -> bool {
        true
    }
}

/// A raw flag flipped by a thread that does not wake the task. It keeps the
/// default `register` (`false`) and is re-checked on a timer.
struct RawFlag(Arc<AtomicBool>);

impl EventReadiness for RawFlag {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

fn under_smol(task: ParkOnce) -> Vec<u32> {
    smol::block_on(async move {
        execute(task, None)
            .expect("engine initialized")
            .into_collect_future()
            .await
    })
}

fn under_tokio(task: ParkOnce) -> Vec<u32> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio runtime");
    rt.block_on(async move {
        execute(task, None)
            .expect("engine initialized")
            .into_collect_future()
            .await
    })
}

/// A `WakeSignal` and a thread that sets it after `PARK`.
fn signal_flipped_later() -> EventReadinessPtr {
    let signal = WakeSignal::new();
    let flipper = signal.clone();
    std::thread::spawn(move || {
        std::thread::sleep(PARK);
        // Built here: the callback is not `Send` without `multi`.
        (flipper.callback())();
    });
    Arc::new(signal)
}

fn raw_flag_flipped_later() -> EventReadinessPtr {
    let flag = Arc::new(AtomicBool::new(false));
    let setter = Arc::clone(&flag);
    std::thread::spawn(move || {
        std::thread::sleep(PARK);
        setter.store(true, Ordering::SeqCst);
    });
    Arc::new(RawFlag(flag))
}

#[valtron_test(seed = 42, timeout = 10_000)]
fn smol_consumer_wakes_after_long_park() {
    let got = under_smol(ParkOnce::new(signal_flipped_later()));
    assert_eq!(got, vec![42]);
}

#[valtron_test(seed = 42, timeout = 10_000)]
fn tokio_consumer_wakes_after_long_park() {
    let got = under_tokio(ParkOnce::new(signal_flipped_later()));
    assert_eq!(got, vec![42]);
}

#[valtron_test(seed = 42, timeout = 10_000)]
fn smol_consumer_wakes_after_deadline_only_park() {
    let got = under_smol(ParkOnce::new(Arc::new(DeadlineOnly(Instant::now() + PARK))));
    assert_eq!(got, vec![42]);
}

#[valtron_test(seed = 42, timeout = 10_000)]
fn tokio_consumer_wakes_after_deadline_only_park() {
    let got = under_tokio(ParkOnce::new(Arc::new(DeadlineOnly(Instant::now() + PARK))));
    assert_eq!(got, vec![42]);
}

#[valtron_test(seed = 42, timeout = 10_000)]
fn smol_consumer_wakes_after_needs_polling_park() {
    let got = under_smol(ParkOnce::new(raw_flag_flipped_later()));
    assert_eq!(got, vec![42]);
}

#[valtron_test(seed = 42, timeout = 10_000)]
fn tokio_consumer_wakes_after_needs_polling_park() {
    let got = under_tokio(ParkOnce::new(raw_flag_flipped_later()));
    assert_eq!(got, vec![42]);
}

//! A task parked on a readiness that carries a deadline wakes at that deadline
//! (spec 70 feature 75, E1).
//!
//! WHY: `Sleepable::Readiness` reported no deadline, so an executor with nothing
//! else to do did not know when the task could become ready. A signal such as
//! "socket ready OR timer passed" (the QUIC driver, the 100-continue wait) was
//! re-checked only when something else woke the executor: the task slept past
//! its timer, or the worker spun to re-check it.
//!
//! WHAT: the only task parks on a signal that is never signalled and has a
//! deadline 200 ms away. It must run again within 200 to 400 ms of the park,
//! without burning the CPU while it waits.

use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{
    execute, valtron_test, AnyReadiness, BoolSignal, Ctx, EventReadiness, EventReadinessPtr,
    ResumeSignal, StreamIteratorExt, TaskIterator, TaskStatus,
};
use futures_core::Stream as FuturesStream;

/// Ready once its instant has passed. Nothing else signals it.
struct DeadlineSignal(Instant);

impl EventReadiness for DeadlineSignal {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        Instant::now() >= self.0
    }

    fn deadline(&self) -> Option<Instant> {
        Some(self.0)
    }
}

/// Parks once on `signal`, then yields the milliseconds from its creation.
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
            1 => {
                let ms = u64::try_from(self.created.elapsed().as_millis()).unwrap_or(u64::MAX);
                Some(TaskStatus::Ready(ms))
            }
            _ => None,
        }
    }
}

/// The CPU time this process has used, all threads.
fn process_cpu() -> Duration {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid out pointer for the call.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut ts) };
    assert_eq!(rc, 0, "clock_gettime");
    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}

/// Run `ParkThenReport` on `signal` and return (milliseconds until it ran again,
/// CPU used meanwhile).
async fn park_and_measure(signal: EventReadinessPtr) -> (u64, Duration) {
    let cpu_before = process_cpu();
    let mut stream = execute(
        ParkThenReport {
            created: Instant::now(),
            signal: Some(signal),
            step: 0,
        },
        None,
    )
    .expect("execute")
    .into_next_stream();
    let value = std::future::poll_fn(|cx| Pin::new(&mut stream).poll_next(cx)).await;
    (
        value.expect("the task yields a value"),
        process_cpu() - cpu_before,
    )
}

/// The only task, parked on a readiness whose deadline is 200 ms away and which
/// is never signalled, runs again within 200 to 400 ms.
#[valtron_test(threads = 1, timeout = 10_000)]
async fn task_parked_on_deadline_readiness_wakes_on_time() {
    let signal = Arc::new(DeadlineSignal(Instant::now() + Duration::from_millis(200)));
    let (ms, cpu) = park_and_measure(signal).await;

    assert!(
        (200..400).contains(&ms),
        "the task ran again after {ms} ms; the deadline is 200 ms"
    );
    assert!(
        cpu < Duration::from_millis(100),
        "the wait for a 200 ms deadline used {cpu:?} of CPU"
    );
}

/// `AnyReadiness` reports the earliest deadline of its children: with children
/// due at 200 ms and 800 ms and one that never fires, the task runs at 200 ms.
#[valtron_test(threads = 1, timeout = 10_000)]
async fn any_readiness_wakes_at_the_earliest_child_deadline() {
    let now = Instant::now();
    let signal = Arc::new(
        AnyReadiness::either(
            Arc::new(DeadlineSignal(now + Duration::from_millis(800))),
            Arc::new(BoolSignal::new(false)),
        )
        .with(Arc::new(DeadlineSignal(now + Duration::from_millis(200)))),
    );
    let (ms, cpu) = park_and_measure(signal).await;

    assert!(
        (200..400).contains(&ms),
        "the task ran again after {ms} ms; the earliest child deadline is 200 ms"
    );
    assert!(
        cpu < Duration::from_millis(100),
        "the wait used {cpu:?} of CPU"
    );
}

/// Parks once on a signal that never fires; ends only when the pool shuts down.
struct ParkForever {
    signal: Option<EventReadinessPtr>,
}

impl TaskIterator for ParkForever {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.signal.take().map(TaskStatus::Depends)
    }
}

/// A worker whose only task is parked on an event-driven signal
/// (`EventReadiness::needs_polling` is `false`) sleeps: it does not re-check the
/// signal on the 1 ms readiness quantum. The CPU of the whole process over one
/// second stays under 20 ms. (With `needs_polling` left at its default `true`
/// the same worker re-checks every millisecond and uses several times that.)
#[valtron_test(threads = 1, timeout = 20_000)]
async fn worker_with_event_driven_sleeper_uses_no_cpu() {
    let signal: EventReadinessPtr = Arc::new(ResumeSignal::new(false));
    let _stream = execute(
        ParkForever {
            signal: Some(signal),
        },
        None,
    )
    .expect("execute")
    .into_next_stream();

    // Let the task reach its park, then hold still.
    std::thread::sleep(Duration::from_millis(300));
    let before = process_cpu();
    std::thread::sleep(Duration::from_secs(1));
    let used = process_cpu() - before;
    assert!(
        used < Duration::from_millis(20),
        "a worker parked on an event-driven signal used {used:?} of CPU in one second"
    );
}

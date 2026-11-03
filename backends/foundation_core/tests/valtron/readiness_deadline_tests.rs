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
//! (The CPU cost of a parked worker is measured by `event_driven_cpu` below.)

use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{
    execute, valtron_test, AnyReadiness, BoolSignal, Ctx, EventReadiness, EventReadinessPtr,
    StreamIteratorExt, TaskIterator, TaskStatus,
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

/// Run `ParkThenReport` on `signal` and return the milliseconds until it ran again.
async fn park_and_measure(signal: EventReadinessPtr) -> u64 {
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
    value.expect("the task yields a value")
}

/// The only task, parked on a readiness whose deadline is 200 ms away and which
/// is never signalled, runs again within 200 to 400 ms.
#[valtron_test(threads = 1, timeout = 10_000)]
async fn task_parked_on_deadline_readiness_wakes_on_time() {
    let signal = Arc::new(DeadlineSignal(Instant::now() + Duration::from_millis(200)));
    let ms = park_and_measure(signal).await;

    assert!(
        (200..400).contains(&ms),
        "the task ran again after {ms} ms; the deadline is 200 ms"
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
    let ms = park_and_measure(signal).await;

    assert!(
        (200..400).contains(&ms),
        "the task ran again after {ms} ms; the earliest child deadline is 200 ms"
    );
}

/// Tests that measure the CPU of pool worker threads: the single-thread executor
/// has none (it runs on the caller).
#[cfg(all(target_os = "linux", feature = "multi"))]
mod event_driven_cpu {
    use super::*;
    use foundation_core::valtron::ResumeSignal;

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

    /// CPU nanoseconds used so far by the threads of this test: the test thread and
    /// the pool workers it started, which carry its name (truncated to 15 bytes).
    /// Other tests of the binary run beside this one, so the whole-process CPU is not
    /// a fair measure.
    fn own_threads_cpu(comm: &str) -> Duration {
        let mut total = 0u64;
        for entry in std::fs::read_dir("/proc/self/task")
            .expect("task dir")
            .flatten()
        {
            let name = std::fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
            if name.trim() != comm {
                continue;
            }
            let sched = std::fs::read_to_string(entry.path().join("schedstat")).unwrap_or_default();
            total += sched
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
        }
        Duration::from_nanos(total)
    }

    /// A signal that never fires and keeps the default `needs_polling` (`true`): the
    /// executor must re-check it on the readiness quantum.
    struct PollingSignal;

    impl EventReadiness for PollingSignal {
        fn is_ready(&self, _dur: Option<Duration>) -> bool {
            false
        }
    }

    /// Park a task on `signal`, let it settle, and return the CPU the test's threads
    /// use over the next second.
    fn cpu_while_parked(signal: EventReadinessPtr, comm: &str) -> Duration {
        let _stream = execute(
            ParkForever {
                signal: Some(signal),
            },
            None,
        )
        .expect("execute")
        .into_next_stream();
        std::thread::sleep(Duration::from_millis(300));
        let before = own_threads_cpu(comm);
        std::thread::sleep(Duration::from_secs(1));
        let used = own_threads_cpu(comm).saturating_sub(before);
        std::mem::forget(_stream);
        used
    }

    /// A worker whose only task is parked on an event-driven signal
    /// (`EventReadiness::needs_polling` is `false`) sleeps: it does not re-check the
    /// signal on the 1 ms readiness quantum. Over one second it uses at most two
    /// thirds of the CPU of a worker that parks on a default (polling) signal; in
    /// practice about half.
    #[valtron_test(threads = 1, timeout = 20_000)]
    async fn worker_with_event_driven_sleeper_uses_less_cpu_than_a_polling_one() {
        // The test thread is named after its module path; the pool workers inherit it.
        let comm = "valtron::readin";
        let event = cpu_while_parked(Arc::new(ResumeSignal::new(false)), comm);
        let polling = cpu_while_parked(Arc::new(PollingSignal), comm);
        eprintln!("event-driven {event:?}, polling {polling:?}");
        assert!(
            event * 3 < polling * 2,
            "a worker parked on an event-driven signal used {event:?} of CPU in one \
         second; a polling one used {polling:?}"
        );
    }
}

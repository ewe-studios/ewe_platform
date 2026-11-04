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
//! deadline 300 ms away. It must run again within 300 to 500 ms of the park, and
//! the executor must check it only a few times while it waits (a counting adapter
//! in the test). (The CPU cost of a parked worker is measured by `event_driven_cpu` below.)

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{
    execute, valtron_test, AnyReadiness, Ctx, EventReadiness, EventReadinessPtr, ReadinessWaker,
    ResumeSignal, StreamIteratorExt, TaskIterator, TaskStatus,
};
use futures_core::Stream as FuturesStream;

/// Ready once its instant has passed. Nothing else signals it, so it needs no
/// re-check on a timer: the executor wakes at its deadline.
struct DeadlineSignal(Instant);

impl EventReadiness for DeadlineSignal {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        Instant::now() >= self.0
    }

    fn deadline(&self) -> Option<Instant> {
        Some(self.0)
    }

    /// Nothing signals it: the executor wakes at its deadline.
    fn register(&self, _waker: &ReadinessWaker) -> bool {
        true
    }
}

/// A test adapter that counts every `is_ready` call on the readiness it wraps.
///
/// WHY: a test that only measures when the task ran again passes on an executor
/// that re-checks the readiness every millisecond. The count tells the two apart:
/// an executor that sleeps checks the readiness a few times, a polling one
/// checks it hundreds of times.
///
/// With `force_polling` it refuses the task's waker (`register` returns `false`),
/// which makes the executor re-check it on the readiness quantum: it is the old
/// behaviour.
struct Counting {
    inner: EventReadinessPtr,
    checks: Arc<AtomicUsize>,
    force_polling: bool,
}

impl Counting {
    fn new(inner: EventReadinessPtr, force_polling: bool) -> (Arc<Self>, Arc<AtomicUsize>) {
        let checks = Arc::new(AtomicUsize::new(0));
        let signal = Arc::new(Self {
            inner,
            checks: Arc::clone(&checks),
            force_polling,
        });
        (signal, checks)
    }
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
        let registered = self.inner.register(waker);
        registered && !self.force_polling
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

/// How long the deadlines of these tests are from the start of the park.
const PARK_MS: u64 = 300;

/// The most readiness checks a parked task may cost over the park. An executor that
/// sleeps checks the readiness at the park, at the deadline and, on the
/// single-thread driver, once per `max_yield_duration` (100 ms) wake-up: 2 to 14
/// checks. The old 1 ms re-check made about `PARK_MS` (300) to 650 checks.
const MAX_CHECKS: usize = 20;

/// One deadline readiness, parked on and counted. `with_any` parks on an
/// `AnyReadiness` of the earliest deadline, a later one and a signal that never fires.
async fn run_deadline_case(with_any: bool, force_polling: bool) -> (u64, usize) {
    let now = Instant::now();
    let due = Duration::from_millis(PARK_MS);
    let inner: EventReadinessPtr = if with_any {
        Arc::new(
            AnyReadiness::either(
                Arc::new(DeadlineSignal(now + due * 4)),
                Arc::new(ResumeSignal::new(false)),
            )
            .with(Arc::new(DeadlineSignal(now + due))),
        )
    } else {
        Arc::new(DeadlineSignal(now + due))
    };
    let (signal, checks) = Counting::new(inner, force_polling);
    let ms = park_and_measure(signal).await;
    (ms, checks.load(Ordering::SeqCst))
}

fn assert_on_time(ms: u64) {
    assert!(
        (PARK_MS..PARK_MS + 200).contains(&ms),
        "the task ran again after {ms} ms; the deadline is {PARK_MS} ms"
    );
}

/// The only task, parked on a readiness whose deadline is 300 ms away and which
/// is never signalled, runs again on time and the executor checks the readiness
/// only a few times while it is parked (the old 1 ms re-check made about 300).
#[valtron_test(threads = 1, timeout = 10_000)]
async fn task_parked_on_deadline_readiness_wakes_on_time() {
    let (ms, checks) = run_deadline_case(false, false).await;
    eprintln!("deadline readiness: {ms} ms, {checks} checks");

    assert_on_time(ms);
    assert!(
        checks <= MAX_CHECKS,
        "the executor checked the parked readiness {checks} times; the bound is {MAX_CHECKS}"
    );
}

/// `AnyReadiness` reports the earliest deadline of its children: with children
/// due at 300 ms and 1200 ms and one that never fires, the task runs at 300 ms
/// with a few checks.
#[valtron_test(threads = 1, timeout = 10_000)]
async fn any_readiness_wakes_at_the_earliest_child_deadline() {
    let (ms, checks) = run_deadline_case(true, false).await;
    eprintln!("any readiness: {ms} ms, {checks} checks");

    assert_on_time(ms);
    assert!(
        checks <= MAX_CHECKS,
        "the executor checked the parked readiness {checks} times; the bound is {MAX_CHECKS}"
    );
}

/// The counting bound does discriminate: with polling forced on (the old
/// behaviour) the same park makes far more checks than `MAX_CHECKS`, and the task
/// still runs on time, so only the count tells the two apart.
#[valtron_test(threads = 1, timeout = 10_000)]
async fn forced_polling_makes_many_checks_during_a_park() {
    for with_any in [false, true] {
        let (ms, checks) = run_deadline_case(with_any, true).await;
        eprintln!("forced polling (any={with_any}): {ms} ms, {checks} checks");

        assert_on_time(ms);
        assert!(
            checks > MAX_CHECKS * 5,
            "with polling forced the executor checked the readiness only {checks} times; \
             the bound {MAX_CHECKS} would not tell it from an event-driven park"
        );
    }
}

/// Tests that measure the CPU of pool worker threads: the single-thread executor
/// has none (it runs on the caller).
#[cfg(all(target_os = "linux", feature = "multi"))]
mod event_driven_cpu {
    use super::*;

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

    /// A signal that never fires and keeps the default `register` (`false`): the
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
    /// (`EventReadiness::register` returns `true`) sleeps: it does not re-check the
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

    /// Per-thread CPU and wake-ups: (tid, comm) -> (cpu ns, voluntary context switches).
    fn thread_stats() -> std::collections::BTreeMap<String, (u64, u64)> {
        let mut out = std::collections::BTreeMap::new();
        for entry in std::fs::read_dir("/proc/self/task")
            .expect("task dir")
            .flatten()
        {
            let tid = entry.file_name().to_string_lossy().into_owned();
            let comm = std::fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
            let cpu = std::fs::read_to_string(entry.path().join("schedstat"))
                .unwrap_or_default()
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            let switches = std::fs::read_to_string(entry.path().join("status"))
                .unwrap_or_default()
                .lines()
                .find_map(|l| l.strip_prefix("voluntary_ctxt_switches:"))
                .and_then(|v| v.trim().parse::<u64>().ok())
                .unwrap_or(0);
            out.insert(format!("{tid} {}", comm.trim()), (cpu, switches));
        }
        out
    }

    /// An idle pool, with its only task parked on a `ResumeSignal`, costs each of
    /// its threads under 2 ms of CPU and under 30 wake-ups in one second.
    ///
    /// The thread that waits for the task's output (the test thread here) used to
    /// wake every millisecond (`NotifyQueue::wait_for_item` made 100 timed waits of
    /// 1 ms): about 27 ms of CPU and 900 wake-ups per second. The workers wake once
    /// per `max_yield_duration` (100 ms) at most: about 10 wake-ups per second.
    #[valtron_test(threads = 1, timeout = 20_000)]
    async fn idle_threads_parked_on_a_resume_signal_use_almost_no_cpu() {
        let _stream = execute(
            ParkForever {
                signal: Some(Arc::new(ResumeSignal::new(false))),
            },
            None,
        )
        .expect("execute")
        .into_next_stream();
        std::thread::sleep(Duration::from_millis(300));
        let before = thread_stats();
        std::thread::sleep(Duration::from_secs(1));
        let after = thread_stats();
        std::mem::forget(_stream);

        let comm = "valtron::readin";
        let mut worst_cpu = Duration::ZERO;
        let mut worst_wakeups = 0;
        for (name, (cpu, switches)) in &after {
            if !name.ends_with(comm) {
                continue;
            }
            let (cpu0, switches0) = before.get(name).copied().unwrap_or((0, 0));
            let used = Duration::from_nanos(cpu.saturating_sub(cpu0));
            let wakeups = switches.saturating_sub(switches0);
            eprintln!("thread {name}: {used:?} CPU, {wakeups} wake-ups");
            worst_cpu = worst_cpu.max(used);
            worst_wakeups = worst_wakeups.max(wakeups);
        }
        assert!(
            worst_cpu < Duration::from_millis(2),
            "an idle thread used {worst_cpu:?} of CPU in one second; the bound is 2 ms"
        );
        assert!(
            worst_wakeups < 30,
            "an idle thread woke {worst_wakeups} times in one second; the bound is 30"
        );
    }
}

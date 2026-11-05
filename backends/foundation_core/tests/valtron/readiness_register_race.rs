//! A change that lands between the executor's readiness check and its
//! `EventReadiness::register` call is not lost (spec 70 feature 80).
//!
//! WHY: the executor checks a park signal once before it registers its waker,
//! because a signal that is already ready never wakes anybody. A source that
//! becomes ready in the window between that check and the register finds no
//! waker to wake, so the task used to sleep until the signal's deadline. In the
//! HTTP client that is the 30 s DNS timeout: a request hung for 30 s after the
//! lookup had already finished.
//!
//! WHAT: the readiness below becomes ready inside its own `register` call and
//! does not wake the waker it is given. The executor must look once more after
//! the register and run the task at once, not at the 500 ms deadline.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_compact::Instant;
use foundation_core::valtron::{
    execute, valtron_test, Ctx, EventReadiness, EventReadinessPtr, ReadinessWaker,
    StreamIteratorExt, TaskIterator, TaskStatus,
};
use futures_core::Stream as FuturesStream;

/// Ready as soon as its `register` runs: the change happens in the window the
/// executor must protect. The deadline is the fallback an executor without the
/// post-register check sleeps to.
struct ChangeBeforeRegister {
    ready: AtomicBool,
    due: Instant,
    checks: Arc<AtomicUsize>,
}

impl EventReadiness for ChangeBeforeRegister {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.checks.fetch_add(1, Ordering::SeqCst);
        self.ready.load(Ordering::SeqCst)
    }

    fn deadline(&self) -> Option<Instant> {
        Some(self.due)
    }

    /// The source changes after the executor's check and before this call, so
    /// it had no waker to wake. Deliberately do not wake `_waker`: the executor
    /// must find the change with its own post-register look.
    fn register(&self, _waker: &ReadinessWaker) -> bool {
        self.ready.store(true, Ordering::SeqCst);
        true
    }
}

/// Parks once on `signal`, then yields the milliseconds since creation.
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

/// Run the parked task and return the milliseconds until it ran again.
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

/// How far away the fallback deadline is. A task that waits for it takes at
/// least this long, which the bound below tells apart from an immediate run.
const DEADLINE_MS: u64 = 500;

/// The most time the task may take when the executor sees the change: far
/// below `DEADLINE_MS`, so an executor that sleeps to the deadline fails.
const MAX_MS: u64 = 250;

/// A change inside `register` is found by the executor's post-register look: the
/// task runs at once, not at the 500 ms deadline.
#[valtron_test(threads = 1, timeout = 10_000)]
async fn change_between_check_and_register_is_not_lost() {
    let checks = Arc::new(AtomicUsize::new(0));
    let signal = Arc::new(ChangeBeforeRegister {
        ready: AtomicBool::new(false),
        due: Instant::now() + Duration::from_millis(DEADLINE_MS),
        checks: Arc::clone(&checks),
    });
    let ms = park_and_measure(signal).await;
    let checks = checks.load(Ordering::SeqCst);
    eprintln!("ran after {ms} ms with {checks} readiness checks");

    assert!(
        ms < MAX_MS,
        "the task ran after {ms} ms; the post-register look must see the change, \
         not the {DEADLINE_MS} ms deadline"
    );
    assert!(
        checks >= 2,
        "the executor checked the readiness {checks} times; it must check once \
         after the register"
    );
}

/// The same race on a pool worker: the task wakes at once there too, so the
/// lost wake cannot come back on the multi-threaded executor.
#[cfg(feature = "multi")]
#[valtron_test(threads = 2, timeout = 10_000)]
async fn change_between_check_and_register_is_not_lost_on_a_pool() {
    for _ in 0..4 {
        let signal = Arc::new(ChangeBeforeRegister {
            ready: AtomicBool::new(false),
            due: Instant::now() + Duration::from_millis(DEADLINE_MS),
            checks: Arc::new(AtomicUsize::new(0)),
        });
        let ms = park_and_measure(signal).await;
        assert!(
            ms < MAX_MS,
            "a pool worker's task ran after {ms} ms; the post-register look must \
             see the change, not the {DEADLINE_MS} ms deadline"
        );
    }
}

//! A wake reaches only the task it belongs to (spec 70 feature 80, E1).
//!
//! WHY: until feature 80 a source woke a parked task with a process-wide call that
//! interrupted every idle executor thread, and every interrupted executor looked at
//! all of its parked tasks. A wake meant for one task made an unrelated task
//! re-check its readiness thousands of times (a wake storm: the full
//! `foundation_core` run of the feature 46 + 77 merge failed 6 of 6 times with
//! "checked the parked readiness 2160-3780 times; the bound is 20").
//!
//! WHAT: task A parks on a readiness that never fires, counted. Task B parks again
//! and again on a fresh `ResumeSignal`, and another thread sets each one as soon
//! as it is published, as fast as it can. B is woken hundreds of times; A's
//! readiness must be checked only a few times.

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use foundation_core::valtron::{
    execute, valtron_test, Ctx, EventQueue, EventReadiness, EventReadinessPtr, ReadinessNotifier,
    ReadinessWaker, ResumeSignal, StreamIteratorExt, TaskIterator, TaskStatus,
};
use futures_core::Stream as FuturesStream;

/// Counts the `is_ready` calls of the readiness it wraps and forwards the rest.
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

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.inner.register(waker)
    }
}

/// Parks once on `signal`, then reports how long it waited.
struct ParkThenReport {
    created: Instant,
    signal: Option<EventReadinessPtr>,
    reported: bool,
}

impl TaskIterator for ParkThenReport {
    type Ready = u64;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<u64, ()>> {
        if let Some(signal) = self.signal.take() {
            return Some(TaskStatus::Depends(signal));
        }
        if self.reported {
            return None;
        }
        self.reported = true;
        let ms = u64::try_from(self.created.elapsed().as_millis()).unwrap_or(u64::MAX);
        Some(TaskStatus::Ready(ms))
    }
}

/// Parks on a fresh `ResumeSignal` each time it runs, until `stop` is set.
struct ParkAgainAndAgain {
    published: Arc<Mutex<Option<ResumeSignal>>>,
    wakes: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    parked: bool,
}

impl TaskIterator for ParkAgainAndAgain {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<(), ()>> {
        if self.parked {
            self.wakes.fetch_add(1, Ordering::SeqCst);
        }
        if self.stop.load(Ordering::SeqCst) {
            return None;
        }
        self.parked = true;
        let signal = ResumeSignal::new(false);
        *self.published.lock().unwrap() = Some(signal.clone());
        Some(TaskStatus::Depends(Arc::new(signal)))
    }
}

/// The most checks the parked task A may cost. A parked task that is woken only by
/// its own readiness is checked at the park and when it is released; the old storm
/// made 2160 to 3780.
const MAX_CHECKS: usize = 20;

#[valtron_test(threads = 2, timeout = 20_000)]
async fn a_wake_for_another_task_does_not_make_a_parked_task_re_check() {
    let published = Arc::new(Mutex::new(None::<ResumeSignal>));
    let wakes = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));

    // Task A: parked on a readiness nobody sets until the end.
    let never = ResumeSignal::new(false);
    let checks = Arc::new(AtomicUsize::new(0));
    let counted: EventReadinessPtr = Arc::new(Counting {
        inner: Arc::new(never.clone()),
        checks: Arc::clone(&checks),
    });
    let mut a = execute(
        ParkThenReport {
            created: Instant::now(),
            signal: Some(counted),
            reported: false,
        },
        None,
    )
    .expect("execute A")
    .into_next_stream();

    // Task B: woken again and again by the storm thread.
    let mut b = execute(
        ParkAgainAndAgain {
            published: Arc::clone(&published),
            wakes: Arc::clone(&wakes),
            stop: Arc::clone(&stop),
            parked: false,
        },
        None,
    )
    .expect("execute B")
    .into_next_stream();

    let storm = {
        let published = Arc::clone(&published);
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_millis(600);
            while Instant::now() < until {
                if let Some(signal) = published.lock().unwrap().take() {
                    signal.set(true);
                } else {
                    std::thread::yield_now();
                }
            }
            stop.store(true, Ordering::SeqCst);
            if let Some(signal) = published.lock().unwrap().take() {
                signal.set(true);
            }
        })
    };
    // Drive the executors until B has seen the stop flag (on one thread the
    // consumer is the executor).
    while std::future::poll_fn(|cx| Pin::new(&mut b).poll_next(cx))
        .await
        .is_some()
    {}
    storm.join().expect("storm thread");

    let checked = checks.load(Ordering::SeqCst);
    let woken = wakes.load(Ordering::SeqCst);
    eprintln!("B was woken {woken} times; A's readiness was checked {checked} times");
    assert!(
        woken >= 100,
        "the storm must be real: B was woken only {woken} times"
    );
    assert!(
        checked <= MAX_CHECKS,
        "task A's readiness was checked {checked} times while B was woken {woken} times; \
         the bound is {MAX_CHECKS}"
    );

    // A still wakes on its own readiness.
    never.set(true);
    let ms = std::future::poll_fn(|cx| Pin::new(&mut a).poll_next(cx))
        .await
        .expect("A runs again after its own readiness is set");
    assert!(ms >= 600, "A ran again after {ms} ms");
}

/// A queue that is closed wakes the task parked on it for items, and the task
/// parked on it for room (feature 80: `close` wakes both sides).
#[valtron_test(threads = 1, timeout = 10_000)]
async fn closing_an_event_queue_wakes_both_sides() {
    for vacancy in [false, true] {
        let queue = EventQueue::<u32>::bounded(1);
        if vacancy {
            queue.push(1).expect("fill");
        }
        let signal: EventReadinessPtr = if vacancy {
            Arc::new(queue.vacancy())
        } else {
            Arc::new(queue.readiness())
        };
        let closer = queue.clone();
        let mut stream = execute(
            ParkThenReport {
                created: Instant::now(),
                signal: Some(signal),
                reported: false,
            },
            None,
        )
        .expect("execute")
        .into_next_stream();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            closer.close();
        });
        let ms = std::future::poll_fn(|cx| Pin::new(&mut stream).poll_next(cx))
            .await
            .expect("the task runs again after the close");
        thread.join().expect("closer");
        assert!(
            (150..400).contains(&ms),
            "vacancy={vacancy}: the task ran again after {ms} ms; the close came at 150 ms"
        );
    }
}

/// The notifier keeps one waker per task, drops the wakers of tasks that are not
/// parked any more, and never grows without bound.
#[test]
fn a_notifier_does_not_grow_with_repeated_registers() {
    let notifier = ReadinessNotifier::new();
    let waker = ReadinessWaker::noop();
    for _ in 0..10_000 {
        assert!(notifier.register(&waker));
    }
    assert!(notifier.has_waiters());
    notifier.notify();
    assert!(
        format!("{notifier:?}").contains("registered: 1"),
        "one task, one waker: {notifier:?}"
    );
}

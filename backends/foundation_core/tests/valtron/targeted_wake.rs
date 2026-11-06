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
//! as it is published, as fast as it can. The storm stops after [`MAX_WAKES`]
//! wakes, or after 600 ms once it has made [`MIN_WAKES`] wakes. B is woken
//! hundreds of times; A's readiness must be checked only a few times.

#![cfg_attr(
    not(feature = "multi"),
    expect(
        clippy::arc_with_non_send_sync,
        reason = "one Arc queue serves both trees; the single-threaded task payload is not Send + Sync and never crosses a thread"
    )
)]
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{
    execute, valtron_test, Ctx, EventQueue, EventReadiness, EventReadinessPtr, ReadinessNotifier,
    ReadinessWaker, ResumeSignal, StreamIteratorExt, TaskIterator, TaskIteratorExt, TaskStatus,
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

/// A `ResumeSignal` that records when the executor has registered its waker,
/// that is, when the task is really parked on it.
///
/// WHY: setting the signal before the task parks makes the executor see a
/// `Depends` on an already-ready signal, which it counts as a task bug and
/// panics on after enough in a row (CI's faster runners hit that). The storm
/// thread sets a signal only after its waker is registered.
#[derive(Clone)]
struct ParkedSignal {
    signal: ResumeSignal,
    registered: Arc<AtomicBool>,
}

impl EventReadiness for ParkedSignal {
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        self.signal.is_ready(dur)
    }

    fn deadline(&self) -> Option<Instant> {
        self.signal.deadline()
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        let changed = self.signal.register(waker);
        self.registered.store(true, Ordering::SeqCst);
        changed
    }
}

/// Parks on a fresh `ResumeSignal` each time it runs, until `stop` is set.
///
/// WHY: each signal goes to the storm thread through an unbounded channel.
/// `send` never blocks, so the executor never waits on a lock that the storm
/// thread holds.
struct ParkAgainAndAgain {
    published: Sender<ParkedSignal>,
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
        let signal = ParkedSignal {
            signal: ResumeSignal::new(false),
            registered: Arc::new(AtomicBool::new(false)),
        };
        if self.published.send(signal.clone()).is_err() {
            // The storm thread is gone. Do not park on a signal that nobody
            // can set.
            return None;
        }
        // The storm can stop between the check above and the send. Check once
        // more: if it did stop, end now. The receiver may still hold this
        // signal. Setting it is harmless because this task does not park.
        if self.stop.load(Ordering::SeqCst) {
            return None;
        }
        Some(TaskStatus::Depends(Arc::new(signal)))
    }
}

/// The most checks the parked task A may cost. A parked task that is woken only by
/// its own readiness is checked at the park and when it is released; the old storm
/// made 2160 to 3780.
const MAX_CHECKS: usize = 20;

/// The fewest wakes the test needs.
const MIN_WAKES: usize = 100;

/// The storm stops after this many wakes, even if its time bound has not passed.
///
/// WHY: a work bound ends the storm early on a fast machine. The bound is above
/// [`MIN_WAKES`], so the test still sees a real storm.
const MAX_WAKES: usize = 200;

/// The time after which the storm ends, if B has also been woken [`MIN_WAKES`]
/// times, in milliseconds.
///
/// WHY: this is a floor check, not a hard cut. Under a heavy host load the
/// executor can be too starved to reach [`MIN_WAKES`] wakes in this time.
const STORM_MS: u64 = 600;

/// The time after which the storm always ends, in milliseconds.
///
/// WHY: this hard cap keeps the test bounded when the executor is starved. It is
/// well below the test's 20 s timeout.
const STORM_HARD_MS: u64 = 5_000;

/// How long the storm waits for a new signal before it reads the counters, in
/// milliseconds.
///
/// WHY: the storm thread must not busy-spin. `recv_timeout` parks it, so the
/// executor gets the CPU. A send wakes it at once.
const STORM_POLL_MS: u64 = 1;

/// The most time the storm waits for B to end after `stop`, in milliseconds.
const STORM_DRAIN_MS: u64 = 5_000;

/// The storm prints the wake count after each group of this many wakes.
const REPORT_EVERY: usize = 100;

/// Sets task B's published signals until the storm ends.
///
/// WHY: the storm stops on work ([`MAX_WAKES`] wakes) or on time once it has
/// made [`MIN_WAKES`] wakes ([`STORM_MS`]), with a hard cap ([`STORM_HARD_MS`]).
/// It then drains the channel until the driver loop returns, so the last wake
/// cannot be lost.
fn run_storm(
    parked: &Receiver<ParkedSignal>,
    wakes: &AtomicUsize,
    stop: &AtomicBool,
    finished: &AtomicBool,
) {
    let started = Instant::now();
    let soft_until = started + Duration::from_millis(STORM_MS);
    let hard_until = started + Duration::from_millis(STORM_HARD_MS);
    let mut pending: Option<ParkedSignal> = None;
    let mut reported = 0usize;
    // Set signals until B has been woken `MAX_WAKES` times, or until
    // `STORM_MS` has passed and B was woken at least `MIN_WAKES` times.
    // `STORM_HARD_MS` always ends the storm.
    loop {
        let woken = wakes.load(Ordering::SeqCst);
        if woken / REPORT_EVERY > reported {
            reported = woken / REPORT_EVERY;
            eprintln!("storm: B was woken {woken} times");
        }
        let now = Instant::now();
        if woken >= MAX_WAKES || (now >= soft_until && woken >= MIN_WAKES) || now >= hard_until {
            break;
        }
        if pending.is_none() {
            // Park until B publishes a signal. This leaves the CPU to the
            // executor; a send wakes this thread at once.
            match parked.recv_timeout(Duration::from_millis(STORM_POLL_MS)) {
                Ok(signal) => pending = Some(signal),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        // Set a signal only after the executor registered its waker: a task
        // that parks on an already-ready signal counts as a violation.
        if pending
            .as_ref()
            .is_some_and(|signal| signal.registered.load(Ordering::SeqCst))
        {
            let signal = pending.take().expect("the pending signal");
            signal.signal.set(true);
        } else if pending.is_some() {
            // Wait for the executor to register its waker. Yield so it runs.
            std::thread::yield_now();
        }
    }
    // End the storm: B sees this flag on its next run.
    stop.store(true, Ordering::SeqCst);
    eprintln!(
        "storm: stop set after {} wakes",
        wakes.load(Ordering::SeqCst)
    );
    // Guarantee the final wake. The executor can publish a signal after the
    // last read of `stop`. Set every signal that is still in the channel so B
    // wakes, sees `stop`, and ends. Keep the receiver alive until the driver
    // loop has returned: dropping it makes a later send fail.
    if let Some(signal) = pending.take() {
        signal.signal.set(true);
    }
    let drain_until = Instant::now() + Duration::from_millis(STORM_DRAIN_MS);
    loop {
        if finished.load(Ordering::SeqCst) || Instant::now() >= drain_until {
            break;
        }
        match parked.recv_timeout(Duration::from_millis(50)) {
            Ok(signal) => signal.signal.set(true),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    // Keep task A parked for at least `STORM_MS`. The final assertion uses that
    // floor, and the wake storm can end on its work bound before the time
    // bound. This wait sets no signal.
    std::thread::sleep(soft_until.saturating_duration_since(Instant::now()));
}

#[valtron_test(threads = 2, timeout = 20_000)]
async fn a_wake_for_another_task_does_not_make_a_parked_task_re_check() {
    let (published, parked) = mpsc::channel::<ParkedSignal>();
    let wakes = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));

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
            published,
            wakes: Arc::clone(&wakes),
            stop: Arc::clone(&stop),
            parked: false,
        },
        None,
    )
    .expect("execute B")
    .into_next_stream();

    let storm = {
        let wakes = Arc::clone(&wakes);
        let stop = Arc::clone(&stop);
        let finished = Arc::clone(&finished);
        std::thread::spawn(move || run_storm(&parked, &wakes, &stop, &finished))
    };
    // Drive the executors until B has seen the stop flag (on one thread the
    // consumer is the executor).
    while std::future::poll_fn(|cx| Pin::new(&mut b).poll_next(cx))
        .await
        .is_some()
    {}
    // Let the storm thread release its receiver. B ended, so nothing more can
    // be published.
    finished.store(true, Ordering::SeqCst);
    storm.join().expect("storm thread");

    let checked = checks.load(Ordering::SeqCst);
    let woken = wakes.load(Ordering::SeqCst);
    eprintln!("B was woken {woken} times; A's readiness was checked {checked} times");
    assert!(
        woken >= MIN_WAKES,
        "the storm must be real: B was woken only {woken} times; the floor is {MIN_WAKES}"
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
            let _ = closer.close();
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

/// A source task for the observer tests: yields 1, 2, 3 and ends.
struct Count(u32);

impl TaskIterator for Count {
    type Ready = u32;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<u32, ()>> {
        if self.0 >= 3 {
            return None;
        }
        self.0 += 1;
        Some(TaskStatus::Ready(self.0))
    }
}

/// Parks a task on the observer's readiness of `split_collector`; a thread drives
/// the continuation (which pushes the copied items, then closes the queue). The
/// task must run again soon after the first push (`close = false`) or after the
/// close with nothing pushed (`close = true`, the predicate matches nothing).
/// The queue's own push and close do the wake: nothing else calls an executor.
async fn observer_wakes(close: bool) -> u64 {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let driver = std::thread::spawn(move || {
        let (observer, mut continuation) = Count(0).split_collector(move |_| !close, 8);
        // The clock starts here, where the 200 ms sleep starts, so the measured
        // wait cannot come out under 200 ms by thread start-up order.
        ready_tx
            .send((observer.readiness(), Instant::now()))
            .expect("send readiness");
        std::thread::sleep(Duration::from_millis(200));
        while continuation.next_status(Ctx::host()).is_some() {}
        drop(continuation);
        // Keep the observer alive until the continuation is done: dropping it
        // closes the queue too.
        drop(observer);
    });
    let (readiness, started) = ready_rx.recv().expect("readiness");
    let readiness: EventReadinessPtr = Arc::new(readiness);
    let mut stream = execute(
        ParkThenReport {
            created: started,
            signal: Some(readiness),
            reported: false,
        },
        None,
    )
    .expect("execute")
    .into_next_stream();
    let ms = std::future::poll_fn(|cx| Pin::new(&mut stream).poll_next(cx))
        .await
        .expect("the task runs again after the observer's queue changed");
    driver.join().expect("driver thread");
    ms
}

/// An observer push wakes the task parked on the observer's readiness.
#[valtron_test(threads = 1, timeout = 10_000)]
async fn an_observer_push_wakes_the_parked_task() {
    let ms = observer_wakes(false).await;
    assert!((200..450).contains(&ms), "the task ran again after {ms} ms");
}

/// Closing the observer's queue wakes the task parked on the observer's
/// readiness, even when nothing was pushed.
#[valtron_test(threads = 1, timeout = 10_000)]
async fn an_observer_close_wakes_the_parked_task() {
    let ms = observer_wakes(true).await;
    assert!((200..450).contains(&ms), "the task ran again after {ms} ms");
}

/// A source outside the queue types: a flag plus a `ReadinessNotifier`, the construct
/// a new kind of source (a socket, a timer, a shutdown flag) holds.
struct FlagSource {
    flag: AtomicBool,
    notifier: ReadinessNotifier,
}

impl EventReadiness for FlagSource {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.notifier.register(waker)
    }
}

/// A task parked on a custom source wakes when another thread sets the flag and calls
/// `notify`, and the executor checks the source only a few times meanwhile.
#[valtron_test(threads = 1, timeout = 10_000)]
async fn a_task_parked_on_a_notifier_source_wakes_on_notify() {
    let source = Arc::new(FlagSource {
        flag: AtomicBool::new(false),
        notifier: ReadinessNotifier::new(),
    });
    let checks = Arc::new(AtomicUsize::new(0));
    let counted: EventReadinessPtr = Arc::new(Counting {
        inner: source.clone(),
        checks: Arc::clone(&checks),
    });
    let mut stream = execute(
        ParkThenReport {
            created: Instant::now(),
            signal: Some(counted),
            reported: false,
        },
        None,
    )
    .expect("execute")
    .into_next_stream();
    let setter = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        source.flag.store(true, Ordering::SeqCst);
        source.notifier.notify();
    });
    let ms = std::future::poll_fn(|cx| Pin::new(&mut stream).poll_next(cx))
        .await
        .expect("the task runs again after notify");
    setter.join().expect("setter thread");
    let checked = checks.load(Ordering::SeqCst);
    eprintln!("notifier source: {ms} ms, {checked} checks");
    assert!((200..450).contains(&ms), "the task ran again after {ms} ms");
    assert!(
        checked <= MAX_CHECKS,
        "the source was checked {checked} times; the bound is {MAX_CHECKS}"
    );
}

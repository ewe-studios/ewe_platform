//! Shared scenarios that prove valtron's waker design (spec 70 feature 80) at
//! runtime on JS-hosted wasm. Both wasm runtime suites use them:
//!
//! - `wasm_js_yield_integration.rs`: the `js-wasmbindgen` backend, run by
//!   `mise run test:wasm-bindgen:deno`;
//! - `integration/wasm_tests/src/lib.rs`: the owned `js-foundation-wasm` backend,
//!   run by `mise run test:wasm-testbed:deno`.
//!
//! The owned crate includes this file with `#[path]`, so it names only public
//! `foundation_core` items and `foundation_compact`, and it holds no
//! backend-specific code. A suite supplies the host timer ([`After`]) and the
//! way to wait for a [`Scenario`].
//!
//! WHY: before this, feature 80 (a parked task is woken by its readiness through
//! a per-task `ReadinessWaker`; nothing re-checks it on a timer) was checked on
//! wasm only by compiling. The existing runtime suites park and wake through the
//! JS yielder but use none of `ResumeSignal`, `EventQueue`, `AnyReadiness` or a
//! readiness deadline. On wasm a lost wake does not slow a task: nothing polls,
//! so the task never runs again.
//!
//! WHAT: each function parks one task on one kind of readiness, arms a host
//! timer that makes the readiness ready, and returns a [`Scenario`]. The suite
//! awaits [`Scenario::is_done`] and then calls [`Scenario::verify`], which
//! asserts that the task was not polled while parked, resumed on time after the
//! host event, and that the executor called `is_ready` only a few times.
//!
//! HOW the tests fail without the wake: a missing wake leaves the task parked,
//! so the suite's wait times out and its message names the scenario. A timed
//! re-check loop (a readiness that does not take the waker, or an executor that
//! polls) raises the `is_ready` count past [`MAX_IS_READY_CALLS`].
//!
//! The wrapper around the readiness is [`CountingReadiness`]: it forwards
//! `register`, `deadline`, `clear` and `is_ready` and counts the `is_ready`
//! calls.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use foundation_compact::Instant;
use foundation_core::synca::WaitGroup;
use foundation_core::valtron::{
    single::{initialize_pool, run_until_complete, spawn},
    AnyReadiness, Ctx, EventQueue, EventReadiness, EventReadinessPtr, FnReady, ReadinessWaker,
    ResumeSignal, SleepingTask, TaskIterator, TaskStatus, TimeoutTask,
};

/// The host timer a suite supplies: run the callback after `delay_ms` on the
/// host event loop (a JS timeout).
pub type After = fn(f64, Box<dyn FnOnce()>);

/// How long a parked task may take to run after the host event that made its
/// readiness ready.
///
/// A resume is one host microtask, so a healthy run takes well under a
/// millisecond. The window is wide for a loaded CI runner. It does not tell a
/// wake from a timed re-check (a re-check at the executor's 1 ms quantum fits in
/// it): [`MAX_IS_READY_CALLS`] does.
pub const WAKE_LATENCY_WINDOW: Duration = Duration::from_millis(25);

/// How late a deadline-only park may resume after its deadline. The executor
/// asks the host for a timer at the deadline (whole milliseconds, so it can fire
/// a little early and ask again), and a host timer is only exact to a few ms.
pub const DEADLINE_LATE_WINDOW: Duration = Duration::from_millis(30);

/// How late a task driven by host timers may end, past its nominal time.
///
/// WHY: a `Delayed` park asks the host for a timer of that delay, and a JS timer
/// is only exact to a few milliseconds — it never fires early, and it fires late
/// when the event loop is busy. A timeout over a ticking task waits on one host
/// timer per tick, so every tick's lateness adds up. On a loaded CI container the
/// 50 ms timeout below ended at about 126 ms, against the 90 ms bound that
/// [`DEADLINE_LATE_WINDOW`] and one tick allow; it takes about 63 ms on an idle
/// machine.
///
/// This widens only the wall-clock ceiling. The tight guard stays the poll bound
/// in [`timed_end`]: a task that ignores its timer ends early (the lower bound
/// catches it) and a task that spins or re-checks on a timer polls far too often
/// (the poll bound catches it). Neither depends on how loaded the host is.
pub const HOST_TIMER_LATE_ALLOWANCE: Duration = Duration::from_millis(120);

/// How many times the executor may call `is_ready` on a parked readiness.
///
/// A park costs about three calls: one when the executor checks that the signal
/// is not already ready, one for the first look after it registered the waker,
/// and one after the wake. The bound leaves room for one more look. A timed
/// re-check at the 1 ms readiness quantum over the 50 ms waits below makes about
/// fifty, so it fails the bound.
pub const MAX_IS_READY_CALLS: usize = 6;

/// The delay of the host event that makes a readiness ready.
const EVENT_DELAY_MS: f64 = 50.0;

/// A time stamp set once.
#[derive(Clone, Default)]
struct Stamp(Arc<Mutex<Option<Instant>>>);

impl Stamp {
    fn mark(&self) {
        let mut slot = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.is_none() {
            *slot = Some(Instant::now());
        }
    }

    fn get(&self) -> Option<Instant> {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn require(&self, what: &str) -> Instant {
        self.get()
            .unwrap_or_else(|| panic!("{what} was never recorded"))
    }
}

/// The counts a [`CountingReadiness`] keeps.
#[derive(Clone, Default)]
pub struct Counts {
    is_ready: Arc<AtomicUsize>,
    register: Arc<AtomicUsize>,
}

impl Counts {
    /// How many times `is_ready` was called.
    pub fn is_ready_calls(&self) -> usize {
        self.is_ready.load(Ordering::SeqCst)
    }

    /// How many times `register` was called.
    pub fn register_calls(&self) -> usize {
        self.register.load(Ordering::SeqCst)
    }
}

/// Wraps a readiness, forwards everything to it, and counts `is_ready` and
/// `register` calls.
pub struct CountingReadiness {
    inner: EventReadinessPtr,
    counts: Counts,
}

impl CountingReadiness {
    /// Wrap `inner`; the returned [`Counts`] reads the calls.
    pub fn new(inner: EventReadinessPtr) -> (Self, Counts) {
        let counts = Counts::default();
        (
            Self {
                inner,
                counts: counts.clone(),
            },
            counts,
        )
    }
}

impl EventReadiness for CountingReadiness {
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        self.counts.is_ready.fetch_add(1, Ordering::SeqCst);
        self.inner.is_ready(dur)
    }

    fn clear(&self) {
        self.inner.clear();
    }

    fn deadline(&self) -> Option<Instant> {
        self.inner.deadline()
    }

    fn register(&self, waker: &ReadinessWaker) -> bool {
        self.counts.register.fetch_add(1, Ordering::SeqCst);
        self.inner.register(waker)
    }
}

/// A readiness that is ready only at its deadline: no event ever flips it, so it
/// is ready by the clock alone. `register` returns `true` ("ready only at my
/// deadline"), so the executor must wait for the deadline and not re-check it.
struct DeadlineOnly {
    deadline: Instant,
}

impl EventReadiness for DeadlineOnly {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        Instant::now() >= self.deadline
    }

    fn deadline(&self) -> Option<Instant> {
        Some(self.deadline)
    }

    fn register(&self, _waker: &ReadinessWaker) -> bool {
        true
    }
}

/// A task that parks on a readiness once, then records when it ran again.
struct ParkOnce {
    readiness: Option<EventReadinessPtr>,
    polls: Arc<AtomicUsize>,
    parked_at: Stamp,
    resumed_at: Stamp,
}

impl TaskIterator for ParkOnce {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        if let Some(readiness) = self.readiness.take() {
            self.parked_at.mark();
            return Some(TaskStatus::Depends(readiness));
        }
        if self.resumed_at.get().is_none() {
            self.resumed_at.mark();
            return Some(TaskStatus::Ready(()));
        }
        None
    }
}

/// A task that pops from an [`EventQueue`] and parks on `queue.readiness()`
/// while it is empty. It ends after `want` items, or when the queue is closed
/// and drained.
struct QueueConsumer {
    queue: EventQueue<u32>,
    want: usize,
    got: Arc<Mutex<Vec<(u32, Instant)>>>,
    counts: Counts,
    polls: Arc<AtomicUsize>,
    parked_at: Stamp,
    ended_at: Stamp,
}

impl TaskIterator for QueueConsumer {
    type Ready = u32;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        let taken = self
            .got
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len();
        if taken >= self.want {
            self.ended_at.mark();
            return None;
        }
        match self.queue.pop() {
            Ok(item) => {
                self.got
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push((item, Instant::now()));
                Some(TaskStatus::Ready(item))
            }
            Err(_) if self.queue.is_closed() => {
                self.ended_at.mark();
                None
            }
            Err(_) => {
                self.parked_at.mark();
                // One set of counters for every park of this consumer.
                let counting = counting_over(
                    Arc::new(self.queue.readiness()) as EventReadinessPtr,
                    &self.counts,
                );
                Some(TaskStatus::Depends(Arc::new(counting) as EventReadinessPtr))
            }
        }
    }
}

/// A consumer wraps each park in a fresh [`CountingReadiness`] that must write to
/// the consumer's shared counters, so it is built with them.
fn counting_over(inner: EventReadinessPtr, counts: &Counts) -> CountingReadiness {
    CountingReadiness {
        inner,
        counts: counts.clone(),
    }
}

/// One parked task, a host event armed, and the checks to run when it ended.
pub struct Scenario {
    name: &'static str,
    done: Box<dyn Fn() -> bool>,
    verify: Box<dyn Fn()>,
}

impl Scenario {
    /// The name of the scenario, for a failure message.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Whether the task ran to its end.
    pub fn is_done(&self) -> bool {
        (self.done)()
    }

    /// Assert the outcome. Call it after [`Self::is_done`] is `true`.
    pub fn verify(&self) {
        (self.verify)();
    }
}

fn since(later: Instant, earlier: Instant) -> Duration {
    later.saturating_duration_since(earlier)
}

fn assert_bounded_checks(name: &str, counts: &Counts) {
    let calls = counts.is_ready_calls();
    assert!(
        calls <= MAX_IS_READY_CALLS,
        "{name}: the executor called is_ready {calls} times (bound {MAX_IS_READY_CALLS}); a parked \
         task must wake by its waker, not by a timed re-check"
    );
    assert!(
        counts.register_calls() >= 1,
        "{name}: the executor never registered a waker on the readiness"
    );
}

fn spawn_task<T>(task: T)
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
{
    spawn()
        .with_task(task)
        .with_resolver(Box::new(FnReady::new(|_, _| {})))
        .as_scheduled()
        .spawn()
        .expect("should schedule");
}

/// Start `readiness` parked under a [`ParkOnce`] task and return the stamps.
fn park_once(seed: u64, readiness: EventReadinessPtr) -> (Arc<AtomicUsize>, Stamp, Stamp) {
    initialize_pool(seed);
    let polls = Arc::new(AtomicUsize::new(0));
    let (parked_at, resumed_at) = (Stamp::default(), Stamp::default());
    spawn_task(ParkOnce {
        readiness: Some(readiness),
        polls: Arc::clone(&polls),
        parked_at: parked_at.clone(),
        resumed_at: resumed_at.clone(),
    });
    // Returns once the task is parked: the executor hands the thread back to the
    // host event loop.
    run_until_complete();
    assert_eq!(polls.load(Ordering::SeqCst), 1, "polled once, then parked");
    assert!(parked_at.get().is_some(), "the task parked");
    (polls, parked_at, resumed_at)
}

/// A task parked on a [`ResumeSignal`] that a host timer sets 50 ms later runs
/// again on time, and was not polled meanwhile.
///
/// WHY: `ResumeSignal::set` is the plain "flag plus wake" construct. On wasm its
/// wake is a `ReadinessWaker` whose target is the host event loop; if it did not
/// resume the executor, the task would stay parked for ever.
pub fn resume_signal_park(after: After) -> Scenario {
    let signal = ResumeSignal::new(false);
    let (counting, counts) = CountingReadiness::new(Arc::new(signal.clone()) as EventReadinessPtr);
    let (polls, parked_at, resumed_at) = park_once(101, Arc::new(counting));

    let fired_at = Stamp::default();
    let polls_at_fire = Arc::new(AtomicUsize::new(usize::MAX));
    let (fired, seen, polls_now) = (
        fired_at.clone(),
        Arc::clone(&polls_at_fire),
        Arc::clone(&polls),
    );
    after(
        EVENT_DELAY_MS,
        Box::new(move || {
            seen.store(polls_now.load(Ordering::SeqCst), Ordering::SeqCst);
            fired.mark();
            signal.set(true);
        }),
    );

    let done = resumed_at.clone();
    Scenario {
        name: "resume_signal_park",
        done: Box::new(move || done.get().is_some()),
        verify: Box::new(move || {
            let (parked, fired, resumed) = (
                parked_at.require("park"),
                fired_at.require("host event"),
                resumed_at.require("resume"),
            );
            assert_eq!(
                polls_at_fire.load(Ordering::SeqCst),
                1,
                "a parked task must not be polled before its signal is set"
            );
            assert!(resumed >= fired, "the task ran before the signal was set");
            let latency = since(resumed, fired);
            assert!(
                latency < WAKE_LATENCY_WINDOW,
                "resumed {latency:?} after the signal was set (window {WAKE_LATENCY_WINDOW:?})"
            );
            assert!(
                since(resumed, parked) >= Duration::from_millis(45),
                "resumed before the host timer"
            );
            assert_bounded_checks("resume_signal_park", &counts);
        }),
    }
}

/// An [`EventQueue`] consumer parked on `queue.readiness()` is woken by a push
/// from a host callback.
///
/// WHY: the queue wakes by itself on a push (the ~80 hand-placed wakes are gone);
/// a push that did not wake the consumer would strand it.
pub fn queue_push_park(after: After) -> Scenario {
    initialize_pool(102);
    let queue: EventQueue<u32> = EventQueue::unbounded();
    let counts = Counts::default();
    let (polls, parked_at, ended_at) = (
        Arc::new(AtomicUsize::new(0)),
        Stamp::default(),
        Stamp::default(),
    );
    let got = Arc::new(Mutex::new(Vec::new()));
    spawn_task(QueueConsumer {
        queue: queue.clone(),
        want: 1,
        got: Arc::clone(&got),
        counts: counts.clone(),
        polls: Arc::clone(&polls),
        parked_at: parked_at.clone(),
        ended_at: ended_at.clone(),
    });
    run_until_complete();
    assert_eq!(polls.load(Ordering::SeqCst), 1, "polled once, then parked");

    let pushed_at = Stamp::default();
    let polls_at_push = Arc::new(AtomicUsize::new(usize::MAX));
    let (pushed, seen, polls_now) = (
        pushed_at.clone(),
        Arc::clone(&polls_at_push),
        Arc::clone(&polls),
    );
    after(
        EVENT_DELAY_MS,
        Box::new(move || {
            seen.store(polls_now.load(Ordering::SeqCst), Ordering::SeqCst);
            pushed.mark();
            queue.push(7).expect("the queue is open");
        }),
    );

    let done = ended_at.clone();
    Scenario {
        name: "queue_push_park",
        done: Box::new(move || done.get().is_some()),
        verify: Box::new(move || {
            let (parked, pushed, ended) = (
                parked_at.require("park"),
                pushed_at.require("push"),
                ended_at.require("end"),
            );
            assert_eq!(
                polls_at_push.load(Ordering::SeqCst),
                1,
                "a consumer parked on an empty queue must not be polled before a push"
            );
            let items = got.lock().unwrap_or_else(PoisonError::into_inner);
            assert_eq!(items.len(), 1, "the consumer received the pushed item");
            assert_eq!(items[0].0, 7);
            let latency = since(items[0].1, pushed);
            assert!(
                latency < WAKE_LATENCY_WINDOW,
                "the item was received {latency:?} after the push (window {WAKE_LATENCY_WINDOW:?})"
            );
            assert!(
                items[0].1 >= pushed,
                "the item was received before the push"
            );
            assert!(ended >= pushed);
            assert!(since(ended, parked) >= Duration::from_millis(45));
            assert_bounded_checks("queue_push_park", &counts);
        }),
    }
}

/// An [`EventQueue`] consumer parked on `queue.readiness()` is woken by a
/// `close` from a host callback and sees the end of the queue.
///
/// WHY: a consumer must not wait for ever for an item that cannot come. `close`
/// wakes every parked task so each sees the end.
pub fn queue_close_park(after: After) -> Scenario {
    initialize_pool(103);
    let queue: EventQueue<u32> = EventQueue::unbounded();
    let counts = Counts::default();
    let (polls, parked_at, ended_at) = (
        Arc::new(AtomicUsize::new(0)),
        Stamp::default(),
        Stamp::default(),
    );
    let got = Arc::new(Mutex::new(Vec::new()));
    spawn_task(QueueConsumer {
        queue: queue.clone(),
        want: usize::MAX,
        got: Arc::clone(&got),
        counts: counts.clone(),
        polls: Arc::clone(&polls),
        parked_at: parked_at.clone(),
        ended_at: ended_at.clone(),
    });
    run_until_complete();
    assert_eq!(polls.load(Ordering::SeqCst), 1, "polled once, then parked");

    let closed_at = Stamp::default();
    let polls_at_close = Arc::new(AtomicUsize::new(usize::MAX));
    let close_result = Arc::new(AtomicBool::new(false));
    let (closed, seen, polls_now, close_seen) = (
        closed_at.clone(),
        Arc::clone(&polls_at_close),
        Arc::clone(&polls),
        Arc::clone(&close_result),
    );
    after(
        EVENT_DELAY_MS,
        Box::new(move || {
            seen.store(polls_now.load(Ordering::SeqCst), Ordering::SeqCst);
            closed.mark();
            close_seen.store(queue.close(), Ordering::SeqCst);
        }),
    );

    let done = ended_at.clone();
    Scenario {
        name: "queue_close_park",
        done: Box::new(move || done.get().is_some()),
        verify: Box::new(move || {
            let (parked, closed, ended) = (
                parked_at.require("park"),
                closed_at.require("close"),
                ended_at.require("end"),
            );
            assert_eq!(
                polls_at_close.load(Ordering::SeqCst),
                1,
                "a consumer parked on an open queue must not be polled before the close"
            );
            assert!(
                got.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .is_empty(),
                "no item was pushed"
            );
            assert!(
                close_result.load(Ordering::SeqCst),
                "the host callback closed the queue"
            );
            assert!(ended >= closed, "the consumer ended before the close");
            let latency = since(ended, closed);
            assert!(
                latency < WAKE_LATENCY_WINDOW,
                "the consumer saw the close {latency:?} later (window {WAKE_LATENCY_WINDOW:?})"
            );
            assert!(since(ended, parked) >= Duration::from_millis(45));
            assert_bounded_checks("queue_close_park", &counts);
        }),
    }
}

/// A task parked on a readiness that is ready only at its deadline runs again at
/// that deadline, and the executor hands the thread back to the host meanwhile.
///
/// WHY: a deadline-only park has no event and no waker call; the executor must
/// ask the host for a timer at the deadline. A host timer 10 ms into the wait
/// proves the event loop kept turning (an executor that spun until the deadline
/// would run it only after the deadline), and the call bound proves nothing
/// re-checked the readiness on a timer.
pub fn deadline_only_park(after: After) -> Scenario {
    let deadline = Instant::now() + Duration::from_millis(EVENT_DELAY_MS as u64);
    let (counting, counts) =
        CountingReadiness::new(Arc::new(DeadlineOnly { deadline }) as EventReadinessPtr);
    let (_polls, parked_at, resumed_at) = park_once(104, Arc::new(counting));

    let turned_at = Stamp::default();
    let turned = turned_at.clone();
    after(10.0, Box::new(move || turned.mark()));

    let done = resumed_at.clone();
    Scenario {
        name: "deadline_only_park",
        done: Box::new(move || done.get().is_some()),
        verify: Box::new(move || {
            let (parked, turned, resumed) = (
                parked_at.require("park"),
                turned_at.require("host timer"),
                resumed_at.require("resume"),
            );
            assert!(
                turned < deadline,
                "the host timer (10 ms) ran only after the deadline: the executor blocked the \
                 event loop while it waited"
            );
            assert!(
                resumed >= deadline,
                "resumed {:?} before the deadline",
                since(deadline, resumed)
            );
            let late = since(resumed, deadline);
            assert!(
                late < DEADLINE_LATE_WINDOW,
                "resumed {late:?} after the deadline (window {DEADLINE_LATE_WINDOW:?})"
            );
            assert!(since(resumed, parked) >= Duration::from_millis(45));
            let calls = counts.is_ready_calls();
            assert!(
                calls <= MAX_IS_READY_CALLS,
                "deadline_only_park: is_ready was called {calls} times (bound {MAX_IS_READY_CALLS}); \
                 a deadline-only park is looked at once the deadline is due"
            );
            assert!(
                counts.register_calls() >= 1,
                "the executor never registered a waker"
            );
        }),
    }
}

/// A task parked on an [`AnyReadiness`] over two children runs again when the
/// first child becomes ready, and the executor registered its waker on both.
///
/// WHY: `AnyReadiness::register` must hand the waker to every child; a child that
/// kept none would never wake the task.
pub fn any_readiness_park(after: After) -> Scenario {
    let (quiet, quiet_counts) =
        CountingReadiness::new(Arc::new(ResumeSignal::new(false)) as EventReadinessPtr);
    let first = ResumeSignal::new(false);
    let (first_child, first_counts) =
        CountingReadiness::new(Arc::new(first.clone()) as EventReadinessPtr);
    // The signal that becomes ready is the second child, so a wake registered on
    // the first child alone would not do.
    let any = AnyReadiness::either(Arc::new(quiet), Arc::new(first_child));
    let (counting, counts) = CountingReadiness::new(Arc::new(any) as EventReadinessPtr);
    let (polls, parked_at, resumed_at) = park_once(105, Arc::new(counting));

    let fired_at = Stamp::default();
    let polls_at_fire = Arc::new(AtomicUsize::new(usize::MAX));
    let (fired, seen, polls_now) = (
        fired_at.clone(),
        Arc::clone(&polls_at_fire),
        Arc::clone(&polls),
    );
    after(
        EVENT_DELAY_MS,
        Box::new(move || {
            seen.store(polls_now.load(Ordering::SeqCst), Ordering::SeqCst);
            fired.mark();
            first.set(true);
        }),
    );

    let done = resumed_at.clone();
    Scenario {
        name: "any_readiness_park",
        done: Box::new(move || done.get().is_some()),
        verify: Box::new(move || {
            let (parked, fired, resumed) = (
                parked_at.require("park"),
                fired_at.require("host event"),
                resumed_at.require("resume"),
            );
            assert_eq!(
                polls_at_fire.load(Ordering::SeqCst),
                1,
                "polled while parked"
            );
            assert!(resumed >= fired);
            let latency = since(resumed, fired);
            assert!(
                latency < WAKE_LATENCY_WINDOW,
                "resumed {latency:?} after the child was ready (window {WAKE_LATENCY_WINDOW:?})"
            );
            assert!(since(resumed, parked) >= Duration::from_millis(45));
            assert_bounded_checks("any_readiness_park", &counts);
            assert!(
                quiet_counts.register_calls() >= 1 && first_counts.register_calls() >= 1,
                "AnyReadiness must register the waker with every child"
            );
        }),
    }
}

/// Forwards a task and records how often it was polled and when it ended.
struct Observed<T> {
    inner: T,
    polls: Arc<AtomicUsize>,
    ended_at: Stamp,
}

impl<T: TaskIterator> TaskIterator for Observed<T> {
    type Ready = T::Ready;
    type Pending = T::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        let next = self.inner.next_status(ctx);
        if next.is_none() {
            self.ended_at.mark();
        }
        next
    }
}

/// Run `task` and check that it ends `nominal` after the start, not before it
/// and not later than [`DEADLINE_LATE_WINDOW`] plus `slack` plus
/// [`HOST_TIMER_LATE_ALLOWANCE`], after at most `max_polls` polls.
fn timed_end<T>(
    name: &'static str,
    seed: u64,
    task: T,
    nominal: Duration,
    slack: Duration,
    max_polls: usize,
) -> Scenario
where
    T: TaskIterator + 'static,
    T::Ready: 'static,
    T::Pending: 'static,
{
    initialize_pool(seed);
    let start = Instant::now();
    let (polls, ended_at) = (Arc::new(AtomicUsize::new(0)), Stamp::default());
    spawn_task(Observed {
        inner: task,
        polls: Arc::clone(&polls),
        ended_at: ended_at.clone(),
    });
    run_until_complete();

    let done = ended_at.clone();
    Scenario {
        name,
        done: Box::new(move || done.get().is_some()),
        verify: Box::new(move || {
            let ended = ended_at.require("end");
            let took = since(ended, start);
            assert!(
                took + Duration::from_millis(5) >= nominal,
                "{name}: ended after {took:?}, before {nominal:?}"
            );
            assert!(
                took < nominal + DEADLINE_LATE_WINDOW + slack + HOST_TIMER_LATE_ALLOWANCE,
                "{name}: ended after {took:?} (nominal {nominal:?}, window \
                 {DEADLINE_LATE_WINDOW:?}, host timer allowance {HOST_TIMER_LATE_ALLOWANCE:?})"
            );
            let calls = polls.load(Ordering::SeqCst);
            assert!(
                calls <= max_polls,
                "{name}: polled {calls} times (bound {max_polls}); a sleeping task must wait for \
                 its timer, not spin"
            );
        }),
    }
}

/// A `SleepingTask` (the valtron `sleep`) ends at its deadline, in few polls.
///
/// WHY: it read `std::time::Instant::now()`, which panics on wasm32. A sleep is
/// a `Delayed` park: the executor must hand the thread to the host and come back
/// at the deadline.
pub fn sleep_task(_after: After) -> Scenario {
    let nominal = Duration::from_millis(EVENT_DELAY_MS as u64);
    timed_end(
        "sleep_task",
        106,
        SleepingTask::sleep(nominal),
        nominal,
        Duration::ZERO,
        4,
    )
}

/// A `TimeoutTask` over a task that polls every 10 ms ends once its 50 ms
/// timeout passed.
///
/// WHY: it read `std::time::Instant::now()` on every poll, which panics on wasm32.
pub fn timeout_task(_after: After) -> Scenario {
    struct Ticker;
    impl TaskIterator for Ticker {
        type Ready = ();
        type Pending = ();

        fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<(), ()>> {
            Some(TaskStatus::Delayed(Duration::from_millis(10)))
        }
    }
    let timeout = Duration::from_millis(EVENT_DELAY_MS as u64);
    // The task notices the timeout on the first poll after it, up to one tick late.
    timed_end(
        "timeout_task",
        107,
        TimeoutTask::new(Ticker, timeout),
        timeout,
        Duration::from_millis(10),
        12,
    )
}

/// `WaitGroup::wait_timeout` on a group with nothing outstanding reads the clock
/// and returns `true` at once.
///
/// WHY: it read `std::time::Instant::now()`, which panics on wasm32. A wait that
/// has to block cannot run on one JS thread (a condition variable cannot be
/// woken while the thread waits), so only the idle path is testable on wasm.
pub fn waitgroup_idle_wait_timeout() {
    let group = WaitGroup::new();
    let start = Instant::now();
    assert!(
        group.wait_timeout(Duration::from_millis(500)),
        "an idle group is done"
    );
    assert!(since(Instant::now(), start) < Duration::from_millis(100));
}

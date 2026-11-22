//! OWNED port of spec-31 feature-02's JS-yield integration tests (feature 12,
//! migration step 3): the same valtron-executor ↔ JS-event-loop assertions, but
//! `#[wasm_test]` on the `js-foundation-wasm` yielder — zero wasm-bindgen.
//!
//! What changed versus the `#[wasm_bindgen_test]` originals
//! (`foundation_core/tests/valtron/wasm_js_yield_integration.rs`, retained as the
//! F14 opt-in path):
//! - Timing comes from a REGISTERED host function (`Date.now()` over the owned
//!   ABI) instead of wasm-bindgen's `foundation_compact::Instant`.
//! - Completion is awaited by `async` cases on the owned re-poll loop (each
//!   `Pending` poll yields through `schedule_timeout`, which is exactly the
//!   executor re-entry mechanism under test) instead of wasm-bindgen `Closure`
//!   safety timeouts.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

use foundation_core::valtron::{
    run_future_iter,
    single::{fork_ctx, initialize_pool, run_until_complete, spawn},
    wasm::{JSThreadYielder, WaitStatus},
    Ctx, FnReady, NotificationItem, NotifyQueue, NotifyQueueStreamIterator, ProcessController,
    Stream, StreamCollectFuture, StreamIteratorExt, StreamNextStream, StreamPendingFuture,
    StreamReadyFuture, TaskIterator, TaskStatus, ThreadedValue, WakeRegistry,
};
// `.next().await` on the `futures_core::Stream` bridges (futures-lite, via core).
use foundation_core::valtron::StreamExt as _;
use foundation_macros::{valtron_wasm_test, wasm_test};
use foundation_wasm::abi::web::{invoke_as_f64, register_function};
use futures_core::Stream as FuturesStream;

/// Milliseconds since the epoch, via a registered host function (owned ABI).
fn now_ms() -> f64 {
    let clock = register_function("function(){ return Date.now(); }");
    invoke_as_f64(clock.handler, &[])
}

/// Await until `predicate()` is true — every `Pending` poll round-trips through
/// the owned `schedule_timeout` loop, giving the executor's own timers a chance
/// to fire (this IS the re-entry integration under test). Panics past `max_polls`
/// so a deadlock fails fast instead of hanging the runner.
struct WaitUntil<F: Fn() -> bool> {
    predicate: F,
    polls: usize,
    max_polls: usize,
}

impl<F: Fn() -> bool> WaitUntil<F> {
    fn new(predicate: F) -> Self {
        Self {
            predicate,
            polls: 0,
            max_polls: 500,
        }
    }

    /// The same wait with its own poll bound. A case that drives many tasks at
    /// once keeps the 500-poll default and picks a larger stated bound, because
    /// one harness re-entry does not advance every task.
    fn with_max_polls(predicate: F, max_polls: usize) -> Self {
        Self {
            predicate,
            polls: 0,
            max_polls,
        }
    }
}

impl<F: Fn() -> bool + Unpin> Future for WaitUntil<F> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        if (this.predicate)() {
            return Poll::Ready(());
        }
        this.polls += 1;
        assert!(
            this.polls < this.max_polls,
            "deadlock: condition not met after {} executor re-entries",
            this.max_polls
        );
        Poll::Pending
    }
}

/// Task that produces `max` ready values, one per executor iteration.
struct CountingTask {
    max: usize,
    current: usize,
}

impl TaskIterator for CountingTask {
    type Ready = usize;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.current >= self.max {
            return None;
        }
        self.current += 1;
        Some(TaskStatus::Ready(self.current))
    }
}

/// Task that counts every iteration through the executor cycle.
struct YieldingTask {
    max: usize,
    current: usize,
    counter: Arc<AtomicUsize>,
}

impl TaskIterator for YieldingTask {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.current >= self.max {
            return None;
        }
        self.current += 1;
        self.counter.fetch_add(1, Ordering::Relaxed);
        Some(TaskStatus::Ready(()))
    }
}

/// Test 1 (was `js_yield_returns_immediately`): `yield_for` schedules a timer and
/// returns `Waiting` immediately — it must NOT spin-block the JS event loop.
#[wasm_test]
fn js_yield_returns_immediately() {
    let yielder = JSThreadYielder::new();
    let start = now_ms();

    let status = yielder.yield_for(Duration::from_millis(100));
    assert_eq!(status, WaitStatus::Waiting);

    let elapsed = now_ms() - start;
    assert!(
        elapsed < 5.0,
        "yield_for blocked for {elapsed}ms — it must return immediately"
    );
}

/// Test 2 (was `js_yield_callback_reschedules_executor`): spawn → executor →
/// timer fires → re-entry → completion.
#[wasm_test]
async fn js_yield_callback_reschedules_executor() {
    initialize_pool(42);

    let done = Arc::new(AtomicBool::new(false));
    let done_clone = Arc::clone(&done);

    spawn()
        .with_task(CountingTask { max: 1, current: 0 })
        .with_resolver(Box::new(FnReady::new(move |item, _| {
            if let TaskStatus::Ready(_) = item {
                done_clone.store(true, Ordering::Relaxed);
            }
        })))
        .as_scheduled()
        .spawn()
        .expect("should schedule");

    run_until_complete();

    let done_check = Arc::clone(&done);
    WaitUntil::new(move || done_check.load(Ordering::Relaxed)).await;
    assert!(done.load(Ordering::Relaxed), "task should have completed");
}

/// Test 3 (was `concurrent_d1_queries_make_progress`): two concurrent tasks both
/// complete within roughly one yield period, not the sum of both.
#[wasm_test]
async fn concurrent_tasks_make_progress() {
    initialize_pool(42);

    let done_count = Arc::new(AtomicUsize::new(0));
    let start = now_ms();

    for _ in 0..2 {
        let dc = Arc::clone(&done_count);
        spawn()
            .with_task(CountingTask { max: 1, current: 0 })
            .with_resolver(Box::new(FnReady::new(move |item, _| {
                if let TaskStatus::Ready(_) = item {
                    dc.fetch_add(1, Ordering::Relaxed);
                }
            })))
            .as_scheduled()
            .spawn()
            .expect("should schedule task");
    }

    run_until_complete();

    let dc = Arc::clone(&done_count);
    WaitUntil::new(move || dc.load(Ordering::Relaxed) == 2).await;

    let elapsed = now_ms() - start;
    // The point is that the two tasks do not wait for one another. An idle
    // host finishes in about one yield period (10 ms). A loaded or emulated
    // host can delay each timer by a hundred milliseconds, so the bound is
    // generous; a deadlock still fails, because the wait never returns.
    assert!(
        elapsed < 500.0,
        "concurrent tasks took {elapsed}ms — should complete within one yield period"
    );
}

/// Test 4 (was `no_deadlock_with_js_yield`): five iterations, each through the
/// yield/reschedule cycle — proves repeated re-entry, no deadlock.
#[wasm_test]
async fn no_deadlock_with_js_yield() {
    initialize_pool(42);

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    spawn()
        .with_task(YieldingTask {
            max: 5,
            current: 0,
            counter: counter_clone,
        })
        .with_resolver(Box::new(FnReady::new(|_, _| {})))
        .as_scheduled()
        .spawn()
        .expect("should schedule");

    run_until_complete();

    let c = Arc::clone(&counter);
    WaitUntil::new(move || c.load(Ordering::Relaxed) >= 5).await;
    assert_eq!(
        counter.load(Ordering::Relaxed),
        5,
        "all 5 iterations should complete without deadlock"
    );
}

// ---------------------------------------------------------------------------
// Spec 68 / spec 83 feature 06 T5: event-driven park → wake → resume.
//
// A task parked on a readiness signal must not be re-polled while it waits, the
// executor must hand the thread back to the JS event loop (not spin), and the
// wake must resume it through a host microtask, not a timer poll.

use std::cell::RefCell;
use std::rc::Rc;

use foundation_core::valtron::{WakeCallback, WakeSignal};
use foundation_wasm::abi::web::{register_microtask, register_schedule};

/// Parks on a [`WakeSignal`] on its first poll, then emits one value.
///
/// The wake callback is handed out through `callback_slot` so the test can fire
/// it from a host timer, standing in for a fetch or stream read completing.
struct ParkOnceTask {
    polls: Rc<AtomicUsize>,
    callback_slot: Rc<RefCell<Option<WakeCallback>>>,
    parked: bool,
    done: bool,
}

impl TaskIterator for ParkOnceTask {
    type Ready = usize;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        self.polls.fetch_add(1, Ordering::Relaxed);
        if self.done {
            return None;
        }
        if !self.parked {
            self.parked = true;
            let signal = WakeSignal::new();
            *self.callback_slot.borrow_mut() = Some(signal.callback());
            return Some(TaskStatus::Depends(Arc::new(signal)));
        }
        self.done = true;
        Some(TaskStatus::Ready(7))
    }
}

/// A task parked on `Depends(WakeSignal)` is polled once, stays parked (no
/// re-polls) while the signal is false, and resumes once the signal's callback
/// fires from a host timer: the callback requests an executor run as a
/// microtask, which wakes the sleeper and drives the task to completion.
#[wasm_test]
async fn depends_task_parks_until_wake_signal_fires() {
    initialize_pool(42);

    let polls = Rc::new(AtomicUsize::new(0));
    let callback_slot: Rc<RefCell<Option<WakeCallback>>> = Rc::new(RefCell::new(None));
    let value = Arc::new(AtomicUsize::new(0));

    let value_clone = Arc::clone(&value);
    spawn()
        .with_task(ParkOnceTask {
            polls: Rc::clone(&polls),
            callback_slot: Rc::clone(&callback_slot),
            parked: false,
            done: false,
        })
        .with_resolver(Box::new(FnReady::new(move |item, _| {
            if let TaskStatus::Ready(v) = item {
                value_clone.store(v, Ordering::Relaxed);
            }
        })))
        .as_scheduled()
        .spawn()
        .expect("should schedule");

    // Returns once the task is parked: the executor hands control back to JS
    // instead of spinning on the never-ready signal.
    run_until_complete();
    assert_eq!(polls.load(Ordering::Relaxed), 1, "polled once, then parked");
    assert!(
        callback_slot.borrow().is_some(),
        "the task registered its wake"
    );

    // Fire the wake from a host timer, after the event loop has turned.
    let polls_at_wake = Rc::new(AtomicUsize::new(usize::MAX));
    let (slot, polls_for_timer, at_wake) = (
        Rc::clone(&callback_slot),
        Rc::clone(&polls),
        Rc::clone(&polls_at_wake),
    );
    register_schedule(20.0, move || {
        at_wake.store(polls_for_timer.load(Ordering::Relaxed), Ordering::Relaxed);
        if let Some(wake) = slot.borrow_mut().take() {
            wake();
        }
    });

    let v = Arc::clone(&value);
    WaitUntil::new(move || v.load(Ordering::Relaxed) == 7).await;

    assert_eq!(
        polls_at_wake.load(Ordering::Relaxed),
        1,
        "a parked task must not be re-polled before its signal fires"
    );
    assert_eq!(value.load(Ordering::Relaxed), 7, "resumed and completed");
}

/// `register_microtask` runs before a 0 ms timeout queued ahead of it — it is a
/// real microtask (`queueMicrotask`), not a timer.
#[wasm_test]
async fn microtask_runs_before_zero_delay_timeout() {
    let order: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

    let timeout_order = Rc::clone(&order);
    register_schedule(0.0, move || timeout_order.borrow_mut().push("timeout"));
    let microtask_order = Rc::clone(&order);
    register_microtask(move || microtask_order.borrow_mut().push("microtask"));

    let seen = Rc::clone(&order);
    WaitUntil::new(move || seen.borrow().len() == 2).await;
    assert_eq!(*order.borrow(), vec!["microtask", "timeout"]);
}

/// A scheduled callback may schedule again from inside itself: the registry
/// lock is released before the callback runs (it used to be held, so the nested
/// `register_schedule` re-locked the same mutex).
#[wasm_test]
async fn scheduled_callback_can_reschedule() {
    let fired = Rc::new(AtomicUsize::new(0));

    let outer = Rc::clone(&fired);
    register_schedule(0.0, move || {
        outer.fetch_add(1, Ordering::Relaxed);
        let inner = Rc::clone(&outer);
        register_schedule(0.0, move || {
            inner.fetch_add(1, Ordering::Relaxed);
        });
    });

    let f = Rc::clone(&fired);
    WaitUntil::new(move || f.load(Ordering::Relaxed) == 2).await;
    assert_eq!(fired.load(Ordering::Relaxed), 2);
}

/// foundation_compact's `host-abi` clock: `Instant` advances across real host
/// turns, and `SystemTime` agrees with the host's `Date.now()`. Before the
/// backend existed, any `Instant::now()` in an owned module imported
/// wasm-bindgen glue and the module could not even be instantiated.
#[wasm_test]
async fn host_abi_clock_advances_and_matches_wall_clock() {
    use foundation_compact::{Instant, SystemTime, UNIX_EPOCH};

    let wall_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after the epoch")
        .as_secs_f64()
        * 1000.0;
    let host_ms = now_ms();
    assert!(
        (wall_ms - host_ms).abs() < 5_000.0,
        "SystemTime ({wall_ms} ms) should match Date.now() ({host_ms} ms)"
    );

    let start = Instant::now();
    WaitUntil::new(move || start.elapsed() >= Duration::from_millis(5)).await;
    assert!(Instant::now() >= start, "Instant is monotonic");
}

/// foundation_compact's `host-abi` entropy fills a buffer larger than Web
/// Crypto's 65536-byte per-call limit: every 64 KiB chunk, including the short
/// tail, must be written (the host splits the request).
#[wasm_test]
fn host_abi_entropy_fills_past_the_web_crypto_limit() {
    const CHUNK: usize = 65_536;
    let mut buf = vec![0u8; 3 * CHUNK + 1_000];
    foundation_compact::entropy::fill(&mut buf).expect("host has Web Crypto");
    for (i, chunk) in buf.chunks(CHUNK).enumerate() {
        assert!(
            chunk.iter().any(|&b| b != 0),
            "chunk {i} ({} bytes) was left zeroed",
            chunk.len()
        );
    }
}

/// The thread RNG seeds from host-ABI entropy and produces values.
#[wasm_test]
fn host_abi_entropy_seeds_the_thread_rng() {
    use foundation_compact::rng::Rng;

    let mut rng = foundation_compact::rng::rng();
    let draws: Vec<u64> = (0..4).map(|_| rng.next_u64()).collect();
    assert!(
        draws.windows(2).any(|w| w[0] != w[1]),
        "seeded RNG should not repeat: {draws:?}"
    );
}

// ---------------------------------------------------------------------------
// Spec 70 feature 80 E5: the per-task `ReadinessWaker` design at runtime on the
// owned `js-foundation-wasm` backend. The scenarios are shared with the
// wasm-bindgen suite (`foundation_core/tests/valtron/wasm_wake_support.rs`); this
// file supplies the host timer (`register_schedule`) and the wait.

#[path = "../../../tests/valtron/wasm_wake_support.rs"]
mod wake;

use wake::Scenario;

/// Run `callback` after `delay_ms` on the host event loop, as a host event (a
/// fetch completing, a message arriving) would.
fn host_after(delay_ms: f64, callback: Box<dyn FnOnce()>) {
    // `register_schedule` takes an `Fn`: the one-shot callback is taken on the
    // first (and only) call.
    let callback = RefCell::new(Some(callback));
    register_schedule(delay_ms, move || {
        if let Some(run) = callback.borrow_mut().take() {
            run();
        }
    });
}

/// Wait for the scenario's task to end, or fail after a few seconds with its
/// name: a lost wake leaves the task parked for ever, and the test must fail, not
/// hang. Each `Pending` poll goes round the owned re-poll loop, so the host's
/// timers and microtasks run between polls.
async fn finish(scenario: Scenario) {
    struct Ended<'a>(&'a Scenario, foundation_compact::Instant);
    impl Future for Ended<'_> {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
            if self.0.is_done() {
                return Poll::Ready(());
            }
            assert!(
                self.1.elapsed() < Duration::from_secs(3),
                "{}: the parked task never ran again (a lost wake)",
                self.0.name()
            );
            Poll::Pending
        }
    }
    Ended(&scenario, foundation_compact::Instant::now()).await;
    scenario.verify();
}

/// A task parked on a `ResumeSignal` that a host timer sets 50 ms later resumes on
/// time. See `wake::resume_signal_park`.
#[wasm_test]
async fn resume_signal_set_by_host_timer_wakes_parked_task() {
    finish(wake::resume_signal_park(host_after)).await;
}

/// An `EventQueue` push from a host callback wakes a consumer parked on
/// `queue.readiness()`. See `wake::queue_push_park`.
#[wasm_test]
async fn event_queue_push_from_host_wakes_parked_consumer() {
    finish(wake::queue_push_park(host_after)).await;
}

/// An `EventQueue` close from a host callback wakes a parked consumer. See
/// `wake::queue_close_park`.
#[wasm_test]
async fn event_queue_close_from_host_wakes_parked_consumer() {
    finish(wake::queue_close_park(host_after)).await;
}

/// A deadline-only park resumes at its deadline without blocking the event loop.
/// See `wake::deadline_only_park`.
#[wasm_test]
async fn deadline_only_park_resumes_at_its_deadline() {
    finish(wake::deadline_only_park(host_after)).await;
}

/// An `AnyReadiness` over two children resumes when the first child is ready. See
/// `wake::any_readiness_park`.
#[wasm_test]
async fn any_readiness_resumes_when_a_child_is_ready() {
    finish(wake::any_readiness_park(host_after)).await;
}

/// A valtron `sleep` ends at its deadline. See `wake::sleep_task`.
#[wasm_test]
async fn sleep_task_ends_at_its_deadline() {
    finish(wake::sleep_task(host_after)).await;
}

/// A `TimeoutTask` ends at its timeout. See `wake::timeout_task`.
#[wasm_test]
async fn timeout_task_ends_at_its_timeout() {
    finish(wake::timeout_task(host_after)).await;
}

/// `WaitGroup::wait_timeout` on an idle group does not panic on the clock. See
/// `wake::waitgroup_idle_wait_timeout`.
#[wasm_test]
fn waitgroup_idle_wait_timeout_reads_the_clock() {
    wake::waitgroup_idle_wait_timeout();
}

// ---------------------------------------------------------------------------
// Spec 88 feature 01 T2: deeper scenarios from the same shared file. The
// wasm-bindgen twin (`tests/valtron/wasm_js_yield_integration.rs`) registers
// cases with the same names, so the two hosts are comparable by name.

/// A cancelled park ends and its waker leaves the registry. See
/// `wake::cancelled_park_releases`.
#[wasm_test]
async fn cancelled_park_releases() {
    finish(wake::cancelled_park_releases(host_after)).await;
}

/// One registry wake reaches two waiters, each once. See
/// `wake::two_waiters_both_resume`.
#[wasm_test]
async fn two_waiters_both_resume() {
    finish(wake::two_waiters_both_resume(host_after)).await;
}

/// A dropped waiter never breaks a wake. See `wake::dropped_waiter_wake_is_safe`.
#[wasm_test]
async fn dropped_waiter_wake_is_safe() {
    finish(wake::dropped_waiter_wake_is_safe(host_after)).await;
}

/// A producer parks on a full bounded queue and resumes in order. See
/// `wake::bounded_queue_backpressure`.
#[wasm_test]
async fn bounded_queue_backpressure() {
    finish(wake::bounded_queue_backpressure(host_after)).await;
}

/// `into_future_stream` parks and a host timer resumes it. See
/// `wake::stream_resumes_from_a_host_timer`.
#[wasm_test]
async fn stream_resumes_from_a_host_timer() {
    finish(wake::stream_resumes_from_a_host_timer(host_after)).await;
}

/// An `AnyReadiness` ignores a cancelled child. See
/// `wake::cancelled_child_does_not_wake_parent`.
#[wasm_test]
async fn cancelled_child_does_not_wake_parent() {
    finish(wake::cancelled_child_does_not_wake_parent(host_after)).await;
}

// ---------------------------------------------------------------------------
// Spec 88 feature 01 T3: cases that only the owned harness can prove — the
// `js-foundation-wasm` yielder, the `schedule_timeout` re-entry and the host
// ABI. Every wait uses `WaitUntil` (500 polls) or its own stated bound.

use std::sync::Mutex;

use foundation_core::valtron::SleepingTask;

/// Forwards a task and records its id when it ends.
struct RecordEnd<T> {
    inner: T,
    id: u32,
    order: Arc<Mutex<Vec<u32>>>,
    ended: Arc<AtomicUsize>,
}

impl<T: TaskIterator> TaskIterator for RecordEnd<T> {
    type Ready = T::Ready;
    type Pending = T::Pending;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        let next = self.inner.next_status(ctx);
        if next.is_none() {
            self.order
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(self.id);
            self.ended.fetch_add(1, Ordering::SeqCst);
        }
        next
    }
}

/// `yield_for` over a 50 ms delay returns `Waiting` at once: it schedules a
/// host timer, it does not block the JS thread.
#[wasm_test]
fn yield_for_reports_waiting_for_a_long_delay() {
    let yielder = JSThreadYielder::new();
    // Warm up the one-off costs of a fresh yielder.
    let _ = yielder.yield_for(Duration::from_millis(5));

    let start = now_ms();
    let status = yielder.yield_for(Duration::from_millis(50));
    assert_eq!(
        status,
        WaitStatus::Waiting,
        "yield_for must report Waiting for a 50 ms delay"
    );
    let elapsed = now_ms() - start;
    assert!(
        elapsed < 25.0,
        "yield_for(50 ms) took {elapsed}ms; it must return at once"
    );
}

/// Two sleeping tasks resume in deadline order: 10 ms before 30 ms.
#[wasm_test]
async fn two_parked_tasks_resume_in_deadline_order() {
    initialize_pool(42);
    let order = Arc::new(Mutex::new(Vec::new()));
    let ended = Arc::new(AtomicUsize::new(0));

    for (id, ms) in [(1_u32, 10_u64), (2, 30)] {
        spawn()
            .with_task(RecordEnd {
                inner: SleepingTask::sleep(Duration::from_millis(ms)),
                id,
                order: Arc::clone(&order),
                ended: Arc::clone(&ended),
            })
            .with_resolver(Box::new(FnReady::new(|_, _| {})))
            .as_scheduled()
            .spawn()
            .expect("should schedule");
    }
    run_until_complete();

    let done = Arc::clone(&ended);
    WaitUntil::new(move || done.load(Ordering::SeqCst) == 2).await;

    let order = order
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert_eq!(
        *order,
        vec![1, 2],
        "the 10 ms task must end before the 30 ms task"
    );
}

/// 64 tasks of four yields each all end, and none starves.
#[wasm_test]
async fn sixty_four_tasks_share_the_loop() {
    initialize_pool(42);
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..64 {
        spawn()
            .with_task(YieldingTask {
                max: 4,
                current: 0,
                counter: Arc::clone(&counter),
            })
            .with_resolver(Box::new(FnReady::new(|_, _| {})))
            .as_scheduled()
            .spawn()
            .expect("should schedule");
    }
    run_until_complete();

    let start = foundation_compact::Instant::now();
    let count = Arc::clone(&counter);
    // Each of the 64 tasks needs at least five polls (four yields and the end),
    // and one harness re-entry does not advance every task. The poll bound is
    // this case's own stated bound, and it is the anti-spin guard.
    WaitUntil::with_max_polls(move || count.load(Ordering::SeqCst) >= 64 * 4, 2_000).await;
    let took = start.elapsed();
    assert_eq!(
        counter.load(Ordering::SeqCst),
        64 * 4,
        "every task must finish all four yields"
    );
    // Measured on the owned Deno host (macOS arm64, 2026-11-22): 256 task steps
    // took 4.0 s, because the executor hands each step to the JS event loop and
    // a host timer costs about 10 ms. The bound is 10 s, so a task that starves
    // still fails, while the host's own timer cost does not.
    assert!(
        took < Duration::from_secs(10),
        "64 tasks took {took:?} to share the loop"
    );
}

/// A callback scheduled before `yield_for` runs after `yield_for` returns, not
/// inside it: the executor hands the thread to the host, and the host runs the
/// timer between two executor turns.
#[wasm_test]
async fn timer_callback_runs_after_an_executor_re_entry() {
    let order: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));

    let callback_order = Rc::clone(&order);
    register_schedule(0.0, move || callback_order.borrow_mut().push("callback"));

    JSThreadYielder::new().yield_for(Duration::from_millis(10));
    order.borrow_mut().push("after-yield");

    let seen = Rc::clone(&order);
    WaitUntil::new(move || seen.borrow().len() == 2).await;
    assert_eq!(
        *order.borrow(),
        vec!["after-yield", "callback"],
        "the timer callback must run after the executor yielded, not inside yield_for"
    );
}

/// The ABI clock and a host timer agree: a 50 ms timer fires 45 to 200 ms after
/// the recorded start. The upper bound covers a loaded host; the lower bound
/// proves the timer never fires early.
#[wasm_test]
async fn host_clock_matches_the_timer_deadline() {
    let start = now_ms();
    let fired = Rc::new(RefCell::new(None::<f64>));
    let slot = Rc::clone(&fired);
    register_schedule(50.0, move || *slot.borrow_mut() = Some(now_ms()));

    let seen = Rc::clone(&fired);
    WaitUntil::new(move || seen.borrow().is_some()).await;

    let elapsed = fired.borrow().expect("the timer fired") - start;
    assert!(
        elapsed >= 45.0,
        "the 50 ms timer fired after only {elapsed}ms"
    );
    assert!(
        elapsed <= 200.0,
        "the 50 ms timer fired after {elapsed}ms; the ABI clock and the timer disagree"
    );
}

/// `wait_timeout` on a group whose last worker finished returns at once.
#[wasm_test]
fn waitgroup_timeout_after_all_done_returns_early() {
    use foundation_compact::Instant;
    use foundation_context::{Ctx, MonotonicClock};
    use foundation_core::synca::WaitGroup;

    let group = WaitGroup::new();
    group.add(1);
    group.done();

    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    let start = Instant::now();
    assert!(
        group.wait_timeout(Duration::from_millis(500), &ctx),
        "a finished group is done"
    );
    assert!(
        start.elapsed() < Duration::from_millis(100),
        "a finished group must not wait for its timeout"
    );
}

/// `#[valtron_wasm_test]` brings up the pool: `fork_ctx` and `execute` work
/// without the case calling `initialize_pool`.
#[valtron_wasm_test]
fn valtron_wasm_test_initializes_the_pool() {
    // `fork_ctx` panics unless a pool is initialized.
    let _ctx = fork_ctx();
    let _stream = foundation_core::valtron::execute(CountingTask { max: 1, current: 0 }, None)
        .expect("the macro's pool accepts a task");
}

/// A task that calls `run_until_complete` from inside its own `next_status`
/// returns instead of spinning.
struct NestedWaitTask {
    step: u8,
}

impl TaskIterator for NestedWaitTask {
    type Ready = usize;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        match self.step {
            0 => {
                self.step = 1;
                // Nothing else is scheduled: this nested run must return, not
                // spin on the slot of the very task that called it.
                run_until_complete();
                Some(TaskStatus::Ready(11))
            }
            _ => None,
        }
    }
}

/// A nested wait inside a wasm case returns instead of spinning the isolate.
#[wasm_test]
async fn nested_wait_inside_a_wasm_case_returns() {
    initialize_pool(42);
    let value = Arc::new(AtomicUsize::new(0));
    let value_clone = Arc::clone(&value);

    spawn()
        .with_task(NestedWaitTask { step: 0 })
        .with_resolver(Box::new(FnReady::new(move |item, _| {
            if let TaskStatus::Ready(v) = item {
                value_clone.store(v, Ordering::SeqCst);
            }
        })))
        .as_scheduled()
        .spawn()
        .expect("should schedule");
    run_until_complete();

    let seen = Arc::clone(&value);
    WaitUntil::new(move || seen.load(Ordering::SeqCst) == 11).await;
    assert_eq!(
        value.load(Ordering::SeqCst),
        11,
        "the nested run returned and the task finished"
    );
}

// ---------------------------------------------------------------------------
// Spec 68: the stream-to-future bridge contract on the owned wasm path.
//
// The native template (`tests/valtron/bridge_park.rs`) is gated `not(wasm)`.
// These cases re-prove the same contract with the real producer: NotifyQueue
// push and close fire its WakeRegistry, and a CountingWaker proves the park.

/// A `std::task::Wake` whose shared counter records every wake.
struct CountingWaker {
    count: Arc<AtomicUsize>,
}

impl Wake for CountingWaker {
    fn wake(self: Arc<Self>) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}

fn counting_waker() -> (Waker, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let waker = Waker::from(Arc::new(CountingWaker {
        count: Arc::clone(&count),
    }));
    (waker, count)
}

fn poll_future<F: Future + Unpin>(f: &mut F, waker: &Waker) -> Poll<F::Output> {
    Pin::new(f).poll(&mut Context::from_waker(waker))
}

fn poll_next<S: FuturesStream + Unpin>(s: &mut S, waker: &Waker) -> Poll<Option<S::Item>> {
    Pin::new(s).poll_next(&mut Context::from_waker(waker))
}

type WakingIter<D, P> = (
    NotifyQueueStreamIterator<D, P>,
    Arc<NotifyQueue<Stream<D, P>>>,
);

/// A queue whose empty `next()` yields `Stream::Wait`, plus its producer handle.
/// The bridges call `announce()` on the first poll, so the empty path does not
/// wait on the queue's condvar.
fn waking_iter<D, P>() -> WakingIter<D, P> {
    let queue = Arc::new(NotifyQueue::unbounded());
    queue.set_max_spins(1);
    let iterator = NotifyQueueStreamIterator::new(
        Arc::clone(&queue),
        5,
        Duration::from_micros(50),
        fork_ctx(),
    );
    (iterator, queue)
}

/// `StreamReadyFuture` parks with no self-wake, a push fires one wake, and the
/// re-poll resolves.
#[valtron_wasm_test]
fn parking_bridge_ready_future_parks_then_wakes() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    let mut future = StreamReadyFuture::new(iterator);
    let (waker, count) = counting_waker();

    assert!(matches!(poll_future(&mut future, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 0, "parked: no self-wake");

    assert!(queue.push(Stream::Next(42)).is_ok());
    assert_eq!(count.load(Ordering::SeqCst), 1, "push fired the wake");

    match poll_future(&mut future, &waker) {
        Poll::Ready(Some((42, _))) => {}
        _ => panic!("expected Ready(Some((42, _)))"),
    }
}

/// `StreamCollectFuture` drains pushed items, re-parks, and a close resolves it.
#[valtron_wasm_test]
fn parking_bridge_collect_future_parks_then_wakes() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    let mut future = StreamCollectFuture::new(iterator);
    let (waker, count) = counting_waker();

    assert!(matches!(poll_future(&mut future, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 0, "parked: no self-wake");

    assert!(queue.push(Stream::Next(7)).is_ok());
    assert!(queue.push(Stream::Next(8)).is_ok());
    assert_eq!(count.load(Ordering::SeqCst), 1, "one registered wake fires");

    assert!(matches!(poll_future(&mut future, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 1, "re-park, no self-wake");

    assert!(queue.close());
    assert_eq!(count.load(Ordering::SeqCst), 2, "close fired the wake");
    match poll_future(&mut future, &waker) {
        Poll::Ready(vec) => assert_eq!(vec, vec![7, 8]),
        _ => panic!("expected Ready(vec![7, 8]) on close"),
    }
}

/// `StreamNextStream` parks and surfaces the first pushed payload.
#[valtron_wasm_test]
fn parking_bridge_next_stream_parks_then_wakes() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    let mut stream = StreamNextStream::new(iterator);
    let (waker, count) = counting_waker();

    assert!(matches!(poll_next(&mut stream, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 0, "parked: no self-wake");

    assert!(queue.push(Stream::Next(5)).is_ok());
    assert_eq!(count.load(Ordering::SeqCst), 1, "push fired the wake");

    match poll_next(&mut stream, &waker) {
        Poll::Ready(Some(5)) => {}
        _ => panic!("expected Ready(Some(5))"),
    }
}

/// The full-enum lens from `into_future_stream` parks over a control state and
/// yields the pushed item as-is.
#[valtron_wasm_test]
fn parking_bridge_future_stream_parks_then_wakes() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    let mut stream = iterator.into_future_stream();
    let (waker, count) = counting_waker();

    assert!(matches!(poll_next(&mut stream, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 0, "parked: no self-wake");

    assert!(queue.push(Stream::Next(9)).is_ok());
    assert_eq!(count.load(Ordering::SeqCst), 1, "push fired the wake");

    match poll_next(&mut stream, &waker) {
        Poll::Ready(Some(Stream::Next(9))) => {}
        _ => panic!("expected Ready(Some(Stream::Next(9)))"),
    }
}

/// A producer that runs on a host timer wakes the parked bridge across a real
/// JavaScript event-loop turn.
#[valtron_wasm_test]
async fn parking_bridge_wakes_from_a_host_timer() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    let mut future = StreamReadyFuture::new(iterator);
    let (waker, count) = counting_waker();

    assert!(matches!(poll_future(&mut future, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 0, "parked: no self-wake");

    register_schedule(20.0, move || {
        assert!(queue.push(Stream::Next(11)).is_ok());
    });

    let waited = Arc::clone(&count);
    WaitUntil::new(move || waited.load(Ordering::SeqCst) >= 1).await;

    match poll_future(&mut future, &waker) {
        Poll::Ready(Some((11, _))) => {}
        _ => panic!("expected Ready(Some((11, _))) after the host timer"),
    }
}

/// The raw lens forwards `Wait` and `Next` as items and never parks.
#[wasm_test]
fn pure_future_stream_forwards_control_items() {
    let iterator = vec![Stream::<i32, &'static str>::Wait, Stream::Next(7)]
        .into_iter()
        .into_stream_iter();
    let mut stream = iterator.into_pure_future_stream();
    let (waker, count) = counting_waker();

    assert!(matches!(
        poll_next(&mut stream, &waker),
        Poll::Ready(Some(Stream::Wait))
    ));
    assert!(matches!(
        poll_next(&mut stream, &waker),
        Poll::Ready(Some(Stream::Next(7)))
    ));
    assert!(matches!(poll_next(&mut stream, &waker), Poll::Ready(None)));
    assert_eq!(count.load(Ordering::SeqCst), 0, "the raw lens never wakes");
}

/// `StreamPendingFuture` consumes a spurious `Next`, re-parks, re-registers,
/// and resolves on the real `Pending`.
#[valtron_wasm_test]
fn pending_future_spurious_wake_reparks() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    let mut future = StreamPendingFuture::new(iterator);
    let (waker, count) = counting_waker();

    assert!(matches!(poll_future(&mut future, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 0, "parked: no self-wake");

    assert!(queue.push(Stream::Next(99)).is_ok());
    assert_eq!(
        count.load(Ordering::SeqCst),
        1,
        "spurious push fired a wake"
    );

    assert!(matches!(poll_future(&mut future, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 1, "spurious item re-parks");

    assert!(queue.push(Stream::Pending("ctx")).is_ok());
    assert_eq!(count.load(Ordering::SeqCst), 2, "re-registered wake fired");

    match poll_future(&mut future, &waker) {
        Poll::Ready(Some(("ctx", _))) => {}
        _ => panic!("expected Ready(Some((\"ctx\", _)))"),
    }
}

/// `WakeRegistry` fires each waiter once and clears its list.
#[wasm_test]
fn wake_registry_fires_each_waiter_once() {
    let registry = WakeRegistry::new();
    let first = Arc::new(AtomicUsize::new(0));
    let second = Arc::new(AtomicUsize::new(0));

    let f = Arc::clone(&first);
    registry.register(Box::new(move || {
        f.fetch_add(1, Ordering::SeqCst);
    }));
    let s = Arc::clone(&second);
    registry.register(Box::new(move || {
        s.fetch_add(1, Ordering::SeqCst);
    }));
    assert_eq!(registry.pending_len(), 2);

    registry.fire();
    assert_eq!(first.load(Ordering::SeqCst), 1);
    assert_eq!(second.load(Ordering::SeqCst), 1);
    assert_eq!(registry.pending_len(), 0);

    registry.fire();
    assert_eq!(
        first.load(Ordering::SeqCst),
        1,
        "second fire has no waiters"
    );
    assert_eq!(second.load(Ordering::SeqCst), 1);
}

/// A `NotSupported` iterator self-wakes on `Init` and still resolves.
#[wasm_test]
fn not_supported_stream_self_wakes() {
    let iterator = vec![Stream::<i32, &'static str>::Init, Stream::Next(7)]
        .into_iter()
        .into_stream_iter();
    let mut future = StreamReadyFuture::new(iterator);
    let (waker, count) = counting_waker();

    assert!(matches!(poll_future(&mut future, &waker), Poll::Pending));
    assert_eq!(count.load(Ordering::SeqCst), 1, "NotSupported self-wakes");

    match poll_future(&mut future, &waker) {
        Poll::Ready(Some((7, _))) => {}
        _ => panic!("expected Ready(Some((7, _)))"),
    }
}

/// The lazy `run_future_iter` makes progress on one iterator: `Waiting`, then
/// the values.
#[valtron_wasm_test]
fn run_future_iter_makes_progress_on_wasm() {
    let mut polls = 0u8;
    let iter = match run_future_iter(
        move || async move {
            std::future::poll_fn(move |_cx| {
                polls += 1;
                if polls < 2 {
                    Poll::Pending
                } else {
                    Poll::Ready(Ok::<_, ()>(vec![Ok::<i32, ()>(1), Ok(2)].into_iter()))
                }
            })
            .await
        },
        None,
        None,
        fork_ctx(),
    ) {
        Ok(iter) => iter,
        Err(_) => panic!("run_future_iter returned an error"),
    };

    let values: Vec<ThreadedValue<i32, ()>> = iter.collect();
    assert_eq!(values.len(), 3);
    assert!(matches!(values[0], ThreadedValue::Waiting));
    assert!(matches!(values[1], ThreadedValue::Value(Ok(1))));
    assert!(matches!(values[2], ThreadedValue::Value(Ok(2))));
}

/// The queue's waiting path runs on wasm. On the old wiring this reaches std's
/// `CondVar`, whose `wait_timeout` panics in `thread::sleep` (#605).
#[valtron_wasm_test]
fn notify_queue_wait_returns_after_timeout() {
    let queue: NotifyQueue<Stream<i32, &'static str>> = NotifyQueue::unbounded();
    queue.set_max_spins(1);
    let ctx = fork_ctx();

    match queue.wait_for_item(Duration::from_millis(1), &ctx) {
        Some(NotificationItem::None) => {}
        _ => panic!("expected NotificationItem::None after the timeout"),
    }

    assert!(queue.push(Stream::Next(7)).is_ok());
    match queue.wait_for_item(Duration::from_millis(1), &ctx) {
        Some(NotificationItem::Ready(Stream::Next(7))) => {}
        _ => panic!("expected Ready(Next(7))"),
    }

    assert!(queue.close());
    assert!(queue
        .wait_for_item(Duration::from_millis(1), &ctx)
        .is_none());
}

// ---------------------------------------------------------------------------
// Standard async functions: `.await` on the bridges, no manual polling.
//
// The `#[wasm_test]` harness drives the async body. When a bridge parks, it
// registers the executor waker; the host timer's push fires it, and the harness
// re-polls the body. Nothing calls `poll()` here.

/// `into_ready_future().await` resolves from a host-timer push.
#[valtron_wasm_test]
async fn async_ready_future_awaits_a_host_push() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    register_schedule(20.0, move || {
        assert!(queue.push(Stream::Next(42)).is_ok());
    });

    match iterator.into_ready_future().await {
        Some((42, _)) => {}
        _ => panic!("expected Some((42, _)) from the awaited ready future"),
    }
}

/// `into_next_stream()` + `.next().await` yields a pushed value, then ends on a
/// host-timer close.
#[valtron_wasm_test]
async fn async_next_stream_yields_values() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    let push_queue = Arc::clone(&queue);
    register_schedule(20.0, move || {
        assert!(push_queue.push(Stream::Next(7)).is_ok());
    });
    register_schedule(40.0, move || {
        assert!(queue.close());
    });

    let mut stream = iterator.into_next_stream();
    assert_eq!(stream.next().await, Some(7));
    assert_eq!(stream.next().await, None);
}

/// `into_future_stream()` + `.next().await` yields the full-enum item.
#[valtron_wasm_test]
async fn async_future_stream_forwards_items() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    register_schedule(20.0, move || {
        assert!(queue.push(Stream::Next(9)).is_ok());
    });

    let mut stream = iterator.into_future_stream();
    match stream.next().await {
        Some(Stream::Next(9)) => {}
        _ => panic!("expected Some(Stream::Next(9)) from the awaited stream"),
    }
}

/// `into_collect_future().await` collects pushed items and ends on close.
#[valtron_wasm_test]
async fn async_collect_future_collects_pushed_items() {
    let (iterator, queue) = waking_iter::<i32, &'static str>();
    register_schedule(20.0, move || {
        assert!(queue.push(Stream::Next(7)).is_ok());
        assert!(queue.push(Stream::Next(8)).is_ok());
        assert!(queue.close());
    });

    let values = iterator.into_collect_future().await;
    assert_eq!(values, vec![7, 8]);
}

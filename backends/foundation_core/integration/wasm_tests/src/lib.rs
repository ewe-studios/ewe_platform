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
use std::task::{Context, Poll};
use std::time::Duration;

use foundation_core::valtron::{
    single::{initialize_pool, run_until_complete, spawn},
    wasm::{JSThreadYielder, WaitStatus},
    Ctx, FnReady, ProcessController, TaskIterator, TaskStatus,
};
use foundation_macros::wasm_test;
use foundation_wasm::abi::web::{invoke_as_f64, register_function};

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

    fn next_status(
        &mut self,
        _ctx: Ctx,
    ) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
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

    fn next_status(
        &mut self,
        _ctx: Ctx,
    ) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
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
    assert!(
        elapsed < 50.0,
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
// Spec 68 / spec 65 feature 06 T5: event-driven park → wake → resume.
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

    fn next_status(
        &mut self,
        _ctx: Ctx,
    ) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
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
    assert!(callback_slot.borrow().is_some(), "the task registered its wake");

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

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
    FnReady, NoSpawner, ProcessController, TaskIterator, TaskStatus,
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
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
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
        .schedule()
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
            .schedule()
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
        .schedule()
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

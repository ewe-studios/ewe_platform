//! WHY: `#[valtron]` / `#[valtron_test]` (`foundation_macros`, re-exported from
//! `foundation_core::valtron`) must actually deliver their contract: the engine
//! is LIVE for the whole wrapped body (the `PoolGuard` from
//! `initialize_pool(seed, threads)` outlives it), return values pass through,
//! and the test variant defaults to the `Some(3)`-minimum thread count.
//!
//! WHAT: Spawn-and-complete through a macro-initialized engine; explicit
//! `seed`/`threads` arguments; `Result`-returning bodies; `#[valtron]` on a
//! plain entry-point-style fn.
//!
//! HOW: Each case relies ONLY on the macro for engine setup — no manual
//! `initialize_pool` — and drives tasks through the unified `sync_one` helper
//! (which exists on both the multi and single executor surfaces, matching
//! whichever pool the macro's `initialize_pool` brought up). A green run
//! therefore proves the guard wiring under the active feature set.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{
    sync_collect_one, sync_one, valtron, valtron_test, NoSpawner, TaskIterator, TaskStatus,
};

/// Produces `max` ready values, one per executor iteration.
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

/// Drive a 3-iteration task to completion on whatever engine the surrounding
/// macro initialized; returns the Ready values the executor produced.
fn run_counting_task() -> Vec<usize> {
    sync_one(CountingTask { max: 3, current: 0 })
        .expect("engine must be initialized by the macro")
}

#[valtron_test]
fn engine_is_live_for_the_whole_body() {
    // No manual initialize_pool anywhere — if the macro didn't set the engine up
    // (or dropped the guard early), sync_one's schedule step would fail.
    assert_eq!(run_counting_task(), vec![1, 2, 3]);
}

#[valtron_test(seed = 42, threads = 4)]
fn explicit_seed_and_threads_are_accepted() {
    assert_eq!(run_counting_task(), vec![1, 2, 3]);
}

#[valtron_test(threads = 1)]
fn thread_count_below_the_minimum_is_clamped_not_rejected() {
    // The macro passes `Some(1)` straight through; the multi engine clamps it up
    // to its ≥ 3 minimum (≥ 2 task workers after a background worker is carved
    // off) rather than panicking — the case still runs on a properly-sized engine.
    assert_eq!(run_counting_task(), vec![1, 2, 3]);
}

#[valtron_test]
fn result_returning_bodies_pass_their_output_through() -> Result<(), String> {
    // `?`/`return` semantics must survive the wrapping (the body becomes an
    // inner FN, not a closure) — and the harness must see the Ok.
    if run_counting_task() != vec![1, 2, 3] {
        return Err("task did not complete".into());
    }
    Ok(())
}

/// An entry-point-style fn: `#[valtron]` brings the engine up around it and
/// hands back the body's return value.
#[valtron(seed = 7, threads = 3)]
fn entry_point_style() -> Vec<usize> {
    run_counting_task()
}

#[test]
fn valtron_entry_point_wraps_and_returns() {
    assert_eq!(entry_point_style(), vec![1, 2, 3]);
}

// ============================================================================
// Feature 00-F3: `#[valtron]` / `#[valtron_test]` accept `async fn`
// ============================================================================

/// A future that yields once — it self-wakes on the first poll (`Pending`) and
/// completes on the second. This exercises the park→wake path the async macro
/// drives via `block_on_future` on both the single and multi executors.
struct YieldOnce(bool);

impl Future for YieldOnce {
    type Output = u32;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.0 {
            Poll::Ready(7)
        } else {
            self.0 = true;
            cx.waker().wake_by_ref(); // arrange the wake, then park
            Poll::Pending
        }
    }
}

#[valtron_test]
async fn async_body_awaits_and_completes() {
    // The macro drives this async body to completion; `.await` on a parking
    // future must resume (proves feature 00-F1 parking under the macro).
    let ready = core::future::ready(41u32).await;
    let yielded = YieldOnce(false).await;
    assert_eq!(ready + yielded, 48);
}

#[valtron_test(seed = 9, threads = 3)]
async fn async_body_preserves_question_and_return() -> Result<(), String> {
    let v = YieldOnce(false).await;
    if v != 7 {
        // `return` exits the future, whose output is the fn's return value.
        return Err("yield-once did not complete".into());
    }
    // `?` propagates through the async body to the fn's `Result` output.
    let parsed: u32 = "3".parse().map_err(|_| "parse failed".to_string())?;
    assert_eq!(parsed, 3);
    Ok(())
}

/// `#[valtron]` (entry-point style) also accepts an `async fn` and returns the
/// awaited value.
#[valtron(seed = 5, threads = 3)]
async fn async_entry_point() -> u32 {
    YieldOnce(false).await + core::future::ready(1u32).await
}

#[test]
fn valtron_async_entry_point_returns() {
    assert_eq!(async_entry_point(), 8);
}

// ============================================================================
// Panic → channel.close(): a panicking task must unblock the caller, not wedge
// ============================================================================

/// A task that panics immediately when the executor polls it.
///
/// Used to verify that valtron closes the result channel on panic (the fix in
/// `task_iters.rs`). Without that fix the channel stays open forever, the
/// collector blocks indefinitely, and the test silently hangs.
struct PanickingTask;

impl TaskIterator for PanickingTask {
    type Ready = u32;
    type Pending = ();
    type Spawner = NoSpawner;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        panic!("intentional panic from within a valtron task — this MUST be caught");
    }
}

/// WHY: A panicking valtron task used to leave its result channel open, so
/// `collect_one` / `sync_collect_one` / `block_on_future` blocked forever.
/// That turned a failing assertion inside a task into a silent test hang
/// instead of a `FAILED` line — the harness never even printed `test result:`.
/// The fix in `task_iters.rs` calls `channel.close()` + `alive.take()` in every
/// `Panicked` arm so the collector unblocks.
///
/// WHAT: Schedule `PanickingTask` via `sync_collect_one` and assert it
/// returns `Err` (channel closed, no ready value). A watchdog thread catches the
/// regression case (hang) so CI cannot wedge silently.
///
/// HOW: `#[valtron_test]` initialises a multi-worker pool; one worker blocks in
/// `sync_collect_one`, another picks up `PanickingTask` and panics → channel
/// closes → the blocked worker wakes and sees `Err`.
#[valtron_test]
fn panicking_task_closes_channel_and_unblocks_caller() {
    // Watchdog: if this test ever hangs (because someone reverted the
    // channel.close() fix), the watchdog fires after 10s and panics so the
    // test runner sees a failure instead of a hung process.
    let done = Arc::new(AtomicBool::new(false));
    let done_clone = Arc::clone(&done);

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        if !done_clone.load(Ordering::SeqCst) {
            panic!(
                "WATCHDOG: panicking task hung for 10s — \
                 valtron may have regressed the panic-channel-close fix in task_iters.rs"
            );
        }
    });

    let result = sync_collect_one(PanickingTask);
    done.store(true, Ordering::SeqCst);

    assert!(
        result.is_err(),
        "panicking task should unblock the caller with Err, got {result:?}"
    );
}

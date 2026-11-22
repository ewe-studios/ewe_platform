//! Feature 06 / spec 88 feature 01 T4: the `StateTask`, `AsyncStateTask` and
//! `#[task]` contract on a build that runs on native AND on a JS-hosted wasm
//! host.
//!
//! This file holds the cases that use only `TaskStatus`, the generated struct and
//! the executor. The blocking sync-boundary cases (`sync_one`) stay in
//! `state_task.rs`, which is native only, because those helpers do not exist on
//! JS-hosted wasm.
//!
//! The two cases at the bottom run on both targets through [`dual!`]: a
//! `#[valtron_test]` natively and a `#[valtron_bindgen]` async test on wasm, so
//! the executor's `Ready`/`Pending`/`Ready` and pre-poll-cancel paths are proven
//! on Deno, Chromium and workerd.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use foundation_core::valtron::{
    execute, task, AsyncStateTask, AsyncTransition, CancelOutcome, CancellableFutureTask,
    StateTask, StreamExt as _, StreamIteratorExt, TaskIterator, TaskStatus,
};

use foundation_context::Ctx;

// ============================================================================
// Core: StateTask (sync)
// ============================================================================

/// A three-step countdown driven manually: `Ready(3)`, `Ready(2)`, `Ready(1)`,
/// end.
///
/// The transition is a plain `FnMut` closure — no macro — proving `StateTask`
/// forwards the engine `Ctx` and the current state on every poll, and that a
/// single machine emits `Ready` once *per step* (not once ever).
#[test]
fn sync_state_task_drives_a_multi_state_machine() {
    let ctx = Ctx::host();
    let mut task: StateTask<u32, u32, (), _> = StateTask::new(3, |_ctx, n| {
        if n == 0 {
            None
        } else {
            Some((n - 1, TaskStatus::Ready(n)))
        }
    });

    // `Ready` is NOT terminal: each step's `Ready(n)` threads `n - 1` back into
    // the machine, so it emits three `Ready`s before `None` ends it.
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ready(3)));
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ready(2)));
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ready(1)));
    assert_eq!(task.next_status(ctx.clone()), None);
    assert_eq!(task.next_status(ctx.clone()), None, "stays terminal");
}

/// Non-`Ready` statuses (`Pending`, `Delayed`, `Spawn`, …) pass straight through
/// unchanged and thread the next state; `Ready` does too — only `None` ends the
/// machine.
#[test]
fn sync_state_task_passes_non_ready_statuses_through() {
    let ctx = Ctx::host();
    let mut task: StateTask<u32, u32, &'static str, _> = StateTask::new(0, |_ctx, n| match n {
        0 => Some((1, TaskStatus::Pending("waiting"))),
        1 => Some((2, TaskStatus::Ready(42))),
        _ => None,
    });

    assert_eq!(
        task.next_status(ctx.clone()),
        Some(TaskStatus::Pending("waiting"))
    );
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ready(42)));
    assert_eq!(task.next_status(ctx.clone()), None);
}

// ============================================================================
// Core: AsyncStateTask (async) — park on `Depends`, resume on wake
// ============================================================================

/// A future that stashes its waker and stays `Pending` until an external
/// `ready` flag is set — mirrors the `WakeParkFuture` used by the
/// `FutureTask` bridge tests (feature 01), so the async core's park/resume
/// semantics are compared against the established seam.
struct WakeParkFuture {
    poll_count: Arc<AtomicUsize>,
    ready: Arc<AtomicBool>,
    waker_slot: Arc<Mutex<Option<Waker>>>,
}

impl Future for WakeParkFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_count.fetch_add(1, Ordering::SeqCst);
        if self.ready.load(Ordering::SeqCst) {
            Poll::Ready(())
        } else {
            *self.waker_slot.lock().unwrap() = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

/// A single-value state machine's state: `Run(v)` still has a value to emit,
/// `Done` ends it — the explicit terminal variant, no extra `bool` flag.
enum RunOnce {
    Run(u32),
    Done,
}

/// WHY: The async variant must park (not busy-poll) when its in-flight future
///      returns `Pending` without a self-wake, and complete only once an
///      external `wake()` lands.
///
/// WHAT: The first poll emits `Depends` over a not-ready signal and polls the
///       future exactly once; after `wake()` the signal flips ready and the next
///       poll completes — total two polls.
///
/// HOW: Drive `AsyncStateTask::next_status` directly (the executor consumes
///      exactly this) and inspect the returned readiness signal.
#[test]
fn async_state_task_parks_then_resumes_on_wake() {
    let ctx = Ctx::host();
    let poll_count = Arc::new(AtomicUsize::new(0));
    let ready = Arc::new(AtomicBool::new(false));
    let waker_slot: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));

    // The transition awaits the wake-park future, emits `Ready(value)`, then
    // advances to `RunOnce::Done` — so the following step (`Done => None`) is
    // terminal. The `Done` variant is the explicit "done" marker: no extra flag.
    let transition: AsyncTransition<RunOnce, u32, ()> = {
        let poll_count = poll_count.clone();
        let ready = ready.clone();
        let waker_slot = waker_slot.clone();
        Box::new(move |_ctx, state: RunOnce| {
            let poll_count = poll_count.clone();
            let ready = ready.clone();
            let waker_slot = waker_slot.clone();
            Box::pin(async move {
                let RunOnce::Run(value) = state else {
                    return None;
                };
                WakeParkFuture {
                    poll_count,
                    ready,
                    waker_slot,
                }
                .await;
                Some((RunOnce::Done, TaskStatus::Ready(value)))
            })
        })
    };

    let mut task = AsyncStateTask::new(RunOnce::Run(42), transition);

    // First poll: the future parks → `Depends` over a not-ready signal.
    let signal = match task.next_status(ctx.clone()) {
        Some(TaskStatus::Depends(signal)) => signal,
        other => panic!("expected Depends on first poll, got {other:?}"),
    };
    assert_eq!(
        poll_count.load(Ordering::SeqCst),
        1,
        "polled once, then parks"
    );

    // Park predicate stays false while blocked — the executor would NOT re-run
    // the task, so the turn count is flat (no busy-poll).
    for _ in 0..1000 {
        assert!(
            !signal.is_ready(None),
            "parked signal must remain not-ready"
        );
    }
    assert_eq!(
        poll_count.load(Ordering::SeqCst),
        1,
        "no re-poll while parked"
    );

    // Wake: set the result and fire the stashed waker — the signal flips ready.
    ready.store(true, Ordering::SeqCst);
    waker_slot
        .lock()
        .unwrap()
        .take()
        .expect("waker stashed")
        .wake();
    assert!(
        signal.is_ready(None),
        "wake() makes the signal ready to re-run"
    );

    // The unparked re-run drains the stale token, re-polls, and completes.
    match task.next_status(ctx.clone()) {
        Some(TaskStatus::Ready(42)) => {}
        other => panic!("expected Ready(42) after wake, got {other:?}"),
    }
    assert_eq!(
        poll_count.load(Ordering::SeqCst),
        2,
        "exactly two polls total"
    );
    assert_eq!(task.next_status(ctx.clone()), None, "task is complete");
}

// ============================================================================
// Macro: `#[valtron::task]` — sync surface
// ============================================================================

/// A countdown that emits `Ignore` for the intermediate steps and a single
/// final `Ready(1)`, then terminates by returning `None`.
#[expect(
    clippy::needless_pass_by_value,
    reason = "the #[task] macro generates a fn(Ctx, T) transition, so ctx is by value"
)]
#[task]
pub(crate) fn count_down(ctx: Ctx, n: u32) -> Option<(u32, TaskStatus<u32, ()>)> {
    let _ = ctx;
    match n {
        0 => None,
        1 => Some((0, TaskStatus::Ready(1))),
        k => Some((k - 1, TaskStatus::Ignore)),
    }
}

/// WHY: `#[task]` on a `fn` must generate a struct (same name) with `new(T)`
///      and drive the machine to `Ready` while keeping the transition callable.
///
/// WHAT: `count_down::new(3)` yields `Ignore`, `Ignore`, `Ready(1)`, then ends;
///       `count_down(ctx, n)` still runs as the raw transition.
#[test]
fn task_macro_sync_drives_to_ready_and_stays_callable() {
    let ctx = Ctx::host();

    // The transition itself remains directly callable (value namespace).
    assert_eq!(count_down(ctx.clone(), 5), Some((4, TaskStatus::Ignore)));

    // The generated struct drives the machine through its states.
    let mut task = count_down::new(3);
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ignore));
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ignore));
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ready(1)));
    assert_eq!(task.next_status(ctx.clone()), None);
}

// ============================================================================
// Macro: `#[valtron::task]` — async surface
// ============================================================================

/// A future that self-wakes once, then completes — exercises the async
/// transition's `Ignore` requeue path (the self-wake arm of the same seam as
/// `FutureTask`).
struct YieldOnce(bool);

impl Future for YieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            cx.waker().wake_by_ref(); // arrange the wake, then park
            Poll::Pending
        }
    }
}

/// An async countdown: awaits `YieldOnce` across each intermediate step, then
/// returns a final `Ready(1)` (the `None` that follows terminates it).
#[task]
pub(crate) async fn async_count_down(ctx: Ctx, n: u32) -> Option<(u32, TaskStatus<u32, ()>)> {
    let _ = ctx;
    match n {
        0 => None,
        1 => Some((0, TaskStatus::Ready(1))),
        k => {
            YieldOnce(false).await;
            Some((k - 1, TaskStatus::Ignore))
        }
    }
}

/// WHY: The async macro must generate a working struct whose in-flight future
///      self-wakes into `Ignore` (requeue) and, on the pool, parks on `Depends`
///      and resumes when woken — never busy-polling.
///
/// WHAT: Driven manually, the machine reports `Ignore` for each self-wake and
///       each transition step, then `Ready(1)`.
#[test]
fn task_macro_async_drives_through_pending() {
    let ctx = Ctx::host();

    let mut task = async_count_down::new(3);
    // Self-wake "still in flight" and the transition's own `Ignore` both report
    // as `Ignore` — the executor requeues and keeps polling either way.
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ignore));
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ignore));
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ignore));
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ignore));
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ready(1)));
    assert_eq!(task.next_status(ctx.clone()), None);
}

// ============================================================================
// Ctx integration (feature 05 × 06): the transition receives/owns the engine Ctx
// ============================================================================

/// A marker the test seeds into `Ctx` so a transition can prove it received the
/// *same* `Ctx` the caller configured — not a freshly-minted empty one.
#[derive(Debug, PartialEq)]
struct Seed(u32);

/// A one-shot state machine's state: `Go` runs the single step, `Done` ends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Once {
    Go,
    Done,
}

/// Reads the engine-seeded value out of `ctx` on its single transition.
#[expect(
    clippy::needless_pass_by_value,
    reason = "the #[task] macro generates a fn(Ctx, T) transition, so ctx is by value"
)]
#[task]
fn read_seeded_ctx(ctx: Ctx, state: Once) -> Option<(Once, TaskStatus<u32, ()>)> {
    match state {
        Once::Done => None,
        Once::Go => {
            let seed = ctx.get::<Seed>().expect("engine seeded the marker").0;
            Some((Once::Done, TaskStatus::Ready(seed)))
        }
    }
}

/// WHY: feature 05 threaded `ctx: Ctx` into `next_status`; the `#[task]`
///      transition must receive that engine `Ctx`, not an empty one (UCS-C7).
///
/// WHAT: seed `Seed(42)` into the `Ctx`, drive `read_seeded_ctx::new(Once::Go)`,
///       and assert the transition reads it back as its `Ready` value.
#[test]
fn transition_receives_the_engine_ctx() {
    let ctx = Ctx::host();
    ctx.insert::<Seed>(Arc::new(Seed(42)));

    let mut task = read_seeded_ctx::new(Once::Go);
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ready(42)));
    assert_eq!(task.next_status(ctx.clone()), None);
}

/// Reads the engine-seeded value out of `ctx` *after* an `.await`, proving the
/// in-flight future owns its `ctx` clone (no borrow, no drop across the yield).
#[task]
async fn async_read_seeded_ctx(ctx: Ctx, state: Once) -> Option<(Once, TaskStatus<u32, ()>)> {
    match state {
        Once::Done => None,
        Once::Go => {
            YieldOnce(false).await; // suspend, then read `ctx` on resume
            let seed = ctx.get::<Seed>().expect("seed visible across await").0;
            Some((Once::Done, TaskStatus::Ready(seed)))
        }
    }
}

/// WHY: the async transition's future must own its `ctx` clone across `.await`
///      (UCS-C8), so it reads the seeded value after resuming from the yield.
///
/// WHAT: seed the `Ctx`, drive the async transition (self-wake `Ignore`, then
///       `Ready(seed)` after the yield), and assert the seed is read back.
#[test]
fn async_transition_owns_ctx_across_await() {
    let ctx = Ctx::host();
    ctx.insert::<Seed>(Arc::new(Seed(7)));

    let mut task = async_read_seeded_ctx::new(Once::Go);
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ignore)); // self-wake
    assert_eq!(task.next_status(ctx.clone()), Some(TaskStatus::Ready(7)));
    assert_eq!(task.next_status(ctx.clone()), None);
}

// ============================================================================
// Success criteria: `Send` + manual `with_ctx` driving
// ============================================================================

fn assert_send<T: Send>() {}

/// The generated structs are `Send` when their `T, R, P` are — pinning the
/// "generated struct is `Send`" success criterion. A sync state task is `Send`
/// in every configuration; an async one only under `multi`, because without it
/// `AsyncTransition` is deliberately `!Send` so `!Send` futures (e.g. a
/// `JsFuture` body) still work.
#[test]
fn generated_structs_are_send() {
    assert_send::<count_down>();
    #[cfg(feature = "multi")]
    assert_send::<async_count_down>();
}

/// `build_house::new(state).with_ctx(ctx)` drives the generated struct manually
/// as a plain `Iterator`, threading the supplied `Ctx` (success criterion).
#[test]
fn generated_struct_drives_manually_via_with_ctx() {
    let ctx = Ctx::host();
    let mut iter = count_down::new(3).with_ctx(ctx);
    assert_eq!(iter.next(), Some(TaskStatus::Ignore));
    assert_eq!(iter.next(), Some(TaskStatus::Ignore));
    assert_eq!(iter.next(), Some(TaskStatus::Ready(1)));
    assert_eq!(iter.next(), None);
}

// ============================================================================
// Spec 88 feature 01 T4: `#[task]` transitions on a JS-hosted wasm executor.
//
// Each case runs twice: `#[valtron_test]` natively and
// `#[foundation_macros::valtron_bindgen]` on wasm.

macro_rules! dual {
    ($name:ident, $body:path) => {
        #[cfg(not(target_family = "wasm"))]
        #[foundation_core::valtron::valtron_test]
        async fn $name() {
            $body().await;
        }

        #[cfg(target_family = "wasm")]
        #[foundation_macros::valtron_bindgen]
        async fn $name() {
            $body().await;
        }
    };
}

/// The phase of the three-state machine the transition drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Start,
    Wait,
    Finish,
    Done,
}

/// A transition that reports `Pending`, then two `Ready` values, then ends.
#[expect(
    clippy::needless_pass_by_value,
    reason = "the #[task] macro generates a fn(Ctx, T) transition, so ctx is by value"
)]
#[task]
fn staged(ctx: Ctx, phase: Phase) -> Option<(Phase, TaskStatus<u32, ()>)> {
    let _ = ctx;
    match phase {
        Phase::Start => Some((Phase::Wait, TaskStatus::Pending(()))),
        Phase::Wait => Some((Phase::Finish, TaskStatus::Ready(1))),
        Phase::Finish => Some((Phase::Done, TaskStatus::Ready(2))),
        Phase::Done => None,
    }
}

/// One observed state of the wrapped task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Pending,
    Ready(u32),
    Done,
}

/// Forwards a task and records every status it reports.
struct RecordingTask<T> {
    inner: T,
    seen: Arc<Mutex<Vec<Step>>>,
}

impl<T> TaskIterator for RecordingTask<T>
where
    T: TaskIterator<Ready = u32, Pending = ()>,
{
    type Ready = u32;
    type Pending = ();

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<u32, ()>> {
        let next = self.inner.next_status(ctx);
        let mut seen = self
            .seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &next {
            Some(TaskStatus::Pending(())) => seen.push(Step::Pending),
            Some(TaskStatus::Ready(v)) => seen.push(Step::Ready(*v)),
            None => seen.push(Step::Done),
            _ => {}
        }
        drop(seen);
        next
    }
}

/// Drives the `#[task]` machine on the executor and collects the full stream,
/// including `Pending`, so the order is observable without blocking.
async fn ready_pending_ready_observes_the_states() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let task = RecordingTask {
        inner: staged::new(Phase::Start),
        seen: Arc::clone(&seen),
    };
    let mut stream = execute(task, None)
        .expect("the pool scheduled the task")
        .into_future_stream();
    while stream.next().await.is_some() {}

    let seen = seen
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(
        seen,
        vec![Step::Pending, Step::Ready(1), Step::Ready(2), Step::Done],
        "the executor must observe Pending, then Ready, then Ready, then the end"
    );
}

dual!(
    task_transition_ready_pending_ready_runs_on_the_executor,
    ready_pending_ready_observes_the_states
);

/// A task cancelled before its first poll returns `Cancelled` and never polls
/// its future body.
#[expect(clippy::unused_async, reason = "dual! awaits the body as a future")]
async fn cancelled_before_first_poll_never_runs() {
    let cancel = Arc::new(AtomicBool::new(true));
    let ran = Arc::new(AtomicBool::new(false));
    let ran_check = Arc::clone(&ran);
    let mut task = CancellableFutureTask::new(
        async move {
            ran_check.store(true, Ordering::SeqCst);
            1_u32
        },
        cancel,
    );

    match task.next_status(Ctx::host()) {
        Some(TaskStatus::Ready(Err(CancelOutcome::Cancelled))) => {}
        other => panic!("expected Cancelled, got {other:?}"),
    }
    assert!(
        !ran.load(Ordering::SeqCst),
        "a task cancelled before its first poll must not run"
    );
}

dual!(
    task_cancelled_before_its_first_poll_ends_without_running,
    cancelled_before_first_poll_never_runs
);

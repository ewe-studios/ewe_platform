//! Cross-runtime coverage for valtron-**driven** tasks.
//!
//! Every task below is scheduled through [`execute`] — valtron drives it, we
//! never poll the task by hand (`next_status`/`with_ctx`). The only thing that
//! varies across the three suites is **which runtime awaits the collected
//! stream**:
//!
//! - **valtron** — `#[valtron_test] async fn`, so `.await` parks on the engine's
//!   own `block_on_future`;
//! - **tokio** — a current-thread tokio runtime polls `into_collect_future()`;
//! - **smol** — `smol::block_on` polls the same future.
//!
//! This pins the runtime-agnostic contract of the task → `execute` → stream →
//! future bridge: whichever executor polls the collect future, the valtron
//! engine produces the same `Vec<T::Ready>`. Under `multi` the task runs on
//! worker threads and the collect future is woken across threads; under the
//! default single-threaded build it is drained inline — both must agree.
//!
//! The pool is initialised by `#[valtron_test]` (never a manual
//! `initialize_pool`), which also serialises pool users via its lifecycle lock.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_core::valtron::{
    execute, from_future, task, valtron_test, Ctx, StreamIteratorExt, TaskIterator,
    TaskStatus,
};

// ============================================================================
// Representative valtron tasks
// ============================================================================

/// Feature 06 sync state machine: `Ignore`, `Ignore`, `Ready(1)`, end.
#[task]
fn count_down(ctx: Ctx, n: u32) -> Option<(u32, TaskStatus<u32, ()>)> {
    let _ = ctx;
    match n {
        0 => None,
        1 => Some((0, TaskStatus::Ready(1))),
        k => Some((k - 1, TaskStatus::Ignore)),
    }
}

/// A future that self-wakes once, then completes — drives the async transition's
/// `Ignore` requeue path.
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

/// Feature 06 async state machine: self-wakes across each step, `Ready(1)`.
#[task]
async fn async_count_down(ctx: Ctx, n: u32) -> Option<(u32, TaskStatus<u32, ()>)> {
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

/// A plain `TaskIterator` (no `#[task]`), emitting `Ready(3), Ready(2), Ready(1)`.
/// Exercises the general task → stream → future bridge, not the state-machine
/// wrapper.
struct Counter(u32);

impl TaskIterator for Counter {
    type Ready = u32;
    type Pending = ();

    fn next_status(
        &mut self,
        _ctx: Ctx,
    ) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.0 == 0 {
            None
        } else {
            let value = self.0;
            self.0 -= 1;
            Some(TaskStatus::Ready(value))
        }
    }
}

/// A future that **genuinely parks** (the event-driven "pipe-future" edge): it
/// returns `Pending` *without* self-waking, hands a clone of its waker to a
/// background thread, and only completes once that thread wakes it. This forces
/// `FutureTask` → `Depends` (park) → cross-thread wake → `Ready`, so the
/// `into_collect_future()` bridge must park the ambient runtime and be woken by
/// valtron's engine — the opposite of the self-wake `Ignore` requeue path.
struct ThreadWakeFuture {
    /// 0 = not started, 2 = thread has fired and the value is ready.
    state: Arc<AtomicUsize>,
    started: bool,
}

impl ThreadWakeFuture {
    fn new() -> Self {
        Self { state: Arc::new(AtomicUsize::new(0)), started: false }
    }
}

impl Future for ThreadWakeFuture {
    type Output = u32;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut(); // Unpin: Arc<AtomicUsize> + bool
        if me.state.load(Ordering::SeqCst) == 2 {
            return Poll::Ready(42);
        }
        if !me.started {
            me.started = true;
            let waker = cx.waker().clone();
            let state = me.state.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(30));
                state.store(2, Ordering::SeqCst);
                waker.wake();
            });
        }
        // No self-wake → FutureTask emits `Depends` and parks until the thread fires.
        Poll::Pending
    }
}

// ============================================================================
// Per-runtime drivers — the task is scheduled via `execute`; only the runtime
// that awaits `into_collect_future()` differs.
// ============================================================================

/// Schedule `task` on valtron, then await its collected values under a
/// current-thread tokio runtime.
fn drive_under_tokio<T>(task: T) -> Vec<T::Ready>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + Clone + Unpin + 'static,
    T::Pending: Send + 'static,
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("tokio runtime");
    rt.block_on(async move {
        execute(task, None)
            .expect("valtron engine must be initialized")
            .into_collect_future()
            .await
    })
}

/// Schedule `task` on valtron, then await its collected values under smol.
fn drive_under_smol<T>(task: T) -> Vec<T::Ready>
where
    T: TaskIterator + Send + 'static,
    T::Ready: Send + Clone + Unpin + 'static,
    T::Pending: Send + 'static,
{
    smol::block_on(async move {
        execute(task, None)
            .expect("valtron engine must be initialized")
            .into_collect_future()
            .await
    })
}

// ============================================================================
// valtron runtime (own engine)
// ============================================================================

#[valtron_test(seed = 42)]
async fn valtron_drives_sync_state_task() {
    let got = execute(count_down::new(3), None)
        .expect("engine initialized")
        .into_collect_future()
        .await;
    assert_eq!(got, vec![1]);
}

#[valtron_test(seed = 42)]
async fn valtron_drives_async_state_task() {
    let got = execute(async_count_down::new(3), None)
        .expect("engine initialized")
        .into_collect_future()
        .await;
    assert_eq!(got, vec![1]);
}

#[valtron_test(seed = 42)]
async fn valtron_drives_general_task() {
    let got = execute(Counter(3), None)
        .expect("engine initialized")
        .into_collect_future()
        .await;
    assert_eq!(got, vec![3, 2, 1]);
}

#[valtron_test(seed = 42)]
async fn valtron_drives_future_task() {
    let got = execute(from_future(core::future::ready(7u32)), None)
        .expect("engine initialized")
        .into_collect_future()
        .await;
    assert_eq!(got, vec![7]);
}

// ============================================================================
// tokio runtime (valtron drives the task; tokio polls the collect future)
// ============================================================================

#[valtron_test(seed = 42)]
fn tokio_drives_sync_state_task() {
    assert_eq!(drive_under_tokio(count_down::new(3)), vec![1]);
}

// Multi-only: async state tasks are `Send` only under `multi` (see
// `AsyncTransition`), and the runtimes here need a `Send` task.
#[cfg(feature = "multi")]
#[valtron_test(seed = 42)]
fn tokio_drives_async_state_task() {
    assert_eq!(drive_under_tokio(async_count_down::new(3)), vec![1]);
}

#[valtron_test(seed = 42)]
fn tokio_drives_general_task() {
    assert_eq!(drive_under_tokio(Counter(3)), vec![3, 2, 1]);
}

#[valtron_test(seed = 42)]
fn tokio_drives_future_task() {
    assert_eq!(drive_under_tokio(from_future(core::future::ready(7u32))), vec![7]);
}

// ============================================================================
// smol runtime (valtron drives the task; smol polls the collect future)
// ============================================================================

#[valtron_test(seed = 42)]
fn smol_drives_sync_state_task() {
    assert_eq!(drive_under_smol(count_down::new(3)), vec![1]);
}

#[cfg(feature = "multi")]
#[valtron_test(seed = 42)]
fn smol_drives_async_state_task() {
    assert_eq!(drive_under_smol(async_count_down::new(3)), vec![1]);
}

#[valtron_test(seed = 42)]
fn smol_drives_general_task() {
    assert_eq!(drive_under_smol(Counter(3)), vec![3, 2, 1]);
}

#[valtron_test(seed = 42)]
fn smol_drives_future_task() {
    assert_eq!(drive_under_smol(from_future(core::future::ready(7u32))), vec![7]);
}

// ============================================================================
// Genuine park — the event-driven edge, cross-runtime
// ============================================================================
//
// Unlike the self-wake `Ignore` path above, `ThreadWakeFuture` parks for real
// (`Depends`), so the collect bridge must park the ambient runtime and be woken
// by valtron's engine when the background thread fires. This is the edge the
// static-`Stream` tests never reach (their `WakeableStream` is `NotSupported`).

#[valtron_test(seed = 42)]
async fn valtron_drives_parked_future_task() {
    let got = execute(from_future(ThreadWakeFuture::new()), None)
        .expect("engine initialized")
        .into_collect_future()
        .await;
    assert_eq!(got, vec![42]);
}

#[valtron_test(seed = 42)]
fn tokio_drives_parked_future_task() {
    assert_eq!(drive_under_tokio(from_future(ThreadWakeFuture::new())), vec![42]);
}

#[valtron_test(seed = 42)]
fn smol_drives_parked_future_task() {
    assert_eq!(drive_under_smol(from_future(ThreadWakeFuture::new())), vec![42]);
}

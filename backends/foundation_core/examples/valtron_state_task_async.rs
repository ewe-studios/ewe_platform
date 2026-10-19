//! Asynchronous state-machine task with `#[valtron::task]`.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p foundation_core --example valtron_state_task_async
//! ```
//!
//! The transition is an `async fn`; while its future is in flight it self-wakes
//! into `TaskStatus::Ignore` ("still working, nothing to see"). Each `.await`
//! parks (never busy-polls) and resumes when woken — the same wake-queue seam
//! `FutureTask` uses.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

// The engine macros expand to `crate::valtron::…` when they run inside this
// package's own example targets (`proc_macro_crate` reports `Itself`), so bring
// the *module* name into the example's crate root to make that path resolve.
use foundation_core::valtron;
use foundation_core::valtron::{
    execute, task, valtron, Ctx, StreamIteratorExt, TaskStatus,
};

/// A tiny future that simulates a unit of async work: it yields `steps` times
/// (each yield is one `Poll::Pending` with a self-wake) before completing.
struct Work(u32);

impl Future for Work {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.0 == 0 {
            Poll::Ready(())
        } else {
            self.0 -= 1;
            cx.waker().wake_by_ref(); // "made progress, poll me again"
            Poll::Pending
        }
    }
}

/// The finished product, emitted as the terminal `Ready` value.
#[derive(Clone, Debug, PartialEq)]
struct House {
    walls: u32,
}

/// Raise one wall per step, awaiting some async work in between.
///
/// The pending type here is `()` — the same shape as the synchronous form.
#[task]
async fn build_house(
    ctx: Ctx,
    walls: u32,
) -> Option<(u32, TaskStatus<House, ()>)> {
    let _ = ctx;
    if walls < 4 {
        Work(2).await; // async work happens *inside* the transition
        Some((walls + 1, TaskStatus::Ignore))
    } else {
        Some((walls, TaskStatus::Ready(House { walls })))
    }
}

#[valtron]
async fn main() {
    let houses = execute(build_house::new(0), None)
        .expect("engine live")
        .into_collect_future()
        .await;
    assert_eq!(houses, vec![House { walls: 4 }]);
    println!("async build_house -> {houses:?}");
}

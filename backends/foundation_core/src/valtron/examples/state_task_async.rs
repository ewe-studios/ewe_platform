//! # Asynchronous state task (`#[valtron::task]`)
//!
//! An `async fn` transition can `.await` inside the body — the signature is the
//! same shape as the synchronous form. While the future is still in flight it
//! self-wakes into `TaskStatus::Ignore` ("still working, nothing to see"), and
//! each `.await` parks on the same wake-queue seam `FutureTask` uses — no
//! busy-polling.
//!
//! This file is a *readable* reference copy. The compiled, runnable version is
//! [`foundation_core/examples/valtron_state_task_async.rs`][run]:
//!
//! ```sh
//! cargo run -p foundation_core --example valtron_state_task_async
//! ```
//!
//! [run]: ../../../examples/valtron_state_task_async.rs

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use foundation_core::valtron::{execute, task, valtron, Ctx, StreamIteratorExt, TaskStatus};

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

/// The finished product, emitted as the `Ready` value.
#[derive(Clone, Debug, PartialEq)]
struct House {
    walls: u32,
}

/// An enum state: either still raising walls, or done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Building {
    Raising(u32),
    Done,
}

/// Raise one wall per step, awaiting some async work in between.
///
/// The pending type here is `()` — the same shape as the synchronous form.
/// `Ready` is **not** terminal; reaching the target emits `Ready(House)` once
/// and moves to `Building::Done`, so the next turn (`Done => None`) ends the
/// machine.
#[task]
async fn build_house(ctx: Ctx, walls: Building) -> Option<(Building, TaskStatus<House, ()>)> {
    let _ = ctx;
    match walls {
        Building::Done => None,
        Building::Raising(n) => {
            if n < 4 {
                Work(2).await; // async work happens *inside* the transition
                Some((Building::Raising(n + 1), TaskStatus::Ignore))
            } else {
                Some((Building::Done, TaskStatus::Ready(House { walls: n })))
            }
        }
    }
}

#[valtron]
async fn main() {
    let (house, _rest) = execute(build_house::new(Building::Raising(0)), None)
        .expect("engine live")
        .into_ready_future()
        .await
        .expect("a house was built");
    assert_eq!(house, House { walls: 4 });
    println!("async build_house -> {house:?}");
}

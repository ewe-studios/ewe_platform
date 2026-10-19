//! Asynchronous `#[valtron::task]` driven by an **enum** state, awaited with the
//! ready-future bridge (a normal `.await`, not a collect-all).
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

use foundation_core::valtron;
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

/// An enum state: either still heating toward the target, or done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Thermostat {
    Heating { current: u32, target: u32 },
    Done,
}

/// The finished product, emitted as the `Ready` value.
#[derive(Debug, PartialEq)]
struct Reading {
    reached: u32,
}

/// Step one degree toward the target, awaiting some async work in between.
///
/// Reaching the target emits `Ready(Reading)` and moves to `Thermostat::Done`,
/// whose arm returns `None` (terminal) on the next turn. `Ready` is not
/// terminal — `None` is.
#[task]
async fn regulate(ctx: Ctx, state: Thermostat) -> Option<(Thermostat, TaskStatus<Reading, ()>)> {
    let _ = ctx;
    match state {
        Thermostat::Done => None,
        Thermostat::Heating { current, target } => {
            if current == target {
                Some((Thermostat::Done, TaskStatus::Ready(Reading { reached: current })))
            } else {
                Work(2).await; // async work happens *inside* the transition
                Some((
                    Thermostat::Heating { current: current + 1, target },
                    TaskStatus::Ignore,
                ))
            }
        }
    }
}

#[valtron]
async fn main() {
    let start = Thermostat::Heating { current: 18, target: 21 };
    let (reading, _rest) = execute(regulate::new(start), None)
        .expect("engine live")
        .into_ready_future()
        .await
        .expect("thermostat reached its target");
    assert_eq!(reading, Reading { reached: 21 });
    println!("async regulate -> {reading:?}");
}

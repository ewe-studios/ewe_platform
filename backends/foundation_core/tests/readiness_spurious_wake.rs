//! A spurious readiness wake does not cost a poll turn (issue #310).
//!
//! WHY: the executor gates a woken sleeper on
//! `(take_woken() || due) && signal.is_ready(dur)`. A notifier that fires while
//! the readiness stays false wakes the thread but does not re-poll the task. A
//! task parked on a pipe's close-only signal must therefore not be woken by a
//! consumer pop: a pop fires the vacancy notifier, not the close one. Nothing
//! public showed those wakes, so this test counts them with
//! `executor_wake_count`.
//!
//! WHAT: a producer task parks on the close-only readiness of a full pipe. A
//! helper thread pops one item, waits, then drops the receiver. The task must
//! see exactly one wake of its executor: the close. The pop adds none.
//!
//! HOW: the task reads the executor wake count at the park and at the resume,
//! both on the thread that hosts it. This file is its own test target, so no
//! other test can wake that executor in the window that the count covers.

#![cfg(not(target_family = "wasm"))]

use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{
    execute, executor_wake_count, valtron_test, Ctx, EventReadinessPtr, Pipe, StreamIteratorExt,
    TaskIterator, TaskStatus,
};

/// The real time the helper gives the engine to park the task before the pop.
const PARK_DELAY: Duration = Duration::from_millis(200);

/// The real time between the pop and the close. It is long enough that a wake
/// from the pop would reach the parked executor before the close.
const POP_GAP: Duration = Duration::from_millis(200);

/// Parks on a readiness, then reports how many wakes its executor received
/// between the park and the resume.
struct ParksOnClose {
    signal: Option<EventReadinessPtr>,
    step: u8,
    wakes_at_park: u64,
}

impl TaskIterator for ParksOnClose {
    type Ready = u64;
    type Pending = ();

    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        match self.step {
            0 => {
                self.step = 1;
                self.wakes_at_park = executor_wake_count();
                Some(TaskStatus::Depends(
                    self.signal.take().expect("the signal is present"),
                ))
            }
            1 => {
                self.step = 2;
                Some(TaskStatus::Ready(
                    executor_wake_count().saturating_sub(self.wakes_at_park),
                ))
            }
            _ => None,
        }
    }
}

#[valtron_test(threads = 1, timeout = 10_000)]
async fn a_pop_does_not_wake_a_parked_producer() {
    let (tx, rx) = Pipe::<u32>::with_depth(2);
    tx.try_send(1).expect("send 1");
    tx.try_send(2).expect("send 2");
    let signal: EventReadinessPtr = Arc::new(tx.closed_readiness());

    let popper = std::thread::spawn(move || {
        // Let the engine park the producer before the pop, so the pop reaches a
        // parked task and any wake it causes would be counted.
        std::thread::sleep(PARK_DELAY);
        assert_eq!(rx.try_recv(), Ok(1), "the pop takes the first item");
        // The pop frees a slot and fires the vacancy notifier, not the closed
        // one. The gap gives a wrong wake time to reach the executor.
        std::thread::sleep(POP_GAP);
        // The close is the one wake the parked producer needs.
        drop(rx);
    });

    let wakes = execute(
        ParksOnClose {
            signal: Some(signal),
            step: 0,
            wakes_at_park: 0,
        },
        None,
    )
    .expect("execute")
    .into_collect_future()
    .await;
    popper.join().expect("the popper thread");

    assert_eq!(
        wakes,
        vec![1],
        "the parked producer must see exactly one executor wake: the close of the \
         pipe. A pop must not wake it"
    );
}

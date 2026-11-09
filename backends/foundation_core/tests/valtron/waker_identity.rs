//! A task's wake identity is stable across polls (issue #329).
//!
//! WHY: the `FutureTask`, `StreamTask` and `AsyncStateTask` adapters used to
//! build a new queue waker on every poll. A source that keeps the waker (a
//! `ResumeSignal` notifier, for example) cannot deduplicate a new waker, so its
//! registration list grew by one on every poll. A hot task that self-wakes made
//! each register and notify cost grow with the poll count, and the executor
//! starved. The proxy stream tests failed with a 5 s deadline under load.
//!
//! WHAT: each adapter now keeps one waker for its whole life. A future, stream
//! or async state machine that registers its waker on every poll keeps one
//! registration, and that registration still wakes it.
#![cfg(not(target_family = "wasm"))]

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use foundation_core::valtron::{
    AsyncStateTask, AsyncTransition, Ctx, EventReadiness, FutureTask, ReadinessWaker, ResumeSignal,
    StreamTask, TaskIterator, TaskStatus,
};
use futures_core::Stream as FuturesStream;

/// Polls per test. Enough for the old per-poll waker to make the growth visible.
const POLLS: usize = 512;

/// Registers the current waker on `signal` and self-wakes until the last poll.
struct RegisteringFuture {
    signal: ResumeSignal,
    polls: usize,
}

impl Future for RegisteringFuture {
    type Output = usize;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<usize> {
        self.polls += 1;
        EventReadiness::register(
            &self.signal,
            &ReadinessWaker::from_waker(cx.waker().clone()),
        );
        if self.polls >= POLLS {
            Poll::Ready(self.polls)
        } else {
            // Self-wake: the executor re-polls the task, as a hot stream does.
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

/// A repeatedly polled future keeps one waker on the source.
#[test]
fn a_repeatedly_polled_future_keeps_one_waker() {
    let signal = ResumeSignal::new(false);
    let mut task = FutureTask::new(RegisteringFuture {
        signal: signal.clone(),
        polls: 0,
    });
    let ctx = Ctx::host();

    for poll in 1..=POLLS {
        let status = task.next_status(ctx.clone()).expect("task still running");
        assert_eq!(
            signal.waiter_count(),
            1,
            "poll {poll}: a re-polled future must keep one registration, not grow one per poll"
        );
        if poll < POLLS {
            assert!(
                matches!(status, TaskStatus::Ignore),
                "poll {poll}: a self-wake must return Ignore, got {status:?}"
            );
        } else {
            assert!(
                matches!(status, TaskStatus::Ready(_)),
                "last poll: expected Ready, got {status:?}"
            );
        }
    }
}

/// A stream that registers the current waker on `signal` on every poll and
/// self-wakes until its last step.
struct RegisteringStream {
    signal: ResumeSignal,
    polls: usize,
}

impl FuturesStream for RegisteringStream {
    type Item = usize;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<usize>> {
        self.polls += 1;
        EventReadiness::register(
            &self.signal,
            &ReadinessWaker::from_waker(cx.waker().clone()),
        );
        if self.polls >= POLLS {
            Poll::Ready(None)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

/// A repeatedly polled stream keeps one waker on the source.
#[test]
fn a_repeatedly_polled_stream_keeps_one_waker() {
    let signal = ResumeSignal::new(false);
    let mut task = StreamTask::new(RegisteringStream {
        signal: signal.clone(),
        polls: 0,
    });
    let ctx = Ctx::host();

    for poll in 1..=POLLS {
        let status = task.next_status(ctx.clone()).expect("task still running");
        assert_eq!(
            signal.waiter_count(),
            1,
            "poll {poll}: a re-polled stream must keep one registration, not grow one per poll"
        );
        if poll < POLLS {
            assert!(
                matches!(status, TaskStatus::Ignore),
                "poll {poll}: a self-wake must return Ignore, got {status:?}"
            );
        } else {
            assert!(
                matches!(status, TaskStatus::Ready(None)),
                "last poll: expected Ready(None), got {status:?}"
            );
        }
    }
}

/// Registers the current waker on `signal` and self-wakes until it has run
/// `remaining` times.
struct KeepAwake {
    signal: ResumeSignal,
    remaining: usize,
}

impl Future for KeepAwake {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        EventReadiness::register(
            &self.signal,
            &ReadinessWaker::from_waker(cx.waker().clone()),
        );
        if self.remaining == 0 {
            Poll::Ready(())
        } else {
            self.remaining -= 1;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

/// A repeatedly polled async state machine keeps one waker on the source.
#[test]
fn a_repeatedly_polled_async_state_task_keeps_one_waker() {
    let signal = ResumeSignal::new(false);
    let transition: AsyncTransition<u8, u8, ()> = {
        let signal = signal.clone();
        Box::new(move |_ctx, state| {
            let signal = signal.clone();
            Box::pin(async move {
                KeepAwake {
                    signal,
                    remaining: POLLS - 1,
                }
                .await;
                Some((state, TaskStatus::Ready(state)))
            })
        })
    };
    let mut task = AsyncStateTask::new(0u8, transition);
    let ctx = Ctx::host();

    for poll in 1..=POLLS {
        let status = task.next_status(ctx.clone()).expect("task still running");
        assert_eq!(
            signal.waiter_count(),
            1,
            "poll {poll}: a re-polled async state machine must keep one registration"
        );
        if poll < POLLS {
            assert!(
                matches!(status, TaskStatus::Ignore),
                "poll {poll}: a self-wake must return Ignore, got {status:?}"
            );
        } else {
            assert!(
                matches!(status, TaskStatus::Ready(0)),
                "last poll: expected Ready(0), got {status:?}"
            );
        }
    }
}

/// A future that registers its waker, then parks until the signal fires.
struct WaitForFlag {
    signal: ResumeSignal,
}

impl Future for WaitForFlag {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<bool> {
        if self.signal.get() {
            return Poll::Ready(true);
        }
        EventReadiness::register(
            &self.signal,
            &ReadinessWaker::from_waker(cx.waker().clone()),
        );
        // Re-check to close the register/fire race.
        if self.signal.get() {
            Poll::Ready(true)
        } else {
            Poll::Pending
        }
    }
}

/// The one kept registration still wakes the task when the source fires.
///
/// WHY: deduplicating registrations must not drop the waker that is in use.
/// WHAT: poll a parking future many times, fire the signal, and assert that the
/// `Depends` signal it returned is now ready and that the next poll completes.
#[test]
fn the_one_kept_registration_still_wakes_the_task() {
    let signal = ResumeSignal::new(false);
    let mut task = FutureTask::new(WaitForFlag {
        signal: signal.clone(),
    });
    let ctx = Ctx::host();

    let mut parking = None;
    for poll in 1..=16 {
        match task.next_status(ctx.clone()) {
            Some(TaskStatus::Depends(ready)) => {
                assert!(
                    !ready.is_ready(None),
                    "poll {poll}: an unfired signal must not report ready"
                );
                parking = Some(ready);
            }
            other => panic!("poll {poll}: expected Depends, got {other:?}"),
        }
        assert_eq!(
            signal.waiter_count(),
            1,
            "poll {poll}: the parked future must keep one registration"
        );
    }

    signal.set(true);
    assert!(
        parking.expect("a park happened").is_ready(None),
        "the one kept waker must fire when the source becomes ready"
    );
    match task.next_status(ctx.clone()) {
        Some(TaskStatus::Ready(true)) => {}
        other => panic!("expected Ready(true) after the signal fired, got {other:?}"),
    }
}

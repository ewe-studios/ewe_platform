//! State-machine task adapters: turn a transition function into a [`TaskIterator`].
//!
//! A *transition* maps `(Ctx, state)` to either the next state paired with a
//! [`TaskStatus`], or `None` (terminal). [`StateTask`] drives a synchronous
//! `FnMut` transition; [`AsyncStateTask`] drives an `async fn` through the same
//! wake-queue seam as [`crate::valtron::FutureTask`]. The `#[valtron::task]`
//! attribute macro (see `foundation_macros::task`) generates a same-named struct
//! wrapping one of the two.
//!
//! Both receive the engine [`Ctx`] on every poll (threaded by
//! [`TaskIterator::next_status`]) and forward it into the transition, so neither
//! owns a `Ctx` of its own.

use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::boxed::Box;
use std::sync::Arc;

use concurrent_queue::ConcurrentQueue;

use crate::valtron::{
    queue_waker, Ctx, QueueReadiness, TaskIterator, TaskStatus, WakeToken,
};

/// The async transition's return: the next state plus a [`TaskStatus`].
pub type TransitionOutput<T, R, P> = Option<(T, TaskStatus<R, P>)>;

/// Synchronous state machine: a [`TaskIterator`] driven by a `FnMut` transition.
///
/// The transition `F` receives the engine [`Ctx`] and the current state, and
/// returns either `None` (terminal) or the next state paired with the
/// [`TaskStatus`] to emit this turn. Every status — `Ready` included — is
/// passed through unchanged and its `next` state is threaded back for the next
/// turn, so a single machine may emit `Ready` many times; only `None` ends it.
/// A transition may therefore legally emit `Pending`, `Delayed`, `Spawn`,
/// `Depends`, `Wait`, `Spread`, and multiple `Ready`s.
pub struct StateTask<T, R, P, F> {
    state: Option<T>,
    transition: F,
    completed: bool,
    // NOTE: no `ctx` field — the engine Ctx is supplied by `next_status` each turn.
    // `R`, `P` are only surface types (returned, never stored); the phantom
    // marks them "used" without imposing Send/Sync/Unpin on `StateTask` itself.
    _marker: PhantomData<fn() -> (R, P)>,
}

impl<T, R, P, F> StateTask<T, R, P, F> {
    /// Wrap `state` and `transition` into a runnable state machine.
    pub fn new(state: T, transition: F) -> Self {
        Self {
            state: Some(state),
            transition,
            completed: false,
            _marker: PhantomData,
        }
    }
}

impl<T, R, P, F> TaskIterator for StateTask<T, R, P, F>
where
    F: FnMut(Ctx, T) -> Option<(T, TaskStatus<R, P>)>,
{
    type Ready = R;
    type Pending = P;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.completed {
            return None;
        }
        let state = self.state.take()?;
        match (self.transition)(ctx, state) {
            // `Ready` is NOT terminal: `next` is threaded back into `state`, so a
            // single machine may emit `Ready` many times. Only `None` ends it.
            Some((next, status)) => {
                self.state = Some(next);
                Some(status)
            }
            None => {
                self.completed = true;
                None
            }
        }
    }
}

/// The boxed, pinned transition of an [`AsyncStateTask`].
///
/// `Send` under `multi` so the iterator crosses threads; `!Send` under `!multi`
/// so `!Send` futures (e.g. a `JsFuture` body) still work.
#[cfg(feature = "multi")]
pub type AsyncTransition<T, R, P> = Box<
    dyn Fn(Ctx, T) -> Pin<Box<dyn Future<Output = TransitionOutput<T, R, P>> + Send>> + Send,
>;

#[cfg(not(feature = "multi"))]
pub type AsyncTransition<T, R, P> =
    Box<dyn Fn(Ctx, T) -> Pin<Box<dyn Future<Output = TransitionOutput<T, R, P>>>>>;

/// The in-flight future of an [`AsyncStateTask`], boxed and pinned so the type is
/// nameable. `Send` under `multi`.
#[cfg(feature = "multi")]
pub type AsyncStateFuture<T, R, P> =
    Pin<Box<dyn Future<Output = TransitionOutput<T, R, P>> + Send>>;

#[cfg(not(feature = "multi"))]
pub type AsyncStateFuture<T, R, P> =
    Pin<Box<dyn Future<Output = TransitionOutput<T, R, P>>>>;

/// Asynchronous state machine: a [`TaskIterator`] driving an `async fn` transition
/// through the [`crate::valtron::FutureTask`] wake-queue seam.
///
/// The transition's future owns its `ctx` clone (moved in at construction), so
/// the `Ctx` lives across `.await` points with no borrow.
pub struct AsyncStateTask<T, R, P>
where
    T: 'static,
    R: 'static,
    P: 'static,
{
    state: Option<T>,
    transition: AsyncTransition<T, R, P>,
    future: Option<AsyncStateFuture<T, R, P>>,
    wake_queue: Arc<ConcurrentQueue<WakeToken>>,
    completed: bool,
}

impl<T, R, P> AsyncStateTask<T, R, P>
where
    T: 'static,
    R: 'static,
    P: 'static,
{
    /// Wrap `state` and a boxed `async` transition into a runnable state machine.
    pub fn new(state: T, transition: AsyncTransition<T, R, P>) -> Self {
        Self {
            state: Some(state),
            transition,
            future: None,
            wake_queue: Arc::new(ConcurrentQueue::unbounded()),
            completed: false,
        }
    }
}

#[cfg(feature = "multi")]
impl<T, R, P> TaskIterator for AsyncStateTask<T, R, P>
where
    T: Send + 'static,
    R: Send + 'static,
    P: Send + 'static,
{
    type Ready = R;
    type Pending = P;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.completed {
            return None;
        }

        // 1. (Re)build the future from `(transition)(ctx, state.take())` if none is
        //    in flight. The future owns its `ctx` clone — no borrow.
        if self.future.is_none() {
            let state = self.state.take()?;
            self.future = Some((self.transition)(ctx, state));
        }

        // 2. Drain stale tokens BEFORE polling so a prior turn's wake isn't mistaken
        //    for a fresh one — a wake landing after this drain stays queued and keeps
        //    the readiness ready (the wake-before-park race is harmless).
        while self.wake_queue.pop().is_ok() {}

        let waker = queue_waker(self.wake_queue.clone(), 0);
        let mut cx = Context::from_waker(&waker);

        match self.future.as_mut().unwrap().as_mut().poll(&mut cx) {
            // `Ready` is NOT terminal: `next` is threaded back into `state` and
            // the future is rebuilt for the next step, so one async machine may
            // emit `Ready` many times. Only `None` ends it.
            Poll::Ready(Some((next, status))) => {
                self.state = Some(next);
                self.future = None;
                Some(status)
            }
            Poll::Ready(None) => {
                self.completed = true;
                None
            }
            Poll::Pending => {
                // Two distinct Pending semantics (mirrors FutureTask, Decision 00-F1):
                //  - self-wake (token pushed during poll): `Ignore` (requeue, no park);
                //  - external-wake (no token): park on the queue readiness.
                if self.wake_queue.is_empty() {
                    Some(TaskStatus::Depends(Arc::new(QueueReadiness::new(
                        self.wake_queue.clone(),
                    ))))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
        }
    }
}

#[cfg(not(feature = "multi"))]
impl<T, R, P> TaskIterator for AsyncStateTask<T, R, P>
where
    T: 'static,
    R: 'static,
    P: 'static,
{
    type Ready = R;
    type Pending = P;

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        // Identical body to the `multi` impl above — see there for the rationale.
        if self.completed {
            return None;
        }

        if self.future.is_none() {
            let state = self.state.take()?;
            self.future = Some((self.transition)(ctx, state));
        }

        while self.wake_queue.pop().is_ok() {}

        let waker = queue_waker(self.wake_queue.clone(), 0);
        let mut cx = Context::from_waker(&waker);

        match self.future.as_mut().unwrap().as_mut().poll(&mut cx) {
            // `Ready` is NOT terminal (see the `multi` impl above): `next` is
            // threaded back and the future rebuilt, so many `Ready`s are fine.
            Poll::Ready(Some((next, status))) => {
                self.state = Some(next);
                self.future = None;
                Some(status)
            }
            Poll::Ready(None) => {
                self.completed = true;
                None
            }
            Poll::Pending => {
                if self.wake_queue.is_empty() {
                    Some(TaskStatus::Depends(Arc::new(QueueReadiness::new(
                        self.wake_queue.clone(),
                    ))))
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
        }
    }
}

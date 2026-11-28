//! The timer effect: one wait that ends at a monotonic deadline.

use core::future::Future;
use core::pin::Pin;

use crate::{Ctx, Instant};

/// A future that a [`Timer`] returns.
///
/// The future is boxed and `Send`, so it crosses a thread and a task boundary.
pub type TimerFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// Waits until a monotonic deadline.
///
/// An implementation chooses the clock. [`ContextTimer`](crate::timer::ContextTimer)
/// binds `Ctx::clock` when the context builds it, so it serves a real clock and a
/// simulated clock. A custom timer may read the clock from `ctx`.
pub trait Timer: Send + Sync + 'static {
    /// Resolves when `deadline` passes on the clock this timer measures.
    ///
    /// `ctx` is a parameter and not a field. A timer held by `Env` must not store
    /// a `Ctx`, because the `Env` is a typed entry of that same `Ctx`; the store
    /// would close an `Arc` cycle (spec 84, section 3, rule 4). A timer may store
    /// an `Arc<dyn Clock>` instead.
    fn sleep_until(&self, ctx: Ctx, deadline: Instant) -> TimerFuture;
}

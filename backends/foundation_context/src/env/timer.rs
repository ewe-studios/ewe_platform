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
/// The timer reads no clock of its own. `ctx` carries the clock that measures
/// `deadline`, so the same timer serves a real clock and a simulated clock.
pub trait Timer: Send + Sync + 'static {
    /// Resolves when `deadline` has passed on the clock of `ctx`.
    ///
    /// `ctx` is a parameter and not a field. A timer that stored a `Ctx` would
    /// close an `Arc` cycle, because the `Env` that holds the timer is a typed
    /// entry of that same `Ctx` (spec 84, section 3, rule 4).
    fn sleep_until(&self, ctx: Ctx, deadline: Instant) -> TimerFuture;
}

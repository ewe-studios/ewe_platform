//! Cooperative sleep — a [`SleepingTask`] that yields [`TaskStatus::Delayed`].
//!
//! Use [`sleep`](crate::valtron::executors::sleep()) (returns the task directly)
//! or [`sleep_async`](crate::valtron::executors::sleep_async()) (returns a
//! `Future` via `execute()` + `into_ready_future()`).

use foundation_compact::Instant;
use std::time::Duration;

use crate::valtron::{Ctx, TaskIterator, TaskStatus};

/// A [`TaskIterator`] that yields [`TaskStatus::Delayed(remaining)`] until
/// the deadline passes, then returns `None`.
pub struct SleepingTask {
    /// The absolute deadline. `None` until the first poll of a duration-based
    /// sleep, which reads the engine clock from the [`Ctx`].
    deadline: Option<Instant>,
    duration: Duration,
}

impl SleepingTask {
    #[must_use]
    pub fn new(deadline: Instant) -> Self {
        Self {
            deadline: Some(deadline),
            duration: Duration::ZERO,
        }
    }

    #[must_use]
    pub fn sleep(duration: Duration) -> Self {
        Self {
            deadline: None,
            duration,
        }
    }

    /// The absolute deadline, once known.
    ///
    /// A duration-based sleep is relative to the engine clock, so its deadline
    /// exists only after the first poll.
    #[must_use]
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// Resolve the absolute deadline on the engine clock, on the first call.
    fn resolve(&mut self, ctx: &Ctx) -> Instant {
        if let Some(deadline) = self.deadline {
            return deadline;
        }
        let now = ctx.clock().now();
        let deadline = now
            .checked_add(self.duration)
            .unwrap_or_else(|| now + Duration::from_secs(1_000_000));
        self.deadline = Some(deadline);
        deadline
    }
}

impl TaskIterator for SleepingTask {
    type Ready = ();
    type Pending = ();

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<(), ()>> {
        let deadline = self.resolve(&ctx);
        let now = ctx.clock().now();
        if now >= deadline {
            None
        } else {
            Some(TaskStatus::Delayed(deadline - now))
        }
    }
}

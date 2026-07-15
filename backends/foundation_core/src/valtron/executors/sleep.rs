//! Cooperative sleep — a [`SleepingTask`] that yields [`TaskStatus::Delayed`].
//!
//! Use [`sleep`](crate::valtron::executors::sleep()) (returns the task directly)
//! or [`sleep_async`](crate::valtron::executors::sleep_async()) (returns a
//! `Future` via `execute()` + `into_ready_future()`).

use std::time::{Duration, Instant};

use crate::valtron::{NoAction, TaskIterator, TaskStatus};

/// A [`TaskIterator`] that yields [`TaskStatus::Delayed(remaining)`] until
/// the deadline passes, then returns `None`.
pub struct SleepingTask {
    deadline: Instant,
}

impl SleepingTask {
    #[must_use]
    pub fn new(deadline: Instant) -> Self {
        Self { deadline }
    }

    #[must_use]
    pub fn sleep(duration: Duration) -> Self {
        Self {
            deadline: Instant::now()
                .checked_add(duration)
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(1_000_000)),
        }
    }

    #[must_use]
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
}

impl TaskIterator for SleepingTask {
    type Ready = ();
    type Pending = ();
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<(), (), NoAction>> {
        let now = Instant::now();
        if now >= self.deadline {
            None
        } else {
            Some(TaskStatus::Delayed(self.deadline - now))
        }
    }
}

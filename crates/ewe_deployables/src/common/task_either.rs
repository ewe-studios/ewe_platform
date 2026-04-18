//! Either type and one-shot task for TaskIterator branching.
//!
//! WHY: `destroy_task()` needs to return the same type from two branches
//!      (state exists vs. no state). Rust's `impl Trait` requires a single
//!      concrete type, so we use `TaskEither` to unify the two branches.
//!      `OneShotTask` yields a single Ready value for the no-op case.
//!
//! WHAT: `TaskEither<A, B>` wraps two `TaskIterator` implementations.
//!       `OneShotTask<R, P, S>` yields a single value then completes.
//!
//! HOW: Delegates `next_status()` to whichever variant is active.

use foundation_core::valtron::{BoxedSendExecutionAction, ExecutionAction, TaskIterator, TaskStatus};

/// Either wrapper for two `TaskIterator` implementations.
pub enum TaskEither<A, B> {
    /// Left variant.
    Left(A),
    /// Right variant.
    Right(B),
}

impl<A, B, R, P, S> TaskIterator for TaskEither<A, B>
where
    A: TaskIterator<Ready = R, Pending = P, Spawner = S>,
    B: TaskIterator<Ready = R, Pending = P, Spawner = S>,
    S: ExecutionAction,
{
    type Ready = R;
    type Pending = P;
    type Spawner = S;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match self {
            Self::Left(a) => a.next_status(),
            Self::Right(b) => b.next_status(),
        }
    }
}

/// A `TaskIterator` that yields a single Ready value then completes.
pub struct OneShotTask<R> {
    value: Option<R>,
}

impl<R> OneShotTask<R> {
    /// Create a new one-shot task that yields the given value.
    pub fn new(value: R) -> Self {
        Self {
            value: Some(value),
        }
    }
}

impl<R> TaskIterator for OneShotTask<R>
where
    R: Send + 'static,
{
    type Ready = R;
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        self.value.take().map(TaskStatus::Ready)
    }
}

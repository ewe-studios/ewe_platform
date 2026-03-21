//! Extension trait and combinators for `TaskIterator`.
//!
//! This module provides builder-style combinator methods for any type implementing
//! `TaskIterator`. These combinators are applied **BEFORE** calling `execute()`.
//!
//! ## Overview
//!
//! The `TaskIteratorExt` trait extends any `TaskIterator` with methods like:
//! - `map_ready()` - Transform Ready values
//! - `map_pending()` - Transform Pending values
//! - `filter_ready()` - Filter Ready values
//! - `stream_collect()` - Collect all Ready values into a Vec
//!
//! ## Usage Pattern
//!
//! ```ignore
//! // 1. Define your task (implements TaskIterator)
//! let task = MyAsyncTask::new();
//!
//! // 2. Apply TaskIteratorExt combinators BEFORE execute()
//! let task = task
//!     .map_ready(|v| transform(v))
//!     .filter_ready(|v| should_keep(v));
//!
//! // 3. Execute to get StreamIterator
//! let stream = execute(task)?;
//! ```
//!
//! ## Relationship to Other Modules
//!
//! - [`super::task`] - Contains `TaskStatus`, `TaskIterator`, `ExecutionAction` definitions
//! - [`super::executors::unified`] - Contains `execute()` entry point
//! - [`super::stream_iterators`] - Contains `StreamIteratorExt` for post-execute combinators

use crate::valtron::{ExecutionAction, TaskIterator, TaskStatus};

/// Extension trait providing builder-style combinator methods for any `TaskIterator`.
///
/// This trait is automatically implemented for any type that implements `TaskIterator`
/// with the appropriate bounds. This includes:
/// - Raw task iterators implementing `TaskIterator`
/// - Driven iterators like `DrivenRecvIterator` and `DrivenSendTaskIterator`
///
/// ## Combinators
///
/// - [`map_ready`](TaskIteratorExt::map_ready) - Transform Ready values
/// - [`map_pending`](TaskIteratorExt::map_pending) - Transform Pending values
/// - [`filter_ready`](TaskIteratorExt::filter_ready) - Filter Ready values
/// - [`stream_collect`](TaskIteratorExt::stream_collect) - Collect all Ready values
///
/// ## Example
///
/// ```ignore
/// let task = MyTask::new()
///     .map_ready(|v| v * 2)
///     .map_pending(|p| format!("Still waiting: {:?}", p))
///     .filter_ready(|v| v > 10);
/// ```
pub trait TaskIteratorExt: TaskIterator + Sized {
    /// Transform Ready values using the provided function.
    ///
    /// Pending, Delayed, Init, and Spawn states pass through unchanged.
    fn map_ready<F, R>(self, f: F) -> TMapReady<Self, F>
    where
        F: Fn(Self::Ready) -> R + Send + 'static,
        R: Send + 'static;

    /// Transform Pending values using the provided function.
    ///
    /// Ready, Delayed, Init, and Spawn states pass through unchanged.
    fn map_pending<F, R>(self, f: F) -> TMapPending<Self, F>
    where
        F: Fn(Self::Pending) -> R + Send + 'static,
        R: Send + 'static;

    /// Filter Ready values using the provided predicate.
    ///
    /// Non-Ready states pass through unchanged. Ready values that don't
    /// satisfy the predicate are returned as `TaskStatus::Ignore`.
    fn filter_ready<F>(self, f: F) -> TFilterReady<Self, F>
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static;

    /// Collect all Ready values into a Vec.
    ///
    /// Unlike `std::Iterator::collect()`, this does NOT block waiting for all items.
    /// It passes through Pending, Delayed, Init, Spawn states unchanged,
    /// replaces Ready values with `TaskStatus::Ignore`, and only yields the
    /// collected `Vec<Ready>` when the inner iterator completes.
    fn stream_collect(self) -> TStreamCollect<Self>
    where
        Self::Ready: Clone + Send + 'static;
}

// Blanket implementation: anything implementing TaskIterator gets TaskIteratorExt
impl<I> TaskIteratorExt for I
where
    I: TaskIterator + Send + 'static,
    I::Ready: Send + 'static,
    I::Pending: Send + 'static,
    I::Spawner: ExecutionAction + Send + 'static,
{
    fn map_ready<F, R>(self, f: F) -> TMapReady<Self, F>
    where
        F: Fn(Self::Ready) -> R + Send + 'static,
        R: Send + 'static,
    {
        TMapReady {
            inner: self,
            mapper: f,
        }
    }

    fn map_pending<F, R>(self, f: F) -> TMapPending<Self, F>
    where
        F: Fn(Self::Pending) -> R + Send + 'static,
        R: Send + 'static,
    {
        TMapPending {
            inner: self,
            mapper: f,
        }
    }

    fn filter_ready<F>(self, f: F) -> TFilterReady<Self, F>
    where
        F: Fn(&Self::Ready) -> bool + Send + 'static,
    {
        TFilterReady {
            inner: self,
            predicate: f,
        }
    }

    fn stream_collect(self) -> TStreamCollect<Self>
    where
        Self::Ready: Send + 'static,
    {
        TStreamCollect {
            inner: self,
            collected: Vec::new(),
            done: false,
        }
    }
}

/// Wrapper type that transforms Ready values.
pub struct TMapReady<I, F> {
    inner: I,
    mapper: F,
}

impl<I, F, R> Iterator for TMapReady<I, F>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> R + Send + 'static,
    R: Send + 'static,
{
    type Item = TaskStatus<R, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|status| match status {
            TaskStatus::Ready(v) => TaskStatus::Ready((self.mapper)(v)),
            TaskStatus::Pending(v) => TaskStatus::Pending(v),
            TaskStatus::Delayed(d) => TaskStatus::Delayed(d),
            TaskStatus::Ignore => TaskStatus::Ignore,
            TaskStatus::Init => TaskStatus::Init,
            TaskStatus::Spawn(s) => TaskStatus::Spawn(s),
        })
    }
}

impl<I, F, R> TaskIterator for TMapReady<I, F>
where
    I: TaskIterator,
    F: Fn(I::Ready) -> R + Send + 'static,
    R: Send + 'static,
{
    type Ready = R;
    type Pending = I::Pending;
    type Spawner = I::Spawner;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

/// Wrapper type that transforms Pending values.
pub struct TMapPending<I, F> {
    inner: I,
    mapper: F,
}

impl<I, F, R> Iterator for TMapPending<I, F>
where
    I: TaskIterator,
    F: Fn(I::Pending) -> R + Send + 'static,
    R: Send + 'static,
{
    type Item = TaskStatus<I::Ready, R, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|status| match status {
            TaskStatus::Ready(v) => TaskStatus::Ready(v),
            TaskStatus::Pending(v) => TaskStatus::Pending((self.mapper)(v)),
            TaskStatus::Delayed(d) => TaskStatus::Delayed(d),
            TaskStatus::Init => TaskStatus::Init,
            TaskStatus::Ignore => TaskStatus::Ignore,
            TaskStatus::Spawn(s) => TaskStatus::Spawn(s),
        })
    }
}

impl<I, F, R> TaskIterator for TMapPending<I, F>
where
    I: TaskIterator,
    F: Fn(I::Pending) -> R + Send + 'static,
    R: Send + 'static,
{
    type Ready = I::Ready;
    type Pending = R;
    type Spawner = I::Spawner;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

/// Wrapper type that filters Ready values.
///
/// Filtered-out Ready values are returned as `TaskStatus::Ignore` to avoid blocking.
pub struct TFilterReady<I, F> {
    inner: I,
    predicate: F,
}

impl<I, F> Iterator for TFilterReady<I, F>
where
    I: TaskIterator,
    F: Fn(&I::Ready) -> bool + Send + 'static,
{
    type Item = TaskStatus<I::Ready, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        let status = self.inner.next()?;
        match &status {
            TaskStatus::Ready(v) => {
                if (self.predicate)(v) {
                    Some(status)
                } else {
                    Some(TaskStatus::Ignore)
                }
            }
            _ => Some(status), // Pass through non-Ready states
        }
    }
}

impl<I, F> TaskIterator for TFilterReady<I, F>
where
    I: TaskIterator,
    F: Fn(&I::Ready) -> bool + Send + 'static,
{
    type Ready = I::Ready;
    type Pending = I::Pending;
    type Spawner = I::Spawner;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

/// Wrapper type that collects all Ready values into a Vec.
///
/// Passes through Pending, Delayed, Init, Spawn states unchanged.
/// Ready values are collected and replaced with `TaskStatus::Ignore`.
/// Only yields the collected Vec when the inner iterator completes.
/// Does NOT require Ready to implement Clone.
pub struct TStreamCollect<I: TaskIterator> {
    inner: I,
    collected: Vec<I::Ready>,
    done: bool,
}

impl<I> Iterator for TStreamCollect<I>
where
    I: TaskIterator,
    I::Ready: Send + 'static,
{
    type Item = TaskStatus<Vec<I::Ready>, I::Pending, I::Spawner>;

    fn next(&mut self) -> Option<Self::Item> {
        // If we've already yielded the collected result, we're done
        if self.done {
            return None;
        }

        match self.inner.next() {
            Some(TaskStatus::Ready(value)) => {
                self.collected.push(value);
                // Keep collecting, return Ignore to signal collected but continue
                Some(TaskStatus::Ignore)
            }
            Some(TaskStatus::Pending(p)) => Some(TaskStatus::Pending(p)),
            Some(TaskStatus::Delayed(d)) => Some(TaskStatus::Delayed(d)),
            Some(TaskStatus::Init) => Some(TaskStatus::Init),
            Some(TaskStatus::Spawn(s)) => Some(TaskStatus::Spawn(s)),
            Some(TaskStatus::Ignore) => Some(TaskStatus::Ignore),
            None => {
                // Inner iterator is done, yield the collected result
                self.done = true;
                let collected = std::mem::take(&mut self.collected);
                Some(TaskStatus::Ready(collected))
            }
        }
    }
}

impl<I> TaskIterator for TStreamCollect<I>
where
    I: TaskIterator,
    I::Ready: Send + 'static,
{
    type Ready = Vec<I::Ready>;
    type Pending = I::Pending;
    type Spawner = I::Spawner;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        Iterator::next(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test task iterator for unit tests
    struct TestTask {
        items: Vec<TaskStatus<u32, String, crate::valtron::NoAction>>,
    }

    impl TestTask {
        fn new(items: Vec<TaskStatus<u32, String, crate::valtron::NoAction>>) -> Self {
            Self { items }
        }
    }

    impl Iterator for TestTask {
        type Item = TaskStatus<u32, String, crate::valtron::NoAction>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.pop()
        }
    }

    impl TaskIterator for TestTask {
        type Ready = u32;
        type Pending = String;
        type Spawner = crate::valtron::NoAction;

        fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
            Iterator::next(self)
        }
    }

    #[test]
    fn test_map_ready() {
        let items = vec![
            TaskStatus::Pending("wait".to_string()),
            TaskStatus::Ready(10),
            TaskStatus::Ready(5),
        ];
        let task = TestTask::new(items);
        let mut mapped = task.map_ready(|x| x * 2);

        // pop() returns items in reverse order: 5, 10, Pending
        // After mapping: 10 (5*2), 20 (10*2), Pending
        assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Ready(10)));
        assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Ready(20)));
        assert_eq!(
            Iterator::next(&mut mapped),
            Some(TaskStatus::Pending("wait".to_string()))
        );
    }

    #[test]
    fn test_map_pending() {
        let items = vec![
            TaskStatus::Pending("wait".to_string()),
            TaskStatus::Ready(5),
        ];
        let task = TestTask::new(items);
        let mut mapped = task.map_pending(|s| s.len());

        assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Ready(5)));
        assert_eq!(Iterator::next(&mut mapped), Some(TaskStatus::Pending(4)));
    }

    #[test]
    fn test_filter_ready() {
        let items = vec![
            TaskStatus::Ready(3),
            TaskStatus::Ready(10),
            TaskStatus::Ready(5),
        ];
        let task = TestTask::new(items);
        let mut filtered = task.filter_ready(|x| *x > 5);

        // pop() returns: Ready(5), Ready(10), Ready(3)
        // filter: 5 > 5 = false → Ignore, 10 > 5 = true → Ready(10), 3 > 5 = false → Ignore
        assert_eq!(Iterator::next(&mut filtered), Some(TaskStatus::Ignore)); // 5 was filtered out
        assert_eq!(Iterator::next(&mut filtered), Some(TaskStatus::Ready(10)));
        assert_eq!(Iterator::next(&mut filtered), Some(TaskStatus::Ignore)); // 3 was filtered out
        assert_eq!(Iterator::next(&mut filtered), None);
    }

    #[test]
    fn test_stream_collect() {
        let items = vec![
            TaskStatus::Ready(2),
            TaskStatus::Pending("wait".to_string()),
            TaskStatus::Ready(1),
        ];
        let task = TestTask::new(items);
        let mut collected = task.stream_collect();

        // First Ready is collected, returns Ignore
        assert_eq!(Iterator::next(&mut collected), Some(TaskStatus::Ignore));

        // Should pass through Pending
        assert_eq!(
            Iterator::next(&mut collected),
            Some(TaskStatus::Pending("wait".to_string()))
        );

        // Second Ready is collected, returns Ignore
        assert_eq!(Iterator::next(&mut collected), Some(TaskStatus::Ignore));

        // Should yield collected Vec at the end
        match Iterator::next(&mut collected) {
            Some(TaskStatus::Ready(vec)) => {
                assert_eq!(vec.len(), 2);
                assert!(vec.contains(&1));
                assert!(vec.contains(&2));
            }
            other => panic!("Expected Ready(Vec), got {:?}", other),
        }

        // Should be done after yielding the collected result
        assert_eq!(Iterator::next(&mut collected), None);
    }
}

//! Retry/timeout wrappers for `TaskIterators`.
//!
//! This module provides composable wrappers that add retry logic, timeouts,
//! and backoff strategies to any `TaskIterator`, enabling robust task execution.

use crate::valtron::{TaskIterator, TaskStatus};
use std::time::Duration;

// ============================================================================
// TimeoutTask - Add timeout to any TaskIterator (requires std)
// ============================================================================

#[cfg(feature = "std")]
use std::time::Instant;

/// Wraps a `TaskIterator` with a timeout.
///
/// WHY: Tasks may hang indefinitely; need automatic timeout handling
/// WHAT: Stops task after specified duration, converts to None
///
/// Only available with `std` feature (requires Instant).
#[cfg(feature = "std")]
pub struct TimeoutTask<T>
where
    T: TaskIterator,
{
    inner: T,
    timeout: Duration,
    started_at: Option<Instant>,
    timed_out: Option<()>,
}

#[cfg(feature = "std")]
impl<T> TimeoutTask<T>
where
    T: TaskIterator,
{
    pub fn new(inner: T, timeout: Duration) -> Self {
        Self {
            inner,
            timeout,
            started_at: None,
            timed_out: None,
        }
    }
}

#[cfg(feature = "std")]
impl<T> TaskIterator for TimeoutTask<T>
where
    T: TaskIterator,
{
    type Ready = T::Ready;
    type Pending = T::Pending;
    type Spawner = T::Spawner;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.timed_out.is_some() {
            return None; // Task stops on timeout
        }

        // Initialize start time on first poll
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }

        // Check if timed out and return timed out ready state
        if self.started_at.unwrap().elapsed() > self.timeout {
            tracing::warn!("Task timed out after {:?}", self.timeout);
            return None;
        }

        // Poll inner task and wrap pending states
        self.inner.next().map(|status| match status {
            TaskStatus::Pending(p) => TaskStatus::Pending(p),
            TaskStatus::Ready(r) => TaskStatus::Ready(r),
            TaskStatus::Delayed(d) => TaskStatus::Delayed(d),
            TaskStatus::Spawn(a) => TaskStatus::Spawn(a),
            TaskStatus::Init => TaskStatus::Init,
        })
    }
}

// ============================================================================
// PollLimitTask - Poll count based "timeout" for no_std
// ============================================================================

/// Wraps a `TaskIterator` with a maximum poll count limit.
///
/// WHY: `no_std` lacks Instant; use poll count as proxy for "timeout"
/// WHAT: Stops task after N polls to prevent infinite loops
///
/// Available in all configurations (`no_std` compatible).
pub struct PollLimitTask<T>
where
    T: TaskIterator,
{
    inner: T,
    max_polls: usize,
    current_polls: usize,
}

impl<T> PollLimitTask<T>
where
    T: TaskIterator,
{
    pub fn new(inner: T, max_polls: usize) -> Self {
        Self {
            inner,
            max_polls,
            current_polls: 0,
        }
    }
}

impl<T> TaskIterator for PollLimitTask<T>
where
    T: TaskIterator,
{
    type Ready = T::Ready;
    type Pending = T::Pending;
    type Spawner = T::Spawner;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.current_polls >= self.max_polls {
            tracing::warn!("Task exceeded poll limit of {}", self.max_polls);
            return None;
        }

        self.current_polls += 1;
        self.inner.next()
    }
}

// ============================================================================
// RetryingTask - Add retry logic to any TaskIterator
// ============================================================================

/// Trait for deciding when to retry a task.
///
/// WHY: Different tasks have different retry conditions
/// WHAT: Determines if a result should trigger a retry
pub trait RetryDecider<T> {
    /// Returns true if the task should be retried based on the result.
    ///
    /// WHY: Caller knows what constitutes a retryable failure
    /// WHAT: Inspect result and current attempt, return bool
    fn should_retry(&self, result: &T, attempt: u32) -> bool;
}

/// Simple retry decider that always retries up to max attempts.
///
/// WHY: Common case is to retry N times regardless of error
/// WHAT: Returns true until `max_attempts` reached
pub struct AlwaysRetry {
    pub max_attempts: u32,
}

impl<T> RetryDecider<T> for AlwaysRetry {
    fn should_retry(&self, _result: &T, attempt: u32) -> bool {
        attempt < self.max_attempts
    }
}

// ============================================================================
// BackoffTask - Add backoff delays to retry attempts
// ============================================================================

/// Backoff strategy for retry delays.
///
/// WHY: Different scenarios need different backoff approaches
/// WHAT: Calculates delay based on attempt number
#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    /// Fixed delay between retries
    Fixed(Duration),

    /// Exponential backoff: base * multiplier^attempt
    Exponential { base: Duration, multiplier: f64 },

    /// Linear backoff: base + (increment * attempt)
    Linear { base: Duration, increment: Duration },
}

impl BackoffStrategy {
    /// Calculate next delay based on strategy and attempt number.
    ///
    /// WHY: Need to compute delay for current retry attempt
    /// WHAT: Returns Duration clamped to `max_delay`
    #[must_use]
    pub fn next_delay(&self, attempt: u32, max_delay: Duration) -> Duration {
        let next = match self {
            Self::Fixed(d) => *d,
            Self::Exponential { base, multiplier } => {
                Duration::from_secs_f64(base.as_secs_f64() * multiplier.powi(attempt as i32))
            }
            Self::Linear { base, increment } => *base + (*increment * attempt),
        };
        next.min(max_delay)
    }
}

/// Wraps a `TaskIterator` with backoff delays.
///
/// WHY: Retries should have delays to avoid overwhelming services
/// WHAT: Inserts delays based on strategy before each retry
pub struct BackoffTask<T>
where
    T: TaskIterator,
{
    inner: T,
    strategy: BackoffStrategy,
    max_delay: Duration,
    current_attempt: u32,
    in_delay: Option<Duration>,
}

impl<T> BackoffTask<T>
where
    T: TaskIterator,
{
    pub fn new(inner: T, strategy: BackoffStrategy, max_delay: Duration) -> Self {
        Self {
            inner,
            strategy,
            max_delay,
            current_attempt: 0,
            in_delay: None,
        }
    }
}

impl<T> TaskIterator for BackoffTask<T>
where
    T: TaskIterator,
{
    type Ready = T::Ready;
    type Pending = T::Pending;
    type Spawner = T::Spawner;

    fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // If we're in a delay, emit it
        if let Some(delay) = self.in_delay.take() {
            return Some(TaskStatus::Delayed(delay));
        }

        // Poll inner task
        match self.inner.next() {
            Some(TaskStatus::Ready(result)) => {
                // On completion, calculate delay for next retry
                // (if this task will be retried by outer RetryingTask)
                self.current_attempt += 1;
                let delay = self
                    .strategy
                    .next_delay(self.current_attempt, self.max_delay);
                self.in_delay = Some(delay);
                Some(TaskStatus::Ready(result))
            }
            other => other,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[allow(dead_code)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::valtron::NoAction;

    struct SimpleTask {
        values: Vec<i32>,
        index: usize,
    }

    impl TaskIterator for SimpleTask {
        type Ready = i32;
        type Pending = ();
        type Spawner = NoAction;

        fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
            if self.index < self.values.len() {
                let val = self.values[self.index];
                self.index += 1;
                Some(TaskStatus::Ready(val))
            } else {
                None
            }
        }
    }

    /// WHY: TimeoutTask must stop task after timeout duration
    /// WHAT: Task that takes too long should return None
    #[test]
    #[cfg(feature = "std")]
    fn test_timeout_task_stops_after_duration() {
        use std::thread;

        struct SlowTask {
            count: u32,
        }

        impl TaskIterator for SlowTask {
            type Ready = u32;
            type Pending = ();
            type Spawner = NoAction;

            fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
                thread::sleep(Duration::from_millis(50));
                self.count += 1;
                if self.count < 100 {
                    Some(TaskStatus::Pending(()))
                } else {
                    Some(TaskStatus::Ready(self.count))
                }
            }
        }

        let task = SlowTask { count: 0 };
        let mut timeout_task = TimeoutTask::new(task, Duration::from_millis(200));

        // Poll a few times
        loop {
            if timeout_task.next().is_none() {
                break;
            }
        }

        // Should eventually timeout
        let res = timeout_task.next();
        assert!(res.is_none(), "Expected None but got: {res:?}");
    }

    /// WHY: PollLimitTask must stop after max polls
    /// WHAT: Task exceeding poll limit should return None
    #[test]
    fn test_poll_limit_task_stops_after_max_polls() {
        struct InfiniteTask;

        impl TaskIterator for InfiniteTask {
            type Ready = i32;
            type Pending = ();
            type Spawner = NoAction;

            fn next(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
                Some(TaskStatus::Pending(()))
            }
        }

        let task = InfiniteTask;
        let mut limited_task = PollLimitTask::new(task, 5);

        // Should allow 5 polls
        for i in 0..5 {
            assert!(limited_task.next().is_some(), "Poll {} should succeed", i);
        }

        // 6th poll should return None
        assert!(limited_task.next().is_none());
    }

    /// WHY: BackoffStrategy::Fixed must return constant delay
    /// WHAT: All attempts return same duration
    #[test]
    fn test_backoff_strategy_fixed() {
        let strategy = BackoffStrategy::Fixed(Duration::from_millis(100));
        let max = Duration::from_secs(10);

        assert_eq!(strategy.next_delay(0, max), Duration::from_millis(100));
        assert_eq!(strategy.next_delay(5, max), Duration::from_millis(100));
        assert_eq!(strategy.next_delay(100, max), Duration::from_millis(100));
    }

    /// WHY: BackoffStrategy::Exponential must increase exponentially
    /// WHAT: Each attempt doubles the delay
    #[test]
    fn test_backoff_strategy_exponential() {
        let strategy = BackoffStrategy::Exponential {
            base: Duration::from_millis(100),
            multiplier: 2.0,
        };
        let max = Duration::from_secs(10);

        assert_eq!(strategy.next_delay(0, max), Duration::from_millis(100)); // 100 * 2^0
        assert_eq!(strategy.next_delay(1, max), Duration::from_millis(200)); // 100 * 2^1
        assert_eq!(strategy.next_delay(2, max), Duration::from_millis(400)); // 100 * 2^2
        assert_eq!(strategy.next_delay(3, max), Duration::from_millis(800)); // 100 * 2^3
    }

    /// WHY: BackoffStrategy::Linear must increase linearly
    /// WHAT: Each attempt adds increment to base
    #[test]
    fn test_backoff_strategy_linear() {
        let strategy = BackoffStrategy::Linear {
            base: Duration::from_millis(100),
            increment: Duration::from_millis(50),
        };
        let max = Duration::from_secs(10);

        assert_eq!(strategy.next_delay(0, max), Duration::from_millis(100)); // 100 + 50*0
        assert_eq!(strategy.next_delay(1, max), Duration::from_millis(150)); // 100 + 50*1
        assert_eq!(strategy.next_delay(2, max), Duration::from_millis(200)); // 100 + 50*2
        assert_eq!(strategy.next_delay(3, max), Duration::from_millis(250)); // 100 + 50*3
    }

    /// WHY: BackoffStrategy must respect max_delay
    /// WHAT: Computed delay should be clamped to max
    #[test]
    fn test_backoff_strategy_respects_max() {
        let strategy = BackoffStrategy::Exponential {
            base: Duration::from_millis(100),
            multiplier: 2.0,
        };
        let max = Duration::from_millis(500);

        assert_eq!(strategy.next_delay(10, max), max); // 100 * 2^10 = 102400ms, clamped to 500ms
    }

    /// WHY: AlwaysRetry decider must retry up to max attempts
    /// WHAT: Returns true until max_attempts reached
    #[test]
    fn test_always_retry_decider() {
        let decider = AlwaysRetry { max_attempts: 3 };

        assert!(decider.should_retry(&42, 0));
        assert!(decider.should_retry(&42, 1));
        assert!(decider.should_retry(&42, 2));
        assert!(!decider.should_retry(&42, 3));
        assert!(!decider.should_retry(&42, 4));
    }
}

//! Tests for the `TaskIterator` wrappers: `TimeoutTask`, `PollLimitTask`,
//! `BackoffStrategy` and `AlwaysRetry`.

#[cfg(feature = "std")]
use foundation_core::valtron::TimeoutTask;
use foundation_core::valtron::{
    AlwaysRetry, BackoffStrategy, Ctx, PollLimitTask, RetryDecider, TaskIterator, TaskStatus,
};
use std::time::Duration;

/// WHY: TimeoutTask must stop task after timeout duration
/// WHAT: Task that takes too long should return None
#[test]
#[cfg(feature = "std")]
fn test_timeout_task_stops_after_duration() {
    use std::thread;

    let ctx = Ctx::new();
    struct SlowTask {
        count: u32,
    }

    impl TaskIterator for SlowTask {
        type Ready = u32;
        type Pending = ();

        fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
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
        if timeout_task.next_status(ctx.clone()).is_none() {
            break;
        }
    }

    // Should eventually timeout
    let res = timeout_task.next_status(ctx.clone());
    assert!(res.is_none(), "Expected None but got: {res:?}");
}

/// WHY: PollLimitTask must stop after max polls
/// WHAT: Task exceeding poll limit should return None
#[test]
fn test_poll_limit_task_stops_after_max_polls() {
    let ctx = Ctx::new();
    struct InfiniteTask;

    impl TaskIterator for InfiniteTask {
        type Ready = i32;
        type Pending = ();

        fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
            Some(TaskStatus::Pending(()))
        }
    }

    let task = InfiniteTask;
    let mut limited_task = PollLimitTask::new(task, 5);

    // Should allow 5 polls
    for i in 0..5 {
        assert!(
            limited_task.next_status(ctx.clone()).is_some(),
            "Poll {} should succeed",
            i
        );
    }

    // 6th poll should return None
    assert!(limited_task.next_status(ctx.clone()).is_none());
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

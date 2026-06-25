// --- Constants

use core::time;

pub const MAX_ROUNDS_IDLE_COUNT: u32 = 64;
pub const MAX_ROUNDS_WHEN_SLEEPING_ENDS: u32 = 32;

/// `DEFAULT_OP_READ_TIME` defaults how long we wait for a message from
/// the activity queue.
pub const DEFAULT_OP_READ_TIME: time::Duration = time::Duration::from_millis(100); // 100ms
pub const DEFAULT_YIELD_WAIT_TIME: time::Duration = time::Duration::from_millis(4000); // 4.0s

/// `DEFAULT_MAX_TURNS` is the default number of poll attempts before yielding `Stream::Ignore`
/// in `ConcurrentQueueStreamIterator`. This balances responsiveness (checking other tasks)
/// against throughput (busy polling for items).
pub const DEFAULT_MAX_TURNS: usize = 15;

/// `DEFAULT_PARK_DURATION` is the default duration to park thread when queue is empty
/// in `ConcurrentQueueStreamIterator` (std mode only).
/// Changed from 20ns to 1ms in Feature 04 - 20ns was being rounded up by OS to microseconds anyway.
pub const DEFAULT_PARK_DURATION: time::Duration = time::Duration::from_millis(1);

/// `DEFAULT_WAIT_CYCLE` is the default duration to wait for queue items to appear
/// in `ConcurrentQueueStreamIterator` (std mode only).
pub const DEFAULT_WAIT_CYCLE: std::time::Duration = std::time::Duration::from_micros(300);

// ============================================================================
// Feature 04: Notification-Based Waiting Constants
// ============================================================================

/// `DEFAULT_NOTIFY_QUEUE_WAIT_TIMEOUT` is the default timeout for `NotifyQueue::wait_for_item()`.
/// This is how long a consumer will wait on the CondVar before returning `None`.
#[cfg(not(target_family = "wasm"))]
pub const DEFAULT_NOTIFY_QUEUE_WAIT_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(10);

#[cfg(target_family = "wasm")]
pub const DEFAULT_NOTIFY_QUEUE_WAIT_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(10);

/// `DEFAULT_NOTIFY_QUEUE_MAX_SPINS` is the maximum number of CondVar wait retries before
/// yielding back to the executor. On wasm32/JS this is 1 so we yield immediately to the
/// event loop. On native we use a higher value (100) to mimic the previous blocking behaviour
/// with CondVar-based waiting.
#[cfg(target_family = "wasm")]
pub const DEFAULT_NOTIFY_QUEUE_MAX_SPINS: usize = 1;

#[cfg(not(target_family = "wasm"))]
pub const DEFAULT_NOTIFY_QUEUE_MAX_SPINS: usize = 100;

/// `DEFAULT_KILL_SIGNAL_CHECK_INTERVAL` is how often to check kill signal in `block_on()` inner loop.
/// Default is every 16 iterations (instead of checking every iteration or only every 200).
pub const DEFAULT_KILL_SIGNAL_CHECK_INTERVAL: usize = 16;

/// `DEFAULT_READINESS_WAIT` is the default duration to wait for a readiness signal before timing out.
pub const DEFAULT_READINESS_WAIT: std::time::Duration = std::time::Duration::from_millis(10);

pub const BACK_OFF_JITER: f32 = 0.75;
pub const BACK_OFF_THREAD_FACTOR: u32 = 6;
pub const BACK_OFF_MIN_DURATION: time::Duration = time::Duration::from_millis(1); // 2 http
pub const BACK_OFF_MAX_DURATION: time::Duration = time::Duration::from_millis(1000); // 1sec

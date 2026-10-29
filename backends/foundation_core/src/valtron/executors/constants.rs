// --- Constants

use core::time;

pub const MAX_ROUNDS_IDLE_COUNT: u32 = 64;
pub const MAX_ROUNDS_WHEN_SLEEPING_ENDS: u32 = 32;

/// `DEFAULT_OP_READ_TIME` defaults how long we wait for a message from
/// the activity queue.
pub const DEFAULT_OP_READ_TIME: time::Duration = time::Duration::from_millis(100); // 100ms
pub const DEFAULT_YIELD_WAIT_TIME: time::Duration = time::Duration::from_millis(4000); // 4.0s

/// `DEFAULT_MAX_TURNS` is the default number of poll attempts before yielding
/// in `NotifyQueueStreamIterator`. This balances responsiveness (checking other
/// tasks) against throughput.
pub const DEFAULT_MAX_TURNS: usize = 15;

/// `DEFAULT_PARK_DURATION` is the default wait duration when the queue is empty
/// in `NotifyQueueStreamIterator`.
/// Changed from 20ns to 1ms in Feature 04 - 20ns was being rounded up by OS to microseconds anyway.
pub const DEFAULT_PARK_DURATION: time::Duration = time::Duration::from_millis(1);

/// `DEFAULT_WAIT_CYCLE` is the default duration to wait for queue items to appear
/// in `NotifyQueueStreamIterator`.
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

/// `DEFAULT_DELIVERY_CAPACITY` is the bound for a `sequenced` task's delivery queue
/// (Decision 15). `sequenced` interleaves the parent's drain with the child's
/// production, so a small bound makes the child park on vacancy when it outruns
/// the parent instead of buffering without limit. Kept small on purpose — a large
/// bound is unbounded with extra steps. `lift` is unbounded (finish-before needs
/// it) and is unaffected by this constant.
pub const DEFAULT_DELIVERY_CAPACITY: usize = 8;

// ============================================================================
// Rule 8 defaults (Limits are configuration, not constants)
// ============================================================================
//
// Every value below is a *default*, referenced only by a config's `Default` impl
// (or a builder default). The code path reads the config field, never this const.

/// Default cap on how long a worker yields between steps (the idle re-check
/// cap). Mirrors the old `MAX_YIELD_DURATION`. Pick: a worker must wake to check
/// for new work at least every 100 ms.
pub const DEFAULT_MAX_YIELD_DURATION: time::Duration = time::Duration::from_millis(100);

/// Default jitter applied to the back-off factor (0.75 = ±25%).
pub const DEFAULT_BACK_OFF_JITTER: f32 = 0.75;

/// Default base factor for the exponential back-off of an idle worker.
pub const DEFAULT_BACK_OFF_THREAD_FACTOR: u32 = 6;

/// Default minimum back-off duration for an idle worker.
pub const DEFAULT_BACK_OFF_MIN_DURATION: time::Duration = time::Duration::from_millis(1);

/// Default maximum back-off duration for an idle worker.
pub const DEFAULT_BACK_OFF_MAX_DURATION: time::Duration = time::Duration::from_millis(1000);

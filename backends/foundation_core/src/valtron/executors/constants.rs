// --- Constants

use core::time;

pub const MAX_ROUNDS_IDLE_COUNT: u32 = 64;
pub const MAX_ROUNDS_WHEN_SLEEPING_ENDS: u32 = 32;

/// `DEFAULT_OP_READ_TIME` defaults how long we wait for a message from
/// the activity queue.
pub const DEFAULT_OP_READ_TIME: time::Duration = time::Duration::from_millis(100); // 100ms
pub const DEFAULT_YIELD_WAIT_TIME: time::Duration = time::Duration::from_secs(6); // 6s

/// `DEFAULT_MAX_TURNS` is the default number of poll attempts before yielding `Stream::Ignore`
/// in `ConcurrentQueueStreamIterator`. This balances responsiveness (checking other tasks)
/// against throughput (busy polling for items).
pub const DEFAULT_MAX_TURNS: usize = 15;

/// `DEFAULT_PARK_DURATION` is the default duration to park thread when queue is empty
/// in `ConcurrentQueueStreamIterator` (std mode only).
pub const DEFAULT_PARK_DURATION: time::Duration = time::Duration::from_nanos(20);

/// `DEFAULT_WAIT_CYCLE` is the default duration to wait for queue items to appear
/// in `ConcurrentQueueStreamIterator` (std mode only).
pub const DEFAULT_WAIT_CYCLE: std::time::Duration = std::time::Duration::from_nanos(100);

pub const BACK_OFF_JITER: f32 = 0.75;
pub const BACK_OFF_THREAD_FACTOR: u32 = 6;
pub const BACK_OFF_MIN_DURATION: time::Duration = time::Duration::from_millis(1); // 2 http
pub const BACK_OFF_MAX_DURATION: time::Duration = time::Duration::from_millis(1000); // 1sec

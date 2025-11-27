// --- Constants

use core::time;

pub const MAX_ROUNDS_IDLE_COUNT: u32 = 64;
pub const MAX_ROUNDS_WHEN_SLEEPING_ENDS: u32 = 32;

/// DEFAULT_OP_READ_TIME defaults how long we wait for a message from
/// the activity queue.
pub const DEFAULT_OP_READ_TIME: time::Duration = time::Duration::from_millis(100); // 100ms

pub const BACK_OFF_JITER: f32 = 0.75;
pub const BACK_OFF_THREAD_FACTOR: u32 = 6;
pub const BACK_OFF_MIN_DURATION: time::Duration = time::Duration::from_millis(1); // 2 http
pub const BACK_OFF_MAX_DURATION: time::Duration = time::Duration::from_millis(1000); // 1sec

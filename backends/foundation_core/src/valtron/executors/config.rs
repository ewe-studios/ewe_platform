use crate::valtron::executors::DEFAULT_WAIT_CYCLE;

/// Configuration for stream execution with fine-grained control over iterator behavior.
///
/// # Fields
///
/// * `wait_cycle` - Polling/wait duration for executor (defaults to [`DEFAULT_WAIT_CYCLE`])
/// * `max_turns` - Max poll attempts before yielding in `ConcurrentQueueStreamIterator`
///   (defaults to [`crate::valtron::executors::DEFAULT_MAX_TURNS`])
/// * `park_duration` - Thread park duration when queue is empty, passed to
///   `ConcurrentQueueStreamIterator` (defaults to [`crate::valtron::executors::DEFAULT_PARK_DURATION`])
///
/// # Example
///
/// ```ignore
/// use crate::valtron::executors::{StreamConfig, DEFAULT_MAX_TURNS};
/// use std::time::Duration;
///
/// let config = StreamConfig::default()
///     .with_max_turns(50)  // Higher throughput
///     .with_park_duration(Duration::from_nanos(50));
///
/// let result = execute_with_config(task, config)?;
/// ```
#[derive(Debug, Clone)]
pub struct StreamConfig {
    /// Wait cycle duration for executor polling
    pub wait_cycle: std::time::Duration,
    /// Max turns for ConcurrentQueueStreamIterator
    pub max_turns: usize,
    /// Park duration for ConcurrentQueueStreamIterator (used as wait_cycle)
    pub park_duration: std::time::Duration,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            wait_cycle: crate::valtron::executors::DEFAULT_WAIT_CYCLE,
            max_turns: crate::valtron::executors::DEFAULT_MAX_TURNS,
            park_duration: crate::valtron::executors::DEFAULT_PARK_DURATION,
        }
    }
}

impl StreamConfig {
    /// Create a new StreamConfig with custom wait_cycle.
    #[must_use]
    pub fn with_wait_cycle(mut self, wait_cycle: std::time::Duration) -> Self {
        self.wait_cycle = wait_cycle;
        self
    }

    /// Create a new StreamConfig with custom max_turns.
    #[must_use]
    pub fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    /// Create a new StreamConfig with custom park_duration.
    #[must_use]
    pub fn with_park_duration(mut self, park_duration: std::time::Duration) -> Self {
        self.park_duration = park_duration;
        self
    }
}

/// Configuration for stream execution with fine-grained control over iterator behavior.
///
/// # Fields
///
/// * `wait_cycle` - Polling/wait duration for executor (defaults to [`DEFAULT_WAIT_CYCLE`])
/// * `max_turns` - Max poll attempts before yielding in `NotifyQueueStreamIterator`
///   (defaults to [`crate::valtron::executors::DEFAULT_MAX_TURNS`])
/// * `park_duration` - Wait duration when queue is empty, passed to
///   `NotifyQueueStreamIterator` (defaults to [`crate::valtron::executors::DEFAULT_PARK_DURATION`])
///
/// # Example
///
/// ```
/// use foundation_core::valtron::{
///     execute_with_config, initialize_pool, Ctx, Stream, StreamConfig, TaskIterator,
///     TaskStatus, DEFAULT_MAX_TURNS,
/// };
/// use std::time::Duration;
///
/// struct MyTask(u32);
///
/// impl TaskIterator for MyTask {
///     type Ready = u32;
///     type Pending = ();
///
///     fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
///         if self.0 == 0 {
///             return None;
///         }
///         self.0 -= 1;
///         Some(TaskStatus::Ready(self.0 + 1))
///     }
/// }
///
/// # let _pool = initialize_pool(7, Some(3));
/// let config = StreamConfig::default()
///     .with_max_turns(50) // Higher throughput
///     .with_park_duration(Duration::from_nanos(50));
///
/// assert_eq!(config.max_turns, 50);
/// assert_eq!(config.park_duration, Duration::from_nanos(50));
/// assert!(DEFAULT_MAX_TURNS > 0);
///
/// let values: Vec<u32> = execute_with_config(MyTask(3), config)
///     .expect("schedule the task")
///     .filter_map(|item| match item {
///         Stream::Next(value) => Some(value),
///         _ => None,
///     })
///     .collect();
/// assert_eq!(values, vec![3, 2, 1]);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct StreamConfig {
    /// Wait cycle duration for executor polling
    pub wait_cycle: std::time::Duration,
    /// Max turns for `NotifyQueueStreamIterator`
    pub max_turns: usize,
    /// Park duration for `NotifyQueueStreamIterator` (used as `wait_cycle`)
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
    /// Create a new `StreamConfig` with custom `wait_cycle`.
    #[must_use]
    pub fn with_wait_cycle(mut self, wait_cycle: std::time::Duration) -> Self {
        self.wait_cycle = wait_cycle;
        self
    }

    /// Create a new `StreamConfig` with custom `max_turns`.
    #[must_use]
    pub fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    /// Create a new `StreamConfig` with custom `park_duration`.
    #[must_use]
    pub fn with_park_duration(mut self, park_duration: std::time::Duration) -> Self {
        self.park_duration = park_duration;
        self
    }
}

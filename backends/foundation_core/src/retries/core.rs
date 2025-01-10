use std::time;

pub const DEFAULT_MIN_DURATION: time::Duration = time::Duration::from_millis(100);

/// Attempts is a state identifying the overall expectation for
/// when a reconnection attempt should re-occur. It is Most
/// useful to allow the ConnectionStateIterator to be able to
/// securely handle retries.
#[derive(Clone, Debug)]
pub struct RetryState {
    pub wait: Option<time::Duration>,
    pub total_allowed: u32,
    pub attempt: u32,
}

impl RetryState {
    pub fn new(attempt: u32, total_allowed: u32, wait: Option<time::Duration>) -> Self {
        Self {
            wait,
            total_allowed,
            attempt,
        }
    }

    pub fn can_retry(&self) -> bool {
        self.attempt == self.total_allowed
    }
}

/// ReconnectionDecider defines an retry mechanism that allows
/// a central system to decide the next reconnection attempt parameters
/// regarding how long to wait before attempt and state info on the current
/// attempts and when such attempt to stop by returning None.
pub trait RetryDecider {
    fn decide(&self, state: RetryState) -> Option<RetryState>;
}

pub trait ClonableReconnectionDecider: RetryDecider {
    fn clone_box(&self) -> Box<dyn ClonableReconnectionDecider>;
}

impl<T> ClonableReconnectionDecider for T
where
    T: RetryDecider + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn ClonableReconnectionDecider> {
        Box::new(self.clone())
    }
}

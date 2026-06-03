/// Valtron task: FdMonitorTask — wraps a RegisteredFd and invokes a callback
/// when the fd becomes readable/writable.

use std::io;
use std::os::unix::io::AsRawFd;
use std::time::Duration;

use foundation_core::valtron::{
    BoxedSendExecutionAction, TaskIterator, TaskStatus,
};

use crate::native::fd::{PollResult, RegisteredFd};
use crate::native::poll::Interest;
use super::StopSignal;

/// A valtron task that monitors a RegisteredFd for readiness.
///
/// Each tick, polls the fd for readability. When ready, invokes the
/// user-provided callback with a reference to the inner IO object.
pub struct FdMonitorTask<T: AsRawFd> {
    fd: RegisteredFd<T>,
    callback: Option<Box<dyn FnMut(&T) -> io::Result<()> + Send>>,
    poll_interval: Duration,
    interest: Interest,
    stop: StopSignal,
}

impl<T: AsRawFd> FdMonitorTask<T> {
    /// Create a new FdMonitorTask wrapping the given RegisteredFd.
    pub fn new(fd: RegisteredFd<T>) -> Self {
        Self {
            fd,
            callback: None,
            poll_interval: Duration::from_millis(50),
            interest: Interest::READABLE,
            stop: StopSignal::new(),
        }
    }

    /// Set the callback to invoke when the fd becomes readable.
    pub fn with_callback(
        mut self,
        callback: impl FnMut(&T) -> io::Result<()> + Send + 'static,
    ) -> Self {
        self.callback = Some(Box::new(callback));
        self
    }

    /// Set the poll interval for each tick. Default: 50ms.
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Set the interest to poll for. Default: Interest::READABLE.
    pub fn with_interest(mut self, interest: Interest) -> Self {
        self.interest = interest;
        self
    }

    /// Get a `StopSignal` that can terminate this task when signaled.
    pub fn stop_signal(&self) -> StopSignal {
        self.stop.clone()
    }

    /// Get a reference to the inner RegisteredFd.
    pub fn fd(&self) -> &RegisteredFd<T> {
        &self.fd
    }

    /// Get a mutable reference to the inner RegisteredFd.
    pub fn fd_mut(&mut self) -> &mut RegisteredFd<T> {
        &mut self.fd
    }
}

impl<T: AsRawFd> TaskIterator for FdMonitorTask<T> {
    type Ready = ();
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        // Check stop signal first — if set, return None to terminate the task.
        if self.stop.is_stopped() {
            return None;
        }

        let readiness = match self.interest {
            Interest::READABLE => self.fd.poll_readable(),
            Interest::WRITABLE => self.fd.poll_writable(),
            _ => self.fd.poll_ready(self.interest),
        };

        match readiness {
            PollResult::Ready(mut guard) => {
                if let Some(ref mut cb) = self.callback {
                    if let Ok(Err(e)) = guard.try_io(|fd| cb(fd.get_ref())) {
                        tracing::error!("FdMonitorTask callback I/O error: {}", e);
                    }
                }
                Some(TaskStatus::Ready(()))
            }
            PollResult::NotReady => Some(TaskStatus::Delayed(self.poll_interval)),
            PollResult::Error(e) => {
                tracing::error!("FdMonitorTask poll error: {}", e);
                // Terminate on error — the fd is broken
                None
            }
        }
    }
}

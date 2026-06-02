/// Valtron task: FdMonitorTask — wraps a RegisteredFd and invokes a callback
/// when the fd becomes readable/writable.
///
/// Saves users from writing the same tick loop boilerplate for arbitrary FD
/// monitoring (inotify fds, signalfd, eventfd, pipes, etc.).

use std::io;
use std::os::unix::io::AsRawFd;
use std::time::Duration;

use crate::fd::{PollResult, ReadyGuard, RegisteredFd};
use crate::poll::Interest;

/// A valtron task that monitors a RegisteredFd for readiness.
///
/// Each tick, polls the fd for readability. When ready, invokes the
/// user-provided callback with a reference to the inner IO object.
///
/// The callback receives the inner fd (via `get_ref()`) — not the RegisteredFd
/// itself. This means the callback can read from the fd but cannot deregister it.
pub struct FdMonitorTask<T: AsRawFd> {
    fd: RegisteredFd<T>,
    callback: Option<Box<dyn FnMut(&T) -> io::Result<()>>>,
    poll_interval: Duration,
    interest: Interest,
}

impl<T: AsRawFd> FdMonitorTask<T> {
    /// Create a new FdMonitorTask wrapping the given RegisteredFd.
    pub fn new(fd: RegisteredFd<T>) -> Self {
        Self {
            fd,
            callback: None,
            poll_interval: Duration::from_millis(50),
            interest: Interest::READABLE,
        }
    }

    /// Set the callback to invoke when the fd becomes readable.
    ///
    /// The callback receives a reference to the inner IO object.
    /// It should perform the actual I/O operation (read, write, etc.).
    pub fn with_callback(
        mut self,
        callback: impl FnMut(&T) -> io::Result<()> + 'static,
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

    /// Get a reference to the inner RegisteredFd.
    pub fn fd(&self) -> &RegisteredFd<T> {
        &self.fd
    }

    /// Get a mutable reference to the inner RegisteredFd.
    pub fn fd_mut(&mut self) -> &mut RegisteredFd<T> {
        &mut self.fd
    }
}

impl<T: AsRawFd> FdMonitorTask<T> {
    /// Poll the fd for readiness and invoke the callback if ready.
    ///
    /// This method is called by the valtron execution engine each tick.
    ///
    /// # Returns
    /// - `Some(ReadyGuard)` if the fd was ready (callback was invoked)
    /// - `None` if the fd was not ready or an error occurred
    pub fn tick(&mut self) -> Option<()> {
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
                Some(())
            }
            PollResult::NotReady => None,
            PollResult::Error(e) => {
                tracing::error!("FdMonitorTask poll error: {}", e);
                None
            }
        }
    }
}

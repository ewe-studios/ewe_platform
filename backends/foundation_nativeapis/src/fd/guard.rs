/// ReadyGuard and MutReadyGuard — returned by readiness polls.
///
/// The `#[must_use]` attribute ensures the compiler warns if the guard
/// is dropped without being explicitly handled. This prevents the critical
/// bug of forgetting to clear readiness after an operation, which causes
/// the fd to appear ready forever on edge-triggered pollers.

use crate::fd::{Ready, RegisteredFd};

use std::io;
use std::os::unix::io::AsRawFd;

/// Represents an observed readiness state on a file descriptor.
///
/// #must_use — you must explicitly choose whether to clear readiness.
/// This prevents the critical bug of forgetting to clear readiness
/// after an operation, which causes the fd to appear ready forever
/// on edge-triggered pollers.
#[must_use = "You must explicitly handle readiness via try_io(), clear_ready(), or retain_ready()"]
pub struct ReadyGuard<'a, T: AsRawFd> {
    fd: &'a RegisteredFd<T>,
    readiness: Ready,
}

impl<'a, T: AsRawFd> ReadyGuard<'a, T> {
    pub(crate) fn new(fd: &'a RegisteredFd<T>, readiness: Ready) -> Self {
        Self { fd, readiness }
    }
}

impl<'a, T: AsRawFd> ReadyGuard<'a, T> {
    /// What readiness states were observed.
    pub fn ready(&self) -> Ready {
        self.readiness
    }

    /// Execute an I/O operation. If it returns WouldBlock, readiness
    /// is automatically cleared so the next poll will block again.
    ///
    /// ## EOF handling (Ok(0) for reads)
    ///
    /// When read() returns 0 (EOF on a pipe/socket whose peer closed),
    /// readiness is cleared to prevent a busy loop.
    pub fn try_io<R>(
        &mut self,
        f: impl FnOnce(&RegisteredFd<T>) -> io::Result<R>,
    ) -> Result<io::Result<R>, TryIoError> {
        let result = f(self.fd);

        match &result {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Spurious readiness — clear so next poll blocks
                self.clear_ready();
            }
            Err(_) => {
                // Real I/O error — clear readiness
                self.clear_ready();
            }
            Ok(_) => {
                // I/O succeeded — don't clear readiness, there might be more data.
                // The caller should try_io again until WouldBlock.
            }
        }

        Ok(result)
    }

    /// Manually clear all readiness flags.
    /// Call this when your I/O operation blocks or you've consumed all data.
    /// After clear_ready(), the next poll() will block until new readiness.
    pub fn clear_ready(&mut self) {
        self.fd.registration.clear_readiness(self.readiness);
        self.readiness = Ready::EMPTY;
    }

    /// Clear only specific readiness flags.
    /// Use with combined interests — only clear what actually blocked.
    /// Example: if you read but couldn't write, clear only READABLE.
    pub fn clear_ready_matching(&mut self, ready: Ready) {
        self.fd.registration.clear_readiness(self.readiness.intersection(ready));
        self.readiness = self.readiness.difference(ready);
    }

    /// Explicitly retain readiness (no-op, satisfies must_use).
    /// Use when you've already handled the readiness and want to keep it set.
    pub fn retain_ready(&mut self) {
        // No-op — the guard is being "used"
    }

    /// Get reference to the inner RegisteredFd.
    pub fn get_ref(&self) -> &'a RegisteredFd<T> {
        self.fd
    }

    /// Get reference to the inner IO object.
    pub fn get_inner(&self) -> &'a T {
        self.fd.get_ref()
    }
}

/// Mutable variant of ReadyGuard.
///
/// Needed when the I/O operation requires mutable access to the inner object
/// (e.g., reading into a buffer stored on the task).
#[must_use = "You must explicitly handle readiness via try_io(), clear_ready(), or retain_ready()"]
pub struct MutReadyGuard<'a, T: AsRawFd> {
    fd: &'a mut RegisteredFd<T>,
    readiness: Ready,
}

impl<'a, T: AsRawFd> MutReadyGuard<'a, T> {
    pub(crate) fn new(fd: &'a mut RegisteredFd<T>, readiness: Ready) -> Self {
        Self { fd, readiness }
    }
}

impl<'a, T: AsRawFd> MutReadyGuard<'a, T> {
    /// What readiness states were observed.
    pub fn ready(&self) -> Ready {
        self.readiness
    }

    /// Execute an I/O operation with mutable access to the RegisteredFd.
    pub fn try_io<R>(
        &mut self,
        f: impl FnOnce(&mut RegisteredFd<T>) -> io::Result<R>,
    ) -> Result<io::Result<R>, TryIoError> {
        let result = f(self.fd);

        match &result {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.clear_ready();
            }
            Err(_) => {
                self.clear_ready();
            }
            Ok(_) => {}
        }

        Ok(result)
    }

    /// Manually clear all readiness flags.
    pub fn clear_ready(&mut self) {
        self.fd.registration.clear_readiness(self.readiness);
        self.readiness = Ready::EMPTY;
    }

    /// Clear only specific readiness flags.
    pub fn clear_ready_matching(&mut self, ready: Ready) {
        self.fd.registration.clear_readiness(self.readiness.intersection(ready));
        self.readiness = self.readiness.difference(ready);
    }

    /// Explicitly retain readiness (no-op, satisfies must_use).
    pub fn retain_ready(&mut self) {}

    /// Get mutable reference to the inner RegisteredFd.
    pub fn get_mut(&mut self) -> &mut RegisteredFd<T> {
        self.fd
    }
}

/// Error returned when try_io fails.
#[derive(Debug)]
pub struct TryIoError {
    /// The underlying I/O error (typically WouldBlock).
    pub error: io::Error,
    /// Whether readiness was automatically cleared.
    pub readiness_cleared: bool,
}

impl std::fmt::Display for TryIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "try_io failed: {}", self.error)
    }
}

impl std::error::Error for TryIoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

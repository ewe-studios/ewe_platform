//! `FdReadiness` — park a valtron task on a socket through the shared reactor.
//!
//! WHY: a task that reads a non-blocking socket gets `WouldBlock` when nothing
//! has arrived. Without a readiness source it can only re-poll on a timer (the
//! h2 server polled every 10 ms). The shared reactor already watches fds for
//! readiness; this lets any task that has a raw fd park on it, without owning
//! a `RegisteredFd` (the fd stays owned by its stream).
//!
//! WHAT: [`FdReadiness`] registers an fd for read and write readiness and is an
//! `EventReadiness` for `TaskStatus::Depends`. It deregisters on drop.
//!
//! HOW: the reactor latches edge-triggered readiness until it is cleared. Call
//! [`FdReadiness::clear`] *before* each attempt to read or write, so an edge
//! that arrives after the attempt's `WouldBlock` stays set and wakes the task
//! (clearing after the attempt could erase it).

use std::io;
use std::os::fd::RawFd;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::EventReadiness;
use foundation_nativeapis::native::fd::{Reactor, Ready, SharedReadiness};
use foundation_nativeapis::native::poll::Interest;
use foundation_nativeapis::Token;

/// Readiness of one fd on the shared reactor. See the module docs.
pub struct FdReadiness {
    readiness: SharedReadiness,
    reactor: Arc<Reactor>,
    fd: RawFd,
    token: Token,
}

impl std::fmt::Debug for FdReadiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FdReadiness")
            .field("fd", &self.fd)
            .field("token", &self.token)
            .finish()
    }
}

impl FdReadiness {
    /// Register `fd` (which must be non-blocking and stay open while this
    /// lives) with the shared reactor, initialising the reactor if needed.
    ///
    /// # Errors
    /// The reactor could not start, or the selector refused the fd.
    pub fn register(fd: RawFd) -> io::Result<Self> {
        let reactor = Reactor::get()?;
        let token = super::accept::next_token();
        let readiness = SharedReadiness::new(
            fd,
            Arc::clone(&reactor),
            token,
            Interest::READABLE | Interest::WRITABLE,
        )?;
        Ok(Self {
            readiness,
            reactor,
            fd,
            token,
        })
    }

    /// Forget the readiness seen so far. Call before each read or write
    /// attempt; see the module docs.
    pub fn clear(&self) {
        self.readiness.clear(Ready::READABLE.union(Ready::WRITABLE));
    }
}

impl EventReadiness for FdReadiness {
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        self.readiness.is_ready(dur)
    }
}

impl Drop for FdReadiness {
    fn drop(&mut self) {
        // The fd may already be closed (its stream dropped first); the
        // selector forgets closed fds on its own, so a failure is harmless.
        if let Err(e) = self.reactor.deregister(self.fd, self.token) {
            tracing::trace!(error = %e, fd = self.fd, "fd readiness deregister");
        }
    }
}

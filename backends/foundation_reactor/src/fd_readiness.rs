//! `FdReadiness` — park a valtron task on a socket through the shared reactor.
//!
//! WHY: a task that reads a non-blocking socket gets `WouldBlock` when nothing
//! has arrived. Without a readiness source it can only re-poll on a timer (the
//! h2 server polled every 10 ms). The shared reactor already watches fds for
//! readiness; this lets any task that has a raw fd park on it, without owning
//! a `RegisteredFd` (the fd stays owned by its stream).
//!
//! WHAT: [`FdReadiness`] registers an fd for read readiness (write readiness is
//! opt-in, see [`FdReadiness::set_writable`]) and is an `EventReadiness` for
//! `TaskStatus::Depends`. It deregisters on drop.
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

use crate::poll::{Interest, Token};
use crate::reactor::{Reactor, SharedReadiness};
use crate::ready::Ready;

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
    /// lives) with the shared reactor for read readiness, initialising the
    /// reactor if needed. Write readiness is off by default: an idle socket is
    /// always writable, so watching it constantly spins the reactor.
    ///
    /// # Errors
    /// The reactor could not start, or the selector refused the fd.
    pub fn register(fd: RawFd) -> io::Result<Self> {
        let reactor = Reactor::get()?;
        let token = crate::token::next_token();
        let readiness = SharedReadiness::new(
            fd,
            Arc::clone(&reactor),
            token,
            Interest::READABLE,
        )?;
        Ok(Self {
            readiness,
            reactor,
            fd,
            token,
        })
    }

    /// Watch the socket for write readiness too (`true`) or read only
    /// (`false`). Callers turn write readiness on only while they have output
    /// queued to flush; an idle socket is always writable, so leaving it on
    /// makes the reactor report the fd ready forever.
    ///
    /// # Errors
    /// The selector could not re-register the fd.
    pub fn set_writable(&self, watch: bool) -> io::Result<()> {
        let interest = if watch {
            Interest::READABLE | Interest::WRITABLE
        } else {
            Interest::READABLE
        };
        self.reactor.reregister(self.fd, self.token, interest)
    }

    /// Forget the readiness seen so far. Call before each read or write
    /// attempt; see the module docs.
    pub fn clear(&self) {
        self.readiness.clear(Ready::READABLE.union(Ready::WRITABLE));
    }

    /// The latched readiness flags for this fd (see [`Reactor::readiness`]).
    ///
    /// WHY: a hangup watcher must tell `READ_CLOSED`/`ERROR` (the peer shut the
    /// connection) from plain `READABLE` (more data), so it does not mistake a
    /// pipelined request for a close.
    #[must_use]
    pub fn ready(&self) -> Ready {
        self.reactor.readiness(self.token)
    }

    /// Arm `waker` so the reactor's drain thread wakes it on the next
    /// readiness edge for this fd (Feature 29 / Feature 03).
    ///
    /// WHY: a native-executor task (tokio/smol) parks by returning `Pending`
    /// from `Stream::poll_next`, so it needs a `std::task::Waker`, not the
    /// valtron `EventReadiness` path.
    ///
    /// WHAT: stores the waker on the reactor entry. Returns whether the fd is
    /// still registered (a dropped [`FdReadiness`] returns `false`).
    ///
    /// HOW: follow the register-before-check protocol — call [`FdReadiness::clear`]
    /// first to disarm any stale edge, register the waker, then read once more
    /// before parking. Clearing on the next read disarms the waker again.
    pub fn register_waker(&self, waker: &std::task::Waker) -> bool {
        self.reactor.register_waker(self.token, waker.clone())
    }

    /// Snapshot the reactor's event generation. A non-task thread (the accept
    /// loop) snapshots this before it reads its fd, then passes it to
    /// [`FdReadiness::wait_for_events`] so it only sleeps when nothing changed
    /// since — the lost-wakeup guard (Feature 05). See [`Reactor::events_generation`].
    #[must_use]
    pub fn events_generation(&self) -> u64 {
        self.reactor.events_generation()
    }

    /// Block the calling thread until the reactor latches new readiness (its
    /// generation advances past `since`) or `timeout` elapses; returns the
    /// generation observed on wake. A dedicated accept-loop thread uses this to
    /// park on a listener instead of a fixed sleep. See [`Reactor::wait_for_events`].
    pub fn wait_for_events(&self, since: u64, timeout: Duration) -> u64 {
        self.reactor.wait_for_events(since, timeout)
    }
}

impl EventReadiness for FdReadiness {
    fn is_ready(&self, dur: Option<Duration>) -> bool {
        self.readiness.is_ready(dur)
    }

    fn clear(&self) {
        self.readiness.clear(Ready::READABLE.union(Ready::WRITABLE));
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

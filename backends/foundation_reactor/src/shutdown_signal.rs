//! `ShutdownSignal` — a wake-capable shutdown flag.
//!
//! WHY: `foundation_core::synca::OnSignal` is a passive atomic flag. A thread
//! parked in `Reactor::wait_for_events` sees its change only on the next poll,
//! so a shutdown waits out the whole park timeout (the accept loop's
//! `accept_shutdown_poll`).
//!
//! WHAT: a flag that also carries a wake link to the shared reactor. On unix,
//! [`ShutdownSignal::register_wake`] stores the reactor; a later
//! [`ShutdownSignal::turn_on`] wakes any thread parked in
//! `Reactor::wait_for_events` at once. On other platforms the reactor does not
//! exist, so the signal is a plain flag (the same as `OnSignal`).
//!
//! HOW: `turn_on` sets the flag (a cheap atomic) and then calls
//! `Reactor::wake_waiters`, which bumps the generation and notifies. The flag
//! is set before the wake, so a woken waiter always observes it.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(unix)]
use std::sync::{Arc, OnceLock};

#[cfg(unix)]
use crate::reactor::Reactor;

/// A shutdown flag that can wake a reactor-parked thread. See the module docs.
#[derive(Debug, Default)]
pub struct ShutdownSignal {
    flag: AtomicBool,
    /// The shared reactor this signal wakes on `turn_on`. Set once by
    /// [`ShutdownSignal::register_wake`]; absent on non-unix.
    #[cfg(unix)]
    reactor: OnceLock<Arc<Reactor>>,
}

impl ShutdownSignal {
    /// A new, not-yet-set signal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return `true` when the signal is set.
    #[inline]
    pub fn probe(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    /// Set the signal and wake the linked reactor (unix). Idempotent: returns
    /// `true` only on the false-to-true change, and wakes only that once.
    #[inline]
    pub fn turn_on(&self) -> bool {
        if self
            .flag
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
        {
            #[cfg(unix)]
            if let Some(reactor) = self.reactor.get() {
                reactor.wake_waiters();
            }
            true
        } else {
            false
        }
    }

    /// Clear the signal. Returns `true` on the true-to-false change.
    #[inline]
    pub fn turn_off(&self) -> bool {
        self.flag
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }

    /// Link this signal to the shared reactor so a later [`ShutdownSignal::turn_on`]
    /// wakes threads parked in `Reactor::wait_for_events`. Best-effort: on
    /// failure the signal stays a plain flag and waiters fall back to their poll
    /// timeout. On non-unix this is a no-op that always succeeds.
    pub fn register_wake(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            let reactor = Reactor::get()?;
            let _ = self.reactor.set(reactor);
        }
        Ok(())
    }
}

/// Linux epoll selector.
use super::super::super::super::event::Events;
use crate::poll::{Interest, Token};

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The internal epoll selector.
///
/// Uses `Arc` + `Mutex` for thread safety — multiple threads can
/// register/deregister fds concurrently while `poll()` runs on one thread.
pub struct Selector {
    /// The epoll file descriptor.
    epoll: OwnedFd,
    /// Waker eventfd, if registered.
    waker_fd: Mutex<Option<OwnedFd>>,
    /// Token associated with the waker.
    waker_token: Mutex<Option<Token>>,
}

impl std::fmt::Debug for Selector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("epoll::Selector")
            .field("epoll_fd", &self.epoll.as_raw_fd())
            .field("waker_fd", &self.waker_fd)
            .field("waker_token", &self.waker_token)
            .finish()
    }
}

impl Selector {
    /// Create a new epoll selector.
    ///
    /// # Errors
    /// Returns the operating-system error when `epoll_create1` fails.
    pub fn new() -> io::Result<Self> {
        let epoll_fd = unsafe {
            let fd = libc::epoll_create1(libc::EPOLL_CLOEXEC);
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            OwnedFd::from_raw_fd(fd)
        };

        Ok(Self {
            epoll: epoll_fd,
            waker_fd: Mutex::new(None),
            waker_token: Mutex::new(None),
        })
    }

    /// Register a raw file descriptor with the epoll selector.
    ///
    /// # Errors
    /// Returns the operating-system error when `epoll_ctl` fails.
    pub fn register_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let mut event = libc::epoll_event {
            events: epoll_events_from_interest(interest),
            u64: token.0 as u64,
        };

        let r = unsafe {
            libc::epoll_ctl(
                self.epoll.as_raw_fd(),
                libc::EPOLL_CTL_ADD,
                fd,
                &raw mut event,
            )
        };

        if r < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Re-register a raw file descriptor with new token and/or interest.
    ///
    /// # Errors
    /// Returns the operating-system error when `epoll_ctl` fails.
    pub fn reregister_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let mut event = libc::epoll_event {
            events: epoll_events_from_interest(interest),
            u64: token.0 as u64,
        };

        let r = unsafe {
            libc::epoll_ctl(
                self.epoll.as_raw_fd(),
                libc::EPOLL_CTL_MOD,
                fd,
                &raw mut event,
            )
        };

        if r < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Deregister a raw file descriptor from the epoll selector.
    ///
    /// # Errors
    /// Returns the operating-system error when `epoll_ctl` fails.
    pub fn deregister_fd(&self, fd: RawFd) -> io::Result<()> {
        let r = unsafe {
            libc::epoll_ctl(
                self.epoll.as_raw_fd(),
                libc::EPOLL_CTL_DEL,
                fd,
                std::ptr::null_mut::<libc::epoll_event>(),
            )
        };

        if r < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Register the waker (eventfd).
    ///
    /// # Errors
    /// Returns the operating-system error when the eventfd cannot be created
    /// or registered.
    ///
    /// # Panics
    /// Panics when a waker mutex is poisoned.
    pub fn register_waker(&self, token: Token) -> io::Result<()> {
        let efd = unsafe {
            let fd = libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK);
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            OwnedFd::from_raw_fd(fd)
        };

        // Register the eventfd with epoll for readability
        self.register_fd(efd.as_raw_fd(), token, Interest::READABLE)?;

        *self.waker_fd.lock().unwrap() = Some(efd);
        *self.waker_token.lock().unwrap() = Some(token);
        Ok(())
    }

    /// Wake the poll selector by writing to the eventfd.
    ///
    /// # Errors
    /// Returns an error when no waker is registered, or the operating-system
    /// error when the write fails.
    ///
    /// # Panics
    /// Panics when a waker mutex is poisoned.
    pub fn wake(&self, _token: Token) -> io::Result<()> {
        let waker_fd = self.waker_fd.lock().unwrap();
        if let Some(ref fd) = *waker_fd {
            let value: u64 = 1;
            let r = unsafe {
                libc::write(
                    fd.as_raw_fd(),
                    std::ptr::from_ref(&value).cast::<libc::c_void>(),
                    std::mem::size_of_val(&value),
                )
            };
            if r < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        } else {
            Err(io::Error::other("waker not registered"))
        }
    }

    /// Clear the waker by reading the eventfd.
    ///
    /// # Panics
    /// Panics when the waker mutex is poisoned.
    pub fn clear_waker(&self) {
        let waker_fd = self.waker_fd.lock().unwrap();
        if let Some(ref fd) = *waker_fd {
            let mut value: u64 = 0;
            unsafe {
                libc::read(
                    fd.as_raw_fd(),
                    std::ptr::from_mut(&mut value).cast::<libc::c_void>(),
                    std::mem::size_of_val(&value),
                );
            }
        }
    }

    /// `EVFILT_VNODE` is not supported on epoll. Returns `Unsupported`.
    ///
    /// # Errors
    /// Always returns `Unsupported`.
    pub fn register_vnode(&self, _fd: RawFd, _token: Token) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "EVFILT_VNODE not supported on epoll",
        ))
    }

    /// `EVFILT_VNODE` deregistration — no-op on epoll.
    ///
    /// # Errors
    /// Never returns an error.
    pub fn deregister_vnode(&self, _fd: RawFd) -> io::Result<()> {
        Ok(())
    }

    /// Wait for readiness events.
    ///
    /// The call waits until an event arrives or `timeout` expires. A signal
    /// with a handler installed interrupts `epoll_wait` with `EINTR`; the wait
    /// then resumes for the time that is left. A signal must never make the
    /// call return before the timeout the caller asked for.
    ///
    /// # Errors
    /// Returns the operating-system error when `epoll_wait` fails. An
    /// interrupted wait does not return an error.
    ///
    /// # Panics
    /// Panics when the waker mutex is poisoned.
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        let deadline = timeout.map(|d| os_now() + d);

        loop {
            let timeout_ms = deadline.map_or(-1, |d| remaining_ms(d, os_now()));

            // Get pointer to the Vec's buffer and its capacity (not length).
            // The Vec has capacity but length 0 — epoll_wait writes directly into
            // the buffer, and we update the length afterward.
            let (ptr, cap) = events.as_mut_ptr_and_cap();

            // The capacity cannot exceed `c_int::MAX` in practice. The cap
            // keeps the call valid if it ever did.
            let max_events = libc::c_int::try_from(cap).unwrap_or(libc::c_int::MAX);

            let n = unsafe {
                libc::epoll_wait(
                    self.epoll.as_raw_fd(),
                    ptr.cast::<libc::epoll_event>(),
                    max_events,
                    timeout_ms,
                )
            };

            if n < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    // A caught signal cut the wait short. Run out the clock
                    // instead of reporting a timeout the caller did not ask for.
                    events.clear();
                    if deadline.is_none_or(|deadline| os_now() < deadline) {
                        continue;
                    }
                    return Ok(());
                }
                return Err(err);
            }

            // set_len is safe here — n is guaranteed ≤ capacity by epoll_wait.
            // The branch above returns on a negative `n`, so the conversion
            // cannot fail.
            events.set_len(usize::try_from(n).unwrap_or(0));
            break;
        }

        // Check if the waker fired and clear it
        let waker_token = self.waker_token.lock().unwrap();
        if let Some(wt) = *waker_token {
            let is_waker = events.iter().any(|e| e.token() == wt);
            if is_waker {
                drop(waker_token);
                self.clear_waker();
            }
        }

        Ok(())
    }
}

/// The real monotonic clock for the kernel wait arithmetic.
///
/// The selector blocks in the operating system, so the deadline it recomputes
/// after an interrupt must use the operating system's monotonic clock. A
/// simulated clock would break the real block. This is the same boundary as
/// the executor's blocking waits (GitHub issue #102).
fn os_now() -> Instant {
    use foundation_core::valtron::Clock as _;
    foundation_core::valtron::MonotonicClock::new().now()
}

/// Milliseconds left until `deadline`, for `epoll_wait`'s timeout argument.
///
/// Rounds up, so a remainder below one millisecond still waits instead of
/// turning into a zero timeout, and caps at `c_int::MAX` (about 24 days).
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn remaining_ms(deadline: Instant, now: Instant) -> libc::c_int {
    let remaining = deadline.saturating_duration_since(now);
    let millis = remaining.as_nanos().div_ceil(1_000_000);
    millis.min(libc::c_int::MAX as u128) as libc::c_int
}

/// Convert `Interest` to epoll event flags.
fn epoll_events_from_interest(interest: Interest) -> u32 {
    let mut events: u32 = 0;

    // Use edge-triggered mode — we track readiness ourselves.
    // `EPOLLET` is bit 31. Its `c_int` form is negative, so `cast_unsigned`
    // reinterprets the same bits as `u32`.
    events |= libc::EPOLLET.cast_unsigned();

    if interest.is_readable() {
        events |= libc::EPOLLIN as u32;
        // Also track read hangups for closed detection
        events |= libc::EPOLLRDHUP as u32;
    }

    if interest.is_writable() {
        events |= libc::EPOLLOUT as u32;
    }

    events
}

/// The raw fd type on Linux.
pub type RawFd = std::os::unix::io::RawFd;

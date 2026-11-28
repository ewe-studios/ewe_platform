/// macOS/BSD kqueue selector.
use crate::poll::event::Event;
use crate::poll::{Events, Interest, Registry, Token};

use std::collections::HashMap;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The internal kqueue selector.
pub struct Selector {
    /// The kqueue file descriptor.
    kq: OwnedFd,
    /// Waker token, if registered.
    waker_token: Mutex<Option<Token>>,
    /// Per-fd read-edge tracking for pipes only.
    ///
    /// macOS kqueue reports `EVFILT_READ` level-triggered for pipes despite
    /// `EV_CLEAR`, so the selector records the byte count delivered on each
    /// readable pipe edge and suppresses a re-fire until the count grows (new
    /// data) or the caller re-arms it.
    ///
    /// Sockets and every other descriptor kind are edge-triggered by the
    /// kernel, so they are not tracked. Suppressing a socket event loses an
    /// edge that arrived before the caller re-armed, and no later re-arm can
    /// recover it (GitHub issue #1218). The map is therefore bounded by the
    /// open pipe count.
    read_edges: Mutex<HashMap<RawFd, usize>>,
}

/// The raw fd type on macOS/BSD.
pub type RawFd = std::os::unix::io::RawFd;

/// `EV_SET` is a C preprocessor macro in `<sys/event.h>`, so `libc` has no
/// binding for it. Initialise the `kevent` in place instead.
///
/// # Safety
///
/// The caller must ensure `ident` and `udata` remain valid for as long as the
/// kernel keeps this registration in the kqueue.
#[inline]
unsafe fn ev_set(
    ev: &mut libc::kevent,
    ident: libc::uintptr_t,
    filter: i16,
    flags: u16,
    filter_flags: u32,
    data: libc::intptr_t,
    udata: *mut libc::c_void,
) {
    ev.ident = ident;
    ev.filter = filter;
    ev.flags = flags;
    ev.fflags = filter_flags;
    ev.data = data;
    ev.udata = udata;
}

/// Convert a file descriptor to the `uintptr_t` that kqueue uses as an `ident`.
///
/// A file descriptor is never negative. A negative value is an invalid input,
/// so report it instead of wrapping it to a large unsigned number.
fn fd_ident(fd: RawFd) -> io::Result<libc::uintptr_t> {
    libc::uintptr_t::try_from(fd)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "negative file descriptor"))
}

/// Whether `fd` is a pipe (FIFO).
///
/// Only pipes need the selector's software read-edge tracking, because macOS
/// kqueue reports their `EVFILT_READ` level-triggered despite `EV_CLEAR`.
/// Sockets and other descriptor kinds are edge-triggered by the kernel.
///
/// A failed `fstat` counts as "not a pipe". Delivering an extra event is safe:
/// the reader re-checks the fd and gets `WouldBlock`. Suppressing a real socket
/// edge would lose the wake (GitHub issue #1218).
fn is_fifo(fd: RawFd) -> bool {
    // SAFETY: `stat` is a valid output buffer for `fstat`, and `fd` is a live
    // descriptor that the caller owns and asked to register.
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    let result = unsafe { libc::fstat(fd, &raw mut stat) };
    result == 0 && (stat.st_mode & libc::S_IFMT) == libc::S_IFIFO
}

impl Selector {
    /// Create a new kqueue selector.
    ///
    /// # Errors
    /// Returns the operating-system error when `kqueue(2)` fails.
    pub fn new() -> io::Result<Self> {
        let kq_fd = unsafe {
            let fd = libc::kqueue();
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            OwnedFd::from_raw_fd(fd)
        };

        Ok(Self {
            kq: kq_fd,
            waker_token: Mutex::new(None),
            read_edges: Mutex::new(HashMap::new()),
        })
    }

    /// Create a kqueue selector wrapped in an `Arc`, with its `Registry`.
    ///
    /// # Errors
    /// Returns the operating-system error when `kqueue(2)` fails.
    pub fn new_with_registry() -> io::Result<(Arc<Self>, Registry)> {
        let selector = Arc::new(Self::new()?);
        let registry = Registry {
            selector: selector.clone(),
        };
        Ok((selector, registry))
    }

    /// Register an `EVFILT_VNODE` filter for file watching on the given fd.
    ///
    /// Unlike `register_fd` (which uses `EVFILT_READ/WRITE`), this registers
    /// `EVFILT_VNODE` to track file system changes: write, delete, extend, rename, revoke.
    /// Multiple filters can coexist on the same kqueue — events are distinguished
    /// by their filter field in the returned kevent.
    ///
    /// The fd must remain open for the lifetime of the watch — kqueue tracks by fd,
    /// not by path. If the fd is closed, the filter is automatically removed.
    ///
    /// # Errors
    /// Returns an error if the descriptor is negative or the kernel refuses the
    /// filter.
    pub fn register_vnode(&self, fd: RawFd, token: Token) -> io::Result<()> {
        let ident = fd_ident(fd)?;
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        unsafe {
            ev_set(
                &mut event,
                ident,
                libc::EVFILT_VNODE,
                libc::EV_ADD | libc::EV_CLEAR,
                libc::NOTE_WRITE
                    | libc::NOTE_DELETE
                    | libc::NOTE_EXTEND
                    | libc::NOTE_RENAME
                    | libc::NOTE_REVOKE,
                0,
                token.0 as *mut libc::c_void,
            );
        }

        let r = unsafe {
            libc::kevent(
                self.kq.as_raw_fd(),
                &raw const event,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };

        if r < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Deregister an `EVFILT_VNODE` filter for the given fd.
    ///
    /// # Errors
    /// Returns an error if the descriptor is negative or the kernel reports a
    /// failure other than a missing filter.
    pub fn deregister_vnode(&self, fd: RawFd) -> io::Result<()> {
        let ident = fd_ident(fd)?;
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        unsafe {
            ev_set(
                &mut event,
                ident,
                libc::EVFILT_VNODE,
                libc::EV_DELETE,
                0,
                0,
                std::ptr::null_mut(),
            );
        }

        let r = unsafe {
            libc::kevent(
                self.kq.as_raw_fd(),
                &raw const event,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };

        // EV_DELETE may return ENOENT if the filter doesn't exist — that's fine
        if r < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::ENOENT) {
                return Err(err);
            }
        }

        Ok(())
    }

    /// Register a raw file descriptor with the kqueue selector.
    ///
    /// # Errors
    /// Returns an error if the descriptor is negative or the kernel refuses a
    /// filter.
    ///
    /// # Panics
    /// Panics if the read-edge lock is poisoned.
    pub fn register_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let ident = fd_ident(fd)?;
        let mut events: [libc::kevent; 2] =
            [unsafe { std::mem::zeroed() }, unsafe { std::mem::zeroed() }];
        let mut count = 0;

        if interest.is_readable() {
            unsafe {
                ev_set(
                    &mut events[count],
                    ident,
                    libc::EVFILT_READ,
                    libc::EV_ADD | libc::EV_CLEAR, // edge-triggered
                    0,
                    0,
                    token.0 as *mut libc::c_void,
                );
            }
            count += 1;
        }

        if interest.is_writable() {
            unsafe {
                ev_set(
                    &mut events[count],
                    ident,
                    libc::EVFILT_WRITE,
                    libc::EV_ADD | libc::EV_CLEAR,
                    0,
                    0,
                    token.0 as *mut libc::c_void,
                );
            }
            count += 1;
        }

        if count > 0 {
            let count = libc::c_int::try_from(count)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "too many kevents"))?;
            let r = unsafe {
                libc::kevent(
                    self.kq.as_raw_fd(),
                    events.as_ptr(),
                    count,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                )
            };
            if r < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        // Track pipes only; see `read_edges`. The bookkeeping follows the
        // kernel call so a failed registration leaves no entry behind, and a
        // fresh registration starts a fresh edge.
        {
            let mut edges = self.read_edges.lock().unwrap();
            if interest.is_readable() && is_fifo(fd) {
                edges.insert(fd, 0);
            } else {
                edges.remove(&fd);
            }
        }

        Ok(())
    }

    /// Re-register a raw file descriptor with new token and/or interest.
    ///
    /// # Errors
    /// Returns an error if a filter cannot be deleted or the new filters cannot
    /// be registered.
    pub fn reregister_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        // kqueue `EV_ADD` only adds or updates a filter; it never removes one.
        // A dropped interest must be `EV_DELETE`d explicitly or the filter keeps
        // firing — an idle socket is always writable, so a stale `EVFILT_WRITE`
        // would busy-loop the drain thread. Delete each filter not in the new
        // interest (a missing filter yields `ENOENT`, which is fine), then add
        // the ones that are.
        self.delete_filter(fd, libc::EVFILT_READ)?;
        self.delete_filter(fd, libc::EVFILT_WRITE)?;
        self.register_fd(fd, token, interest)
    }

    /// Delete one kqueue filter, tolerating `ENOENT` (the filter is absent).
    fn delete_filter(&self, fd: RawFd, filter: i16) -> io::Result<()> {
        let ident = fd_ident(fd)?;
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        unsafe {
            ev_set(
                &mut event,
                ident,
                filter,
                libc::EV_DELETE,
                0,
                0,
                std::ptr::null_mut(),
            );
        }
        let r = unsafe {
            libc::kevent(
                self.kq.as_raw_fd(),
                &raw const event,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };
        if r < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::ENOENT) {
                return Err(err);
            }
        }
        Ok(())
    }

    /// Deregister a raw file descriptor from the kqueue selector.
    ///
    /// # Errors
    /// Returns an error if the descriptor is negative or the kernel reports a
    /// failure other than a missing filter.
    ///
    /// # Panics
    /// Panics if the read-edge lock is poisoned.
    pub fn deregister_fd(&self, fd: RawFd) -> io::Result<()> {
        let ident = fd_ident(fd)?;
        let mut events: [libc::kevent; 2] =
            [unsafe { std::mem::zeroed() }, unsafe { std::mem::zeroed() }];
        let mut count = 0;

        // Delete both read and write filters
        unsafe {
            ev_set(
                &mut events[count],
                ident,
                libc::EVFILT_READ,
                libc::EV_DELETE,
                0,
                0,
                std::ptr::null_mut(),
            );
        }
        count += 1;

        unsafe {
            ev_set(
                &mut events[count],
                ident,
                libc::EVFILT_WRITE,
                libc::EV_DELETE,
                0,
                0,
                std::ptr::null_mut(),
            );
        }
        count += 1;

        let count = libc::c_int::try_from(count)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "too many kevents"))?;
        let r = unsafe {
            libc::kevent(
                self.kq.as_raw_fd(),
                events.as_ptr(),
                count,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };

        // EV_DELETE may return ENOENT if the filter doesn't exist — that's fine
        if r < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::ENOENT) {
                return Err(err);
            }
        }

        self.read_edges.lock().unwrap().remove(&fd);

        Ok(())
    }

    /// Re-arm the read edge for a tracked pipe after the caller drained it.
    ///
    /// Resets the recorded byte count so the next readable event is delivered
    /// as a fresh edge. Sockets are not tracked, so this is a no-op for them:
    /// the kernel already delivers their next edge on its own, and a re-arm
    /// could never recover an edge that the selector did not suppress.
    /// The reactor's clear path calls this after a consumer clears its
    /// readiness; it is public so a test can drive the selector directly.
    ///
    /// # Panics
    /// Panics if the read-edge lock is poisoned.
    pub fn rearm_readable(&self, fd: RawFd) {
        if let Some(delivered) = self.read_edges.lock().unwrap().get_mut(&fd) {
            *delivered = 0;
        }
    }

    /// Register the waker (`EVFILT_USER`).
    ///
    /// # Errors
    /// Returns the kernel error when the user event cannot be added.
    ///
    /// # Panics
    /// Panics if the waker-token lock is poisoned.
    pub fn register_waker(&self, token: Token) -> io::Result<()> {
        let event = unsafe {
            let mut ev: libc::kevent = std::mem::zeroed();
            ev_set(
                &mut ev,
                2887, // magic identifier for the waker
                libc::EVFILT_USER,
                libc::EV_ADD | libc::EV_CLEAR,
                0,
                0,
                token.0 as *mut libc::c_void,
            );
            ev
        };

        let r = unsafe {
            libc::kevent(
                self.kq.as_raw_fd(),
                &raw const event,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };

        if r < 0 {
            Err(io::Error::last_os_error())
        } else {
            *self.waker_token.lock().unwrap() = Some(token);
            Ok(())
        }
    }

    /// Wake the poll selector by triggering the `EVFILT_USER` event.
    ///
    /// # Errors
    /// Returns the kernel error when the trigger fails, or an error when no
    /// waker is registered.
    ///
    /// # Panics
    /// Panics if the waker-token lock is poisoned.
    pub fn wake(&self, _token: Token) -> io::Result<()> {
        let waker_token = self.waker_token.lock().unwrap();
        if let Some(wt) = *waker_token {
            let mut event: libc::kevent = unsafe { std::mem::zeroed() };
            unsafe {
                ev_set(
                    &mut event,
                    2887,
                    libc::EVFILT_USER,
                    0,
                    libc::NOTE_TRIGGER,
                    0,
                    wt.0 as *mut libc::c_void,
                );
            }

            let r = unsafe {
                libc::kevent(
                    self.kq.as_raw_fd(),
                    &raw const event,
                    1,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                )
            };

            if r < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        } else {
            Err(io::Error::other("waker not registered"))
        }
    }

    /// Wait for readiness events.
    ///
    /// The call waits until an event arrives or `timeout` expires. A signal
    /// with a handler installed interrupts `kevent` with `EINTR`; the wait
    /// then resumes for the time that is left. A signal must never make the
    /// call return before the timeout the caller asked for.
    ///
    /// # Errors
    /// Returns the `kevent` error for any failure other than interruption.
    ///
    /// # Panics
    /// Panics if an internal lock is poisoned.
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        let deadline = timeout.map(|d| os_now() + d);

        loop {
            let ts = deadline.map(|d| remaining_timespec(d, os_now()));
            let ts_ptr = ts
                .as_ref()
                .map_or(std::ptr::null(), std::ptr::from_ref::<libc::timespec>);

            // Get pointer to the Vec's buffer and its capacity (not length)
            let (ptr, cap) = events.as_mut_ptr_and_cap();
            let nchanges = libc::c_int::try_from(cap)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "too many events"))?;

            let n = unsafe {
                libc::kevent(
                    self.kq.as_raw_fd(),
                    std::ptr::null(),
                    0,
                    ptr.cast::<libc::kevent>(),
                    nchanges,
                    ts_ptr,
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

            // `kevent` never returns a negative count here, so the conversion
            // cannot fail.
            events.set_len(usize::try_from(n).unwrap_or_default());
            break;
        }

        // macOS kqueue reports `EVFILT_READ` level-triggered for pipes despite
        // `EV_CLEAR`, so for a tracked pipe the selector enforces the read edge
        // itself: it delivers a readable event once per edge and suppresses a
        // re-fire while the byte count has not grown (no new data) since the
        // last delivered event. Sockets are not tracked — the kernel is
        // edge-triggered for them, and suppressing their events dropped a real
        // edge (GitHub issue #1218). Read-closed and error events are never
        // suppressed — they are control signals, not data.
        let keep = {
            let len = events.len();
            let (ptr, _cap) = events.as_mut_ptr_and_cap();
            let slice = unsafe { std::slice::from_raw_parts_mut::<Event>(ptr, len) };
            let mut edges = self.read_edges.lock().unwrap();
            let mut keep = 0usize;
            for i in 0..slice.len() {
                let ev = slice[i];
                let plain_readable = ev.is_readable() && !ev.is_read_closed() && !ev.is_error();
                if plain_readable {
                    // `ident` carries the registered fd. An untracked fd (a
                    // socket or another descriptor kind) is edge-triggered by
                    // the kernel, so keep its event without bookkeeping. A
                    // value that does not fit an fd is not one this selector
                    // tracks either.
                    if let Ok(fd) = RawFd::try_from(ev.ident()) {
                        if let Some(delivered) = edges.get_mut(&fd) {
                            let data = usize::try_from(ev.data()).unwrap_or_default();
                            if data <= *delivered {
                                continue;
                            }
                            *delivered = data;
                        }
                    }
                }
                slice[keep] = ev;
                keep += 1;
            }
            keep
        };
        events.set_len(keep);

        // Check if the waker fired and clear it
        let waker_token = self.waker_token.lock().unwrap();
        if let Some(wt) = *waker_token {
            let is_waker = events.iter().any(|e| e.token() == wt);
            if is_waker {
                drop(waker_token);
                // Trigger another EV_SET to clear the waker state
                let mut event: libc::kevent = unsafe { std::mem::zeroed() };
                unsafe {
                    ev_set(
                        &mut event,
                        2887,
                        libc::EVFILT_USER,
                        0,
                        0, // no NOTE_TRIGGER — just acknowledge
                        0,
                        std::ptr::null_mut(),
                    );
                }
                // No-op kevent to acknowledge
            }
        }

        Ok(())
    }
}

/// The time left until `deadline`, as a `timespec` for `kevent`.
///
/// A zero value means "do not block", so the last round after a signal waits
/// for whatever is left of the caller's timeout, not for the whole timeout
/// again.
/// The real monotonic clock for the kernel wait arithmetic.
///
/// The selector blocks in the operating system, so the deadline it recomputes
/// after an interrupt must use the operating system's monotonic clock. A
/// simulated clock would break the real block. This is the same boundary as
/// the executor's blocking waits (GitHub issue #102).
fn os_now() -> Instant {
    use foundation_context::Clock as _;
    foundation_context::MonotonicClock::new().now()
}

fn remaining_timespec(deadline: Instant, now: Instant) -> libc::timespec {
    let remaining = deadline.saturating_duration_since(now);
    libc::timespec {
        tv_sec: libc::time_t::try_from(remaining.as_secs()).unwrap_or(libc::time_t::MAX),
        tv_nsec: libc::c_long::from(remaining.subsec_nanos()),
    }
}

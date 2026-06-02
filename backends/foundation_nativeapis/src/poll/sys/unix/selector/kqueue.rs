/// macOS/BSD kqueue selector.

use crate::poll::event::Event;
use crate::poll::Events;
use crate::poll::{Interest, Registry, Token};

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The internal kqueue selector.
pub struct Selector {
    /// The kqueue file descriptor.
    kq: OwnedFd,
    /// Waker token, if registered.
    waker_token: Mutex<Option<Token>>,
}

/// The raw fd type on macOS/BSD.
pub type RawFd = std::os::unix::io::RawFd;

impl Selector {
    /// Create a new kqueue selector.
    pub fn new_with_registry() -> io::Result<(Arc<Self>, Registry)> {
        let kq_fd = unsafe {
            let fd = libc::kqueue();
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            OwnedFd::from_raw_fd(fd)
        };

        let selector = Arc::new(Self {
            kq: kq_fd,
            waker_token: Mutex::new(None),
        });

        let registry = Registry {
            selector: selector.clone(),
        };

        Ok((selector, registry))
    }

    /// Register a raw file descriptor with the kqueue selector.
    pub fn register_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let mut events: [libc::kevent; 2] = [unsafe { std::mem::zeroed() }, unsafe {
            std::mem::zeroed()
        }];
        let mut count = 0;

        if interest.is_readable() {
            unsafe {
                libc::EV_SET(
                    &mut events[count],
                    fd as libc::uintptr_t,
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
                libc::EV_SET(
                    &mut events[count],
                    fd as libc::uintptr_t,
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
            let r = unsafe {
                libc::kevent(
                    self.kq.as_raw_fd(),
                    events.as_ptr(),
                    count as libc::c_int,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                )
            };
            if r < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(())
    }

    /// Re-register a raw file descriptor with new token and/or interest.
    pub fn reregister_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        // On kqueue, re-registering is the same as registering — it updates the existing entry.
        self.register_fd(fd, token, interest)
    }

    /// Deregister a raw file descriptor from the kqueue selector.
    pub fn deregister_fd(&self, fd: RawFd) -> io::Result<()> {
        let mut events: [libc::kevent; 2] = [unsafe { std::mem::zeroed() }, unsafe {
            std::mem::zeroed()
        }];
        let mut count = 0;

        // Delete both read and write filters
        unsafe {
            libc::EV_SET(
                &mut events[count],
                fd as libc::uintptr_t,
                libc::EVFILT_READ,
                libc::EV_DELETE,
                0,
                0,
                std::ptr::null_mut(),
            );
        }
        count += 1;

        unsafe {
            libc::EV_SET(
                &mut events[count],
                fd as libc::uintptr_t,
                libc::EVFILT_WRITE,
                libc::EV_DELETE,
                0,
                0,
                std::ptr::null_mut(),
            );
        }
        count += 1;

        let r = unsafe {
            libc::kevent(
                self.kq.as_raw_fd(),
                events.as_ptr(),
                count as libc::c_int,
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

    /// Register the waker (EVFILT_USER).
    pub fn register_waker(&self, token: Token) -> io::Result<()> {
        let mut event = unsafe {
            let mut ev: libc::kevent = std::mem::zeroed();
            libc::EV_SET(
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
                &event,
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

    /// Wake the poll selector by triggering the EVFILT_USER event.
    pub fn wake(&self, _token: Token) -> io::Result<()> {
        let waker_token = self.waker_token.lock().unwrap();
        if waker_token.is_some() {
            let mut event: libc::kevent = unsafe { std::mem::zeroed() };
            unsafe {
                libc::EV_SET(
                    &mut event,
                    2887,
                    libc::EVFILT_USER,
                    0,
                    libc::NOTE_TRIGGER,
                    0,
                    std::ptr::null_mut(),
                );
            }

            let r = unsafe {
                libc::kevent(
                    self.kq.as_raw_fd(),
                    &event,
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
            Err(io::Error::new(
                io::ErrorKind::Other,
                "waker not registered",
            ))
        }
    }

    /// Wait for readiness events.
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
        let ts_ptr: *const libc::timespec = if let Some(d) = timeout {
            ts.tv_sec = d.as_secs() as libc::time_t;
            ts.tv_nsec = d.subsec_nanos() as libc::c_long;
            &ts
        } else {
            std::ptr::null()
        };

        let kevents = events.as_mut_slice();

        let n = unsafe {
            libc::kevent(
                self.kq.as_raw_fd(),
                std::ptr::null(),
                0,
                kevents.as_mut_ptr() as *mut libc::kevent,
                kevents.len() as libc::c_int,
                if ts_ptr.is_null() {
                    std::ptr::null()
                } else {
                    ts_ptr
                },
            )
        };

        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                events.clear();
                return Ok(());
            }
            return Err(err);
        }

        unsafe {
            events.set_len(n as usize);
        }

        // Check if the waker fired and clear it
        let waker_token = self.waker_token.lock().unwrap();
        if let Some(wt) = *waker_token {
            let is_waker = events.iter().any(|e| e.token() == wt);
            if is_waker {
                drop(waker_token);
                // Trigger another EV_SET to clear the waker state
                let mut event: libc::kevent = unsafe { std::mem::zeroed() };
                unsafe {
                    libc::EV_SET(
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

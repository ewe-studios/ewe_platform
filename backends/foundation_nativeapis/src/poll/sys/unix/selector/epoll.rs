/// Linux epoll selector.

use crate::poll::event::Events;
use crate::poll::{Interest, Registry, Token};

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

impl Selector {
    /// Create a new epoll selector.
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

    /// Create a new selector and return both the Arc<Selector> and its Registry.
    pub fn new_with_registry() -> io::Result<(Arc<Self>, Registry)> {
        let epoll_fd = unsafe {
            let fd = libc::epoll_create1(libc::EPOLL_CLOEXEC);
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            OwnedFd::from_raw_fd(fd)
        };

        let selector = Arc::new(Self {
            epoll: epoll_fd,
            waker_fd: Mutex::new(None),
            waker_token: Mutex::new(None),
        });

        let registry = Registry {
            selector: selector.clone(),
        };

        Ok((selector, registry))
    }

    /// Register a raw file descriptor with the epoll selector.
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
                &mut event,
            )
        };

        if r < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Re-register a raw file descriptor with new token and/or interest.
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
                &mut event,
            )
        };

        if r < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Deregister a raw file descriptor from the epoll selector.
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
    pub fn wake(&self, _token: Token) -> io::Result<()> {
        let waker_fd = self.waker_fd.lock().unwrap();
        if let Some(ref fd) = *waker_fd {
            let value: u64 = 1;
            let r = unsafe {
                libc::write(
                    fd.as_raw_fd(),
                    &value as *const _ as *const libc::c_void,
                    std::mem::size_of_val(&value),
                )
            };
            if r < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "waker not registered",
            ))
        }
    }

    /// Clear the waker by reading the eventfd.
    pub fn clear_waker(&self) {
        let waker_fd = self.waker_fd.lock().unwrap();
        if let Some(ref fd) = *waker_fd {
            let mut value: u64 = 0;
            unsafe {
                libc::read(
                    fd.as_raw_fd(),
                    &mut value as *mut _ as *mut libc::c_void,
                    std::mem::size_of_val(&value),
                );
            }
        }
    }

    /// EVFILT_VNODE is not supported on epoll. Returns Unsupported.
    pub fn register_vnode(&self, _fd: RawFd, _token: Token) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "EVFILT_VNODE not supported on epoll"))
    }

    /// EVFILT_VNODE deregistration — no-op on epoll.
    pub fn deregister_vnode(&self, _fd: RawFd) -> io::Result<()> {
        Ok(())
    }

    /// Wait for readiness events.
    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        let timeout_ms = timeout
            .map(|d| d.as_millis() as libc::c_int)
            .unwrap_or(-1);

        // Get pointer to the Vec's buffer and its capacity (not length).
        // The Vec has capacity but length 0 — epoll_wait writes directly into
        // the buffer, and we update the length afterward.
        let (ptr, cap) = events.as_mut_ptr_and_cap();

        let n = unsafe {
            libc::epoll_wait(
                self.epoll.as_raw_fd(),
                ptr as *mut libc::epoll_event,
                cap as libc::c_int,
                timeout_ms,
            )
        };

        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                // EINTR — no events, not an error
                events.clear();
                return Ok(());
            }
            return Err(err);
        }

        // set_len is safe here — n is guaranteed ≤ capacity by epoll_wait
        events.set_len(n as usize);

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

/// Convert `Interest` to epoll event flags.
fn epoll_events_from_interest(interest: Interest) -> u32 {
    let mut events: u32 = 0;

    // Use edge-triggered mode — we track readiness ourselves
    events |= libc::EPOLLET as u32;

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

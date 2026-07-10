//! Linux io_uring readiness selector (F41).
//!
//! Implements the same internal `Selector` interface as `epoll.rs`, using
//! `IORING_OP_POLL_ADD` (multishot poll) for fd readiness. The `IoUring`
//! instance is held behind a `Mutex` for thread safety.
//!
//! Kernel requirement: Linux ≥ 5.13 (multishot poll).

use std::collections::HashMap;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use io_uring::{opcode, types, IoUring};

use super::super::super::super::event::Events;
use crate::native::poll::{Interest, Registry, Token};

/// The raw fd type on Linux.
pub type RawFd = std::os::unix::io::RawFd;

/// Per-fd tracking entry.
struct FdEntry {
    fd: RawFd,
    interest: Interest,
}

/// io_uring-based selector. Ring access serialised through a `Mutex`.
pub struct Selector {
    ring: Mutex<IoUring>,
    /// Token → fd tracking.
    entries: Mutex<HashMap<Token, FdEntry>>,
    waker_fd: Mutex<Option<OwnedFd>>,
    waker_token: Mutex<Option<Token>>,
}

impl Selector {
    const DEFAULT_ENTRIES: u32 = 256;

    pub fn new() -> io::Result<Self> {
        let ring = IoUring::new(Self::DEFAULT_ENTRIES)?;
        Ok(Self {
            ring: Mutex::new(ring),
            entries: Mutex::new(HashMap::new()),
            waker_fd: Mutex::new(None),
            waker_token: Mutex::new(None),
        })
    }

    pub fn new_with_registry() -> io::Result<(Arc<Self>, Registry)> {
        let selector = Arc::new(Self::new()?);
        let registry = Registry { selector: selector.clone() };
        Ok((selector, registry))
    }

    /// Submit a multishot `POLL_ADD` for the given fd.
    fn arm_poll(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let mut poll_mask: i16 = 0;
        if interest.is_readable() { poll_mask |= libc::POLLIN as i16; }
        if interest.is_writable() { poll_mask |= libc::POLLOUT as i16; }

        let sqe = opcode::PollAdd::new(
            types::Fd(fd),
            poll_mask as u32,
        )
        .multi(true)
        .build()
        .user_data(token.0 as u64);

        let mut ring = self.ring.lock().unwrap();
        // SAFETY: SQE is fully initialized.
        unsafe { ring.submission().push(&sqe) }
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "SQE queue full"))?;
        ring.submit()?;
        Ok(())
    }

    /// Submit a `POLL_REMOVE` to cancel polling for a token.
    fn disarm_poll(&self, token: Token) {
        let mut ring = self.ring.lock().unwrap();
        let sqe = opcode::PollRemove::new(token.0 as u64)
            .build()
            .user_data(u64::MAX); // tracking SQE
        unsafe { ring.submission().push(&sqe) }.ok();
        ring.submit().ok();
    }

    pub fn register_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let mut entries = self.entries.lock().unwrap();
        self.arm_poll(fd, token, interest)?;
        entries.insert(token, FdEntry { fd, interest });
        Ok(())
    }

    pub fn reregister_fd(&self, fd: RawFd, token: Token, interest: Interest) -> io::Result<()> {
        let mut entries = self.entries.lock().unwrap();
        // Cancel existing poll, re-arm.
        self.disarm_poll(token);
        entries.remove(&token);
        self.arm_poll(fd, token, interest)?;
        entries.insert(token, FdEntry { fd, interest });
        Ok(())
    }

    pub fn deregister_fd(&self, _fd: RawFd) -> io::Result<()> {
        let mut entries = self.entries.lock().unwrap();
        let token = entries.iter()
            .find(|(_, e)| e.fd == _fd)
            .map(|(t, _)| *t);
        if let Some(token) = token {
            self.disarm_poll(token);
            entries.remove(&token);
        }
        Ok(())
    }

    pub fn register_waker(&self, token: Token) -> io::Result<()> {
        let efd = unsafe {
            let fd = libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK);
            if fd < 0 { return Err(io::Error::last_os_error()); }
            OwnedFd::from_raw_fd(fd)
        };
        self.register_fd(efd.as_raw_fd(), token, Interest::READABLE)?;
        *self.waker_fd.lock().unwrap() = Some(efd);
        *self.waker_token.lock().unwrap() = Some(token);
        Ok(())
    }

    pub fn wake(&self, _token: Token) -> io::Result<()> {
        let guard = self.waker_fd.lock().unwrap();
        if let Some(ref fd) = *guard {
            let val: u64 = 1;
            let r = unsafe {
                libc::write(fd.as_raw_fd(), &val as *const _ as *const libc::c_void,
                            std::mem::size_of_val(&val))
            };
            if r < 0 { return Err(io::Error::last_os_error()); }
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "waker not registered"))
        }
    }

    pub fn clear_waker(&self) {
        let guard = self.waker_fd.lock().unwrap();
        if let Some(ref fd) = *guard {
            let mut val: u64 = 0;
            unsafe { libc::read(fd.as_raw_fd(), &mut val as *mut _ as *mut libc::c_void,
                                std::mem::size_of_val(&val)); }
        }
    }

    pub fn register_vnode(&self, _fd: RawFd, _token: Token) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "EVFILT_VNODE not supported on io_uring"))
    }
    pub fn deregister_vnode(&self, _fd: RawFd) -> io::Result<()> { Ok(()) }

    // ── Poll ────────────────────────────────────────────────────────────────

    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> io::Result<()> {
        // Submit pending SQEs + wait for CQEs in one lock scope.
        let mut ring = self.ring.lock().unwrap();
        ring.submit()?;

        if !timeout.map_or(false, |d| d.is_zero()) {
            match ring.submitter().submit_and_wait(1) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => {}
            }
        }

        // Drain CQEs (cq borrows ring — scope it).
        let mut seen: HashMap<Token, u32> = HashMap::new();
        let mut needs_rearm: Vec<Token> = Vec::new();
        {
            let mut cq = unsafe { ring.completion() };
            while let Some(cqe) = cq.next() {
                let user_data = cqe.user_data();
                if user_data == u64::MAX { continue; }
                let token = Token(user_data as usize);
                let result = cqe.result();
                let has_more = io_uring::cqueue::more(cqe.flags());
                if result < 0 { continue; }
                let mask = result as u32;
                *seen.entry(token).or_insert(0) |= mask;
                if !has_more { needs_rearm.push(token); }
            }
        }
        drop(ring);

        // Re-arm polls that lost multishot.
        let entries = self.entries.lock().unwrap();
        for token in &needs_rearm {
            if let Some(e) = entries.get(token) {
                self.arm_poll(e.fd, *token, e.interest).ok();
            }
        }
        drop(entries);

        // Write into Events buffer.
        let (ptr, cap) = events.as_mut_ptr_and_cap();
        let mut n = 0;
        for (token, mask) in &seen {
            let mut flags: u32 = 0;
            if mask & (libc::POLLIN as u32) != 0 { flags |= libc::EPOLLIN as u32; }
            if mask & (libc::POLLOUT as u32) != 0 { flags |= libc::EPOLLOUT as u32; }
            if mask & (libc::POLLHUP as u32) != 0 {
                flags |= libc::EPOLLRDHUP as u32 | libc::EPOLLHUP as u32;
            }
            if mask & (libc::POLLERR as u32) != 0 { flags |= libc::EPOLLERR as u32; }
            if flags != 0 && n < cap {
                unsafe {
                    let raw = libc::epoll_event { events: flags, u64: token.0 as u64 };
                    *ptr.add(n) = std::mem::transmute::<libc::epoll_event, super::super::super::super::event::Event>(raw);
                }
                n += 1;
            }
        }
        events.set_len(n);

        // Clear waker.
        if let Some(wt) = *self.waker_token.lock().unwrap() {
            if seen.contains_key(&wt) { self.clear_waker(); }
        }
        Ok(())
    }
}

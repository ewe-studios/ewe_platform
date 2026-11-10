/// Linux IO multiplexing — epoll-based event loop.
use std::{ffi, mem, time::Duration};

use super::Fd;

/// An epoll instance with a waker eventfd.
pub struct IoMultiplexing {
    fd: Fd,
    pub(crate) waker_fd: Fd,
}

impl IoMultiplexing {
    /// Create the epoll instance and its waker.
    ///
    /// # Panics
    ///
    /// Panics when the operating system cannot create the epoll instance or the eventfd.
    #[must_use]
    pub fn new() -> Self {
        let fd = unsafe {
            let fd = libc::epoll_create1(0);
            assert_ne!(fd, -1);
            Fd::from_raw(fd)
        };

        let waker_fd = unsafe {
            let efd = libc::eventfd(0, libc::EFD_CLOEXEC);
            assert_ne!(efd, -1);
            Fd::from_raw(efd)
        };

        let im = Self { fd, waker_fd };

        im.register(&im.waker_fd);

        im
    }

    pub(crate) fn register(&self, fd: &Fd) {
        unsafe {
            let mut ev = libc::epoll_event {
                events: libc::EPOLLIN as _,
                u64: fd.event_token(),
            };
            let r = libc::epoll_ctl(
                self.fd.as_raw(),
                libc::EPOLL_CTL_ADD,
                fd.as_raw(),
                &raw mut ev,
            );
            assert_ne!(r, -1);
        }
    }

    pub(crate) fn wait(&self, events: &mut Vec<libc::epoll_event>, timeout: Option<Duration>) {
        unsafe {
            let n = libc::epoll_wait(
                self.fd.as_raw(),
                events.as_mut_ptr(),
                i32::try_from(events.capacity()).unwrap_or(i32::MAX),
                timeout.map_or(-1, |tot| {
                    ffi::c_int::try_from(tot.as_millis()).unwrap_or(ffi::c_int::MAX)
                }),
            );
            if n < 1 {
                events.clear();
            } else {
                events.set_len(usize::try_from(n).unwrap_or(0));
            }
        }
    }

    /// Wake a thread that waits in `wait`.
    ///
    /// # Panics
    ///
    /// Panics when the operating system cannot write to the waker.
    pub fn wake(&self) {
        unsafe {
            let u: u64 = 1;
            let r = libc::write(
                self.waker_fd.as_raw(),
                (&raw const u).cast::<ffi::c_void>(),
                mem::size_of_val(&u),
            );
            assert_ne!(r, -1);
        }
    }

    pub(crate) fn clear_waker(&self) {
        unsafe {
            let mut u: u64 = 0;
            libc::read(
                self.waker_fd.as_raw(),
                (&raw mut u).cast::<ffi::c_void>(),
                mem::size_of_val(&u),
            );
        }
    }
}

impl Default for IoMultiplexing {
    fn default() -> Self {
        Self::new()
    }
}

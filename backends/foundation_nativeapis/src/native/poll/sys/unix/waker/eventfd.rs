/// Linux waker using eventfd.
///
/// Writing a u64 to the eventfd unblocks `epoll_wait()`.
/// The eventfd is registered with `EPOLLIN` and `EPOLLET` flags.

use std::os::fd::OwnedFd;
use std::os::unix::io::{AsRawFd, FromRawFd};

/// A waker file descriptor for Linux.
/// Wraps an eventfd that can be written to from any thread to wake epoll.
pub struct WakerFd {
    fd: OwnedFd,
}

impl WakerFd {
    /// Create a new eventfd waker.
    pub fn new() -> std::io::Result<Self> {
        let efd = unsafe {
            let fd = libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK);
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            OwnedFd::from_raw_fd(fd)
        };

        Ok(Self { fd: efd })
    }

    /// Wake the poller by writing 1 to the eventfd.
    pub fn wake(&self) -> std::io::Result<()> {
        let value: u64 = 1;
        let r = unsafe {
            libc::write(
                self.fd.as_raw_fd(),
                &value as *const _ as *const libc::c_void,
                std::mem::size_of_val(&value),
            )
        };
        if r < 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Clear the eventfd by reading from it.
    pub fn clear(&self) {
        let mut value: u64 = 0;
        unsafe {
            libc::read(
                self.fd.as_raw_fd(),
                &mut value as *mut _ as *mut libc::c_void,
                std::mem::size_of_val(&value),
            );
        }
    }

    /// Get the raw fd.
    pub fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.fd.as_raw_fd()
    }
}

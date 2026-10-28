/// Linux waker using eventfd.
///
/// Writing a u64 to the eventfd unblocks `epoll_wait()`.
/// The eventfd is registered with `EPOLLIN` and `EPOLLET` flags.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

/// A waker file descriptor for Linux.
/// Wraps an eventfd that can be written to from any thread to wake epoll.
#[allow(dead_code)]
pub struct WakerFd {
    fd: OwnedFd,
}

#[allow(dead_code)]
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

    /// Wake the epoll instance.
    pub fn wake(&self) -> std::io::Result<()> {
        let buf: u64 = 1;
        let ret = unsafe { libc::write(self.fd.as_raw_fd(), &buf as *const u64 as *const _, 8) };
        if ret < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    /// Clear the eventfd.
    pub fn clear(&self) {
        let mut buf: u64 = 0;
        unsafe {
            libc::read(self.fd.as_raw_fd(), &mut buf as *mut u64 as *mut _, 8);
        }
    }

    /// Get the raw file descriptor.
    pub fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

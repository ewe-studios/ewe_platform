/// Linux IPC fd types — Fd, Remote, Local.

use std::{
    io, mem,
    os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
    sync::{Mutex, MutexGuard},
};

/// Owned file descriptor wrapper.
pub struct Fd(OwnedFd);

impl Fd {
    pub fn clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }

    pub unsafe fn from_raw(raw: RawFd) -> Self {
        Self(OwnedFd::from_raw_fd(raw))
    }

    pub fn into_raw(self) -> RawFd {
        self.0.into_raw_fd()
    }

    pub fn as_raw(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl PartialEq for Fd {
    fn eq(&self, other: &Self) -> bool {
        self.as_raw() == other.as_raw()
    }
}

impl std::fmt::Debug for Fd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Fd({})", self.as_raw())
    }
}

/// The remote (write) end of a socket connection. Thread-safe via Mutex.
#[derive(Debug)]
pub struct Remote {
    v: i32,
    fd: Mutex<Fd>,
}

impl Remote {
    pub fn new(fd: Fd) -> Self {
        Self {
            v: fd.as_raw(),
            fd: Mutex::new(fd),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, Fd> {
        self.fd.lock().unwrap()
    }

    /// Check if the remote socket is dead via `getsockopt(SO_ERROR)`.
    pub fn is_dead(&self) -> bool {
        let mut err: i32 = 0;
        let mut len: u32 = mem::size_of_val(&err) as _;
        let r = unsafe {
            libc::getsockopt(
                self.v,
                libc::SOL_SOCKET,
                libc::SO_ERROR,
                &mut err as *mut _ as *mut _,
                &mut len,
            )
        };
        r == -1 || err != 0
    }
}

impl PartialEq for Remote {
    fn eq(&self, other: &Self) -> bool {
        self.v == other.v
    }
}

/// The local (read) end of a socket connection.
pub struct Local(pub(crate) Fd);

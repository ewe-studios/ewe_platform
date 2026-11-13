/// Linux IPC fd types — Fd, Remote, Local.
use std::{
    io, mem,
    os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
    sync::{Mutex, MutexGuard},
};

/// Owned file descriptor wrapper.
pub struct Fd(OwnedFd);

impl Fd {
    /// Duplicate this file descriptor with `dup`.
    ///
    /// # Errors
    ///
    /// Returns the operating system error when the descriptor cannot be duplicated.
    pub fn duplicate(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }

    /// Take ownership of a raw file descriptor.
    ///
    /// # Safety
    ///
    /// `raw` must be an open file descriptor that no other owner closes.
    #[must_use]
    pub unsafe fn from_raw(raw: RawFd) -> Self {
        Self(OwnedFd::from_raw_fd(raw))
    }

    /// Give up ownership and return the raw file descriptor.
    #[must_use]
    pub fn into_raw(self) -> RawFd {
        self.0.into_raw_fd()
    }

    /// The raw file descriptor. This `Fd` keeps the ownership.
    #[must_use]
    pub fn as_raw(&self) -> RawFd {
        self.0.as_raw_fd()
    }

    /// The epoll event token of this descriptor: its raw value as an unsigned number.
    #[must_use]
    pub fn event_token(&self) -> u64 {
        u64::from(self.as_raw().cast_unsigned())
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
    /// Wrap a file descriptor as the remote end.
    #[must_use]
    pub fn new(fd: Fd) -> Self {
        Self {
            v: fd.as_raw(),
            fd: Mutex::new(fd),
        }
    }

    /// Lock the file descriptor for writing.
    ///
    /// # Panics
    ///
    /// Panics when the lock is poisoned.
    pub fn lock(&self) -> MutexGuard<'_, Fd> {
        self.fd.lock().unwrap()
    }

    /// Check if the remote socket is dead via `getsockopt(SO_ERROR)`.
    pub fn is_dead(&self) -> bool {
        let mut err: i32 = 0;
        let mut len = libc::socklen_t::try_from(mem::size_of_val(&err)).unwrap_or(4);
        let r = unsafe {
            libc::getsockopt(
                self.v,
                libc::SOL_SOCKET,
                libc::SO_ERROR,
                (&raw mut err).cast::<libc::c_void>(),
                &raw mut len,
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

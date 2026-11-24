//! The same-machine transport: a Unix domain socket for the registration
//! service.
//!
//! WHY: when the registrar sidecar runs on the proxy's machine, file
//! permissions do the job of mutual TLS (mTLS), so the operator needs no token
//! and no certificate (spec 82 feature 00, section 5.5).
//!
//! WHAT: [`UnixSocketAcceptor`] wraps a `std::os::unix::net::UnixListener` as a
//! [`Acceptor`](foundation_netio::native::connection::Acceptor), so
//! `foundation_http`'s `serve_with_acceptor` serves the `ConnectRPC` app on it.
//!
//! HOW: the socket file gets mode `0o660`. At start the code checks that the
//! file's mode is `0o660` and that its owner and group are the process's, and
//! refuses a mismatch with a typed error. An accepted connection is a
//! `Connection::Unix`; the placeholder address is `127.0.0.1:0` because a Unix
//! socket has no Internet Protocol (IP) address.

use std::io;
use std::net::SocketAddr;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use foundation_netio::native::connection::{Acceptor, Connection};

use super::RegistrationAuthorityError;

/// The file mode of the registration Unix socket: owner and group read and
/// write, nobody else.
pub const SOCKET_MODE: u32 = 0o660;

/// A Unix socket that serves the registration service.
#[derive(Debug)]
pub struct UnixSocketAcceptor {
    listener: UnixListener,
    path: PathBuf,
    placeholder: SocketAddr,
}

impl UnixSocketAcceptor {
    /// Bind the socket at `path` with mode `0o660`.
    ///
    /// A stale file at `path` is removed first, as the control socket does. The
    /// owner and group of the new file must be the process's own.
    ///
    /// # Errors
    /// [`RegistrationAuthorityError::Socket`] when the directory refuses the
    /// file, the bind fails, or the owner, group or mode is wrong.
    pub fn bind(path: impl Into<PathBuf>) -> Result<Self, RegistrationAuthorityError> {
        Self::bind_with_group(path, None)
    }

    /// Bind the socket and give it `group` (a GID). `None` keeps the process's
    /// effective GID. The process must belong to `group`, unless it is root.
    ///
    /// # Errors
    /// [`RegistrationAuthorityError::Socket`] when the bind, the mode, the
    /// group or the owner is wrong.
    pub fn bind_with_group(
        path: impl Into<PathBuf>,
        group: Option<u32>,
    ) -> Result<Self, RegistrationAuthorityError> {
        let path = path.into();
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).map_err(|e| {
            RegistrationAuthorityError::Socket(format!("bind {}: {e}", path.display()))
        })?;
        if let Err(error) = set_mode_and_check(&path, group) {
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        Ok(Self {
            listener,
            path,
            placeholder: SocketAddr::from(([127, 0, 0, 1], 0)),
        })
    }

    /// The socket file's path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Acceptor for UnixSocketAcceptor {
    fn accept_connection(&self) -> io::Result<(Connection, SocketAddr)> {
        let (stream, _addr) = self.listener.accept()?;
        Ok((Connection::Unix(stream), self.placeholder))
    }

    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.listener.set_nonblocking(nonblocking)
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.placeholder)
    }

    fn raw_fd(&self) -> Option<std::os::fd::RawFd> {
        Some(self.listener.as_raw_fd())
    }
}

impl Drop for UnixSocketAcceptor {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Set mode `0o660`, move the file to `group`, and refuse a wrong owner,
/// group or mode.
fn set_mode_and_check(path: &Path, group: Option<u32>) -> Result<(), RegistrationAuthorityError> {
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(SOCKET_MODE)).map_err(|e| {
        RegistrationAuthorityError::Socket(format!("chmod {}: {e}", path.display()))
    })?;
    // SAFETY: `geteuid` and `getegid` read process state and take no pointer.
    let (uid, gid) = unsafe { (libc::geteuid(), libc::getegid()) };
    let expected_gid = group.unwrap_or(gid);
    if expected_gid != gid {
        // The owner stays the process; only the group changes. A process can
        // move a file to a group that it belongs to, and root can move it to
        // any group.
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| {
            RegistrationAuthorityError::Socket(format!(
                "{} has a NUL byte in its path",
                path.display()
            ))
        })?;
        // SAFETY: `c_path` is a valid null-terminated string that lives for the
        // call; `chown` reads it and the two ids.
        let changed = unsafe { libc::chown(c_path.as_ptr(), uid, expected_gid) };
        if changed != 0 {
            return Err(RegistrationAuthorityError::Socket(format!(
                "chown {} to group {expected_gid}: {}",
                path.display(),
                std::io::Error::last_os_error()
            )));
        }
    }
    let metadata = std::fs::metadata(path)
        .map_err(|e| RegistrationAuthorityError::Socket(format!("stat {}: {e}", path.display())))?;
    let mode = metadata.mode() & 0o777;
    if mode != SOCKET_MODE {
        return Err(RegistrationAuthorityError::Socket(format!(
            "{} has mode {mode:#o}; expected {SOCKET_MODE:#o}",
            path.display()
        )));
    }
    if metadata.uid() != uid || metadata.gid() != expected_gid {
        return Err(RegistrationAuthorityError::Socket(format!(
            "{} is owned by {}/{}; the proxy runs as {uid}/{expected_gid}",
            path.display(),
            metadata.uid(),
            metadata.gid()
        )));
    }
    Ok(())
}

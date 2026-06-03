/// The `event::Source` trait for registering types with the poll selector.
///
/// Implement this trait for any type that produces readiness events
/// (e.g., sockets, pipes, inotify fds) so it can be registered with a
/// [`Registry`](super::super::Registry).

use crate::native::poll::{Interest, Registry, Token};

use std::io;

/// Types that can be registered with a [`Registry`].
///
/// The trait is implemented for [`SourceFd`](super::super::sys::SourceFd),
/// `TcpStream`, `TcpListener`, `UdpSocket`, and other I/O types.
/// You can also implement it for custom types that produce readiness events.
pub trait Source {
    /// Register `self` with the given [`Registry`] under the given [`Token`]
    /// for the given [`Interest`].
    ///
    /// # Errors
    /// Returns an error if the fd cannot be registered (already registered,
    /// invalid fd, etc.).
    fn register(&mut self, registry: &Registry, token: Token, interest: Interest)
        -> io::Result<()>;

    /// Re-register `self` with a new [`Token`] and/or [`Interest`].
    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<()>;

    /// Deregister `self` from the [`Registry`].
    fn deregister(&mut self, registry: &Registry) -> io::Result<()>;
}

/// `SourceFd` — a wrapper that lets you register any raw file descriptor
/// with the poll selector.
///
/// The fd must already be set to nonblocking mode.
/// The fd is NOT closed on drop — ownership is not transferred.
///
/// # Example
///
/// ```no_run
/// use std::io;
/// use std::os::unix::io::AsRawFd;
/// use foundation_reactor::{Poll, Token, Interest, SourceFd};
///
/// let poll = Poll::new().unwrap();
/// // A nonblocking descriptor you own (for example one end of a pipe).
/// let fd = io::stdin().as_raw_fd();
///
/// poll.registry().register(
///     &mut SourceFd(fd),
///     Token(0),
///     Interest::READABLE,
/// ).unwrap();
///
/// // The fd is NOT closed — you are still responsible for closing it.
/// ```

use super::super::super::event::Source;
use crate::poll::{Interest, Registry, Token};

use std::io;
use std::os::unix::io::RawFd;

/// A wrapper for a raw file descriptor that implements [`Source`].
pub struct SourceFd(pub RawFd);

impl Source for SourceFd {
    fn register(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        registry.register_fd(self.0, token, interest)
    }

    fn reregister(
        &mut self,
        registry: &Registry,
        token: Token,
        interest: Interest,
    ) -> io::Result<()> {
        registry.reregister_fd(self.0, token, interest)
    }

    fn deregister(&mut self, registry: &Registry) -> io::Result<()> {
        registry.deregister_fd(self.0)
    }
}

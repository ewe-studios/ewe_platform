/// Windows stubs — placeholder until IOCP selector is implemented.

use crate::poll::event::Event;
use crate::poll::Events;
use crate::poll::{Interest, Registry, Token};

use std::io;
use std::sync::Arc;
use std::time::Duration;

/// Raw handle type on Windows.
pub type RawFd = std::os::windows::io::RawHandle;

/// Selector stub — Windows IOCP not yet implemented.
pub struct Selector;

impl Selector {
    pub fn new_with_registry() -> io::Result<(Arc<Self>, Registry)> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows IOCP selector not yet implemented",
        ))
    }

    pub fn register_fd(&self, _fd: RawFd, _token: Token, _interest: Interest) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not implemented"))
    }

    pub fn reregister_fd(&self, _fd: RawFd, _token: Token, _interest: Interest) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not implemented"))
    }

    pub fn deregister_fd(&self, _fd: RawFd) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not implemented"))
    }

    pub fn register_waker(&self, _token: Token) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not implemented"))
    }

    pub fn wake(&self, _token: Token) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not implemented"))
    }

    pub fn poll(&self, _events: &mut Events, _timeout: Option<Duration>) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not implemented"))
    }
}

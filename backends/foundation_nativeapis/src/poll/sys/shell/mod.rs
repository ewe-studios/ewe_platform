/// Shell stubs for unsupported platforms.
/// Used when no os-poll feature is enabled.

use crate::poll::event::Event;
use crate::poll::Events;
use crate::poll::{Interest, Registry, Token};

use std::io;
use std::sync::Arc;
use std::time::Duration;

/// Raw fd type for unsupported platforms.
pub type RawFd = i32;

/// Selector stub — always returns an error.
pub struct Selector;

impl Selector {
    /// Returns an unsupported error.
    pub fn new_with_registry() -> io::Result<(Arc<Self>, Registry)> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "poll is not supported on this platform",
        ))
    }

    pub fn register_fd(&self, _fd: RawFd, _token: Token, _interest: Interest) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "unsupported"))
    }

    pub fn reregister_fd(&self, _fd: RawFd, _token: Token, _interest: Interest) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "unsupported"))
    }

    pub fn deregister_fd(&self, _fd: RawFd) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "unsupported"))
    }

    pub fn register_vnode(&self, _fd: RawFd, _token: Token) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "unsupported"))
    }

    pub fn deregister_vnode(&self, _fd: RawFd) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "unsupported"))
    }

    pub fn register_waker(&self, _token: Token) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "unsupported"))
    }

    pub fn wake(&self, _token: Token) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "unsupported"))
    }

    pub fn poll(&self, _events: &mut Events, _timeout: Option<Duration>) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "unsupported"))
    }
}

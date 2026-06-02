/// Unix platform selector and utilities.

#[cfg(target_os = "linux")]
pub mod selector {
    pub mod epoll;
    pub use epoll::{Selector, RawFd};
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
))]
pub mod selector {
    pub mod kqueue;
    pub use kqueue::{Selector, RawFd};
}

#[cfg(target_os = "linux")]
pub mod waker {
    /// Linux waker using eventfd.
    mod eventfd;
    pub use eventfd::WakerFd;
}

/// SourceFd — register any raw file descriptor with the poll selector.
pub mod sourcefd;
pub use sourcefd::SourceFd;

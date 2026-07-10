/// Platform-specific selector implementations.
///
/// On Linux: epoll
/// On macOS/BSD: kqueue
/// On Windows: IOCP

#[cfg(all(target_os = "linux", not(feature = "uring")))]
pub use self::unix::selector::epoll::{Selector, RawFd};
#[cfg(all(target_os = "linux", feature = "uring"))]
pub use self::unix::selector::uring::{Selector, RawFd};

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
))]
pub use self::unix::selector::kqueue::{Selector, RawFd};

#[cfg(target_os = "windows")]
pub use self::windows::{Selector, RawFd};

#[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly",
    target_os = "windows",
)))]
pub use self::shell::{Selector, RawFd};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use self::unix::SourceFd;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(any(unix, target_os = "windows")))]
mod shell;

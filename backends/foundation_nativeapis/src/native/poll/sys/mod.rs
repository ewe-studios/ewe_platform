/// Platform-specific selector implementations.
///
/// On Linux: epoll
/// On macOS/BSD: kqueue
/// On Windows: IOCP

/// On Linux the concrete backend is chosen at runtime by the F42 probe ladder,
/// so `Selector` is the dispatch enum rather than one of the leaf selectors.
#[cfg(target_os = "linux")]
pub use self::unix::selector::dispatch::{RawFd, Selector};

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

/// Public so the F41 parity suite can construct `epoll::Selector` and
/// `uring::Selector` side by side in one test process.
#[cfg(unix)]
pub mod unix;
#[cfg(unix)]
pub use self::unix::SourceFd;

#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(any(unix, target_os = "windows")))]
mod shell;

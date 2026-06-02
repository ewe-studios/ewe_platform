use std::path::Path;
use std::time::Duration;

use crate::error::Result;
use crate::event::WatchEvent;

/// A pluggable native file watcher for a specific platform.
///
/// Implementors use the native OS mechanism:
/// - Linux: inotify (via epoll or io_uring)
/// - macOS/BSD: kqueue with EVFILT_VNODE
/// - Windows: IOCP with ReadDirectoryChangesW
/// - Fallback: polling via metadata stat
pub trait NativeWatcher: Send + Sync {
    /// Add a path to watch (file or directory).
    ///
    /// For directories, when `recursive` is true the implementation will
    /// recursively watch all subdirectories. On platforms that don't
    /// support recursive watching natively (e.g., Linux inotify), this
    /// walks the directory tree and registers each subdirectory.
    fn watch(&mut self, path: &Path, recursive: bool) -> Result<()>;

    /// Remove a previously watched path.
    fn unwatch(&mut self, path: &Path) -> Result<()>;

    /// Poll for events with an optional timeout.
    ///
    /// Returns immediately if events are available, or blocks up to `timeout`.
    /// Returns an empty Vec on timeout.
    fn poll(&mut self, timeout: Duration) -> Result<Vec<WatchEvent>>;

    /// Remove all watches and release resources.
    fn clear(&mut self) -> Result<()>;
}

/// PollWatcher — stdlib-only metadata polling fallback.
#[cfg(feature = "watcher")]
pub mod poll_watcher;

/// InotifyWatcher — Linux inotify-based file watching.
#[cfg(all(target_os = "linux", feature = "watcher-linux"))]
pub mod linux;

/// KqueueWatcher — macOS/BSD kqueue-based file watching (EVFILT_VNODE).
#[cfg(all(
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
    ),
    feature = "watcher-macos"
))]
pub mod unix;

use std::path::Path;
use std::time::Duration;

use super::error::Result;
use super::event::WatchEvent;

/// A pluggable native file watcher for a specific platform.
///
/// Implementors use the native OS mechanism:
/// - Linux: inotify (via epoll or io_uring) — `crate::native::watcher::InotifyWatcher`
/// - macOS/BSD: kqueue with EVFILT_VNODE — `crate::native::watcher::KqueueWatcher`
/// - Windows: IOCP with ReadDirectoryChangesW — `crate::native::watcher::WinWatcher`
/// - Fallback: polling via metadata stat — `PollWatcher`
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

    /// Check if events are available without consuming them.
    ///
    /// If `timeout` is `Some`, blocks up to that duration waiting for the
    /// first event. If `None` (or `Some(Duration::ZERO)`), returns immediately.
    ///
    /// This is a **peek** — calling this method does not drain events.
    /// Implementations cache detected events so that a subsequent `poll()`
    /// call will return them without re-scanning.
    ///
    /// On level-triggered platforms (Linux epoll, macOS/BSD kqueue), this
    /// checks OS readiness. On dequeuing platforms (Windows IOCP), events
    /// are cached internally so they can be peeked without being lost.
    ///
    /// The default implementation returns `false` (conservative fallback
    /// for platforms that can't peek).
    fn has_events(&mut self, timeout: Option<Duration>) -> bool {
        let _ = timeout;
        false
    }
}

/// PollWatcher — stdlib-only metadata polling fallback.
pub mod poll_watcher;

/// SharedNativeWatcher — thread-safe Arc<RwLock<T>> wrapper + type-erased SharedWatcher.
pub mod shared;

pub use shared::{SharedNativeWatcher, SharedWatcher};
pub use poll_watcher::PollWatcher;

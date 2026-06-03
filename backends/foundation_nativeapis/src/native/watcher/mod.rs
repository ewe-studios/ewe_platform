/// Platform-specific native file watcher implementations.
///
/// These modules use OS-specific APIs and are feature-gated:
/// - Linux: `inotify` (via epoll)
/// - macOS/BSD: `kqueue` (EVFILT_VNODE)
/// - Windows: `ReadDirectoryChangesW` (via IOCP)

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

/// WinWatcher — Windows ReadDirectoryChangesW-based file watching.
#[cfg(all(target_os = "windows", feature = "watcher-windows"))]
pub mod windows;

/// Platform-specific native module — feature-gated, requires OS-specific APIs.
///
/// Contains:
/// - `poll` — I/O readiness polling (epoll/kqueue/IOCP)
/// - `fd` — File descriptor readiness tracking (Unix only)
/// - `net` — TCP/UDP/Unix socket wrappers (Unix only)
/// - `watcher` — Platform-specific file watchers (inotify, kqueue, IOCP)

#[cfg(feature = "poll")]
pub mod poll;

#[cfg(feature = "poll")]
pub mod net;

#[cfg(feature = "fd")]
pub mod fd;

/// Platform-specific native watchers.
pub mod watcher;

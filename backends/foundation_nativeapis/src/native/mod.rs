//! Platform-specific native module — feature-gated, requires OS-specific APIs.
//!
//! Contains:
//! - `poll` — I/O readiness polling (epoll/kqueue/IOCP), re-exported from
//!   `foundation_reactor` so `foundation_netio` can reach it without a cycle
//! - `fd` — File descriptor readiness tracking (Unix only)
//! - `net` — TCP/UDP/Unix socket wrappers (Unix only)
//! - `watcher` — Platform-specific file watchers (inotify, kqueue, IOCP)

#[cfg(feature = "poll")]
pub use foundation_reactor::poll;

#[cfg(all(feature = "poll", unix))]
pub mod net;

#[cfg(all(feature = "fd", unix))]
pub mod fd;

/// Platform-specific native watchers.
pub mod watcher;

/// Native filesystem and search implementations. `NativeFs` and the delta
/// backends need `vfs-native`; the host-process search backends need
/// `vfs-search`.
#[cfg(any(feature = "vfs-native", feature = "vfs-search"))]
pub mod vfs;

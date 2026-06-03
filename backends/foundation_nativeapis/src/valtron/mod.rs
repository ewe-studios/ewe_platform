/// Valtron executor integration — shareable, platform-agnostic task adapters.
///
/// These types work with any `EventReadiness` impl and the valtron executor model.
/// They don't depend on OS-specific APIs.
///
/// The `native` sub-module (behind `#[cfg(feature = "fd")]`) contains tasks
/// that require native OS APIs (e.g., `FdMonitorTask` uses `RegisteredFd`).

mod broadcaster;
mod file_watcher;
pub use file_watcher::{FileWatcherBuilder, WatchEventStream};
mod stop_signal;

pub use broadcaster::Broadcaster;
pub use file_watcher::FileWatcherTask;
pub use stop_signal::{CompositeReadiness, StopSignal};

// Re-export FdState from shared so native/fd_monitor.rs can access it.
pub use super::shared::FdState;

#[cfg(feature = "fd")]
pub mod native;
#[cfg(feature = "fd")]
pub use native::FdMonitorTask;

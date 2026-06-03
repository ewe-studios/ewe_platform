/// Valtron executor integration — shareable, platform-agnostic task adapters.
///
/// Requires the `task` feature flag. These types don't depend on OS-specific APIs
/// — they work with any `EventReadiness` impl and the valtron executor model.
///
/// The `native` sub-module (behind `#[cfg(feature = "fd")]`) contains tasks
/// that require native OS APIs (e.g., `FdMonitorTask` uses `RegisteredFd`).

mod broadcaster;
mod file_watcher;
mod stop_signal;

pub use broadcaster::EventBroadcaster;
pub use file_watcher::FileWatcherTask;
pub use stop_signal::{CompositeReadiness, StopSignal};

#[cfg(feature = "fd")]
pub mod native;
#[cfg(feature = "fd")]
pub use native::FdMonitorTask;

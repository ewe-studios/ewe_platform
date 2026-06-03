/// Valtron executor integration — shareable, platform-agnostic task adapters.
///
/// Requires the `task` feature flag. These types don't depend on OS-specific APIs
/// — they work with any `EventReadiness` impl and the valtron executor model.

mod broadcaster;
mod fd_monitor;
mod file_watcher;
mod stop_signal;

pub use broadcaster::EventBroadcaster;
pub use fd_monitor::FdMonitorTask;
pub use file_watcher::FileWatcherTask;
pub use stop_signal::{CompositeReadiness, StopSignal};

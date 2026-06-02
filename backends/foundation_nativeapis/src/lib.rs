pub mod api;
pub mod error;
pub mod event;

#[cfg(feature = "poll")]
pub mod poll;

#[cfg(feature = "poll")]
pub mod net;

#[cfg(feature = "fd")]
pub mod fd;

#[cfg(feature = "watcher")]
pub mod watcher;

#[cfg(feature = "task")]
pub mod task;

// Re-export shared watcher types
pub use watcher::{SharedNativeWatcher, SharedWatcher};

// Re-export common types at the crate root
pub use api::{native_watcher, NativeAPI, WatcherBuilder};
pub use error::{Result, WatchError};
pub use event::{WatchEvent, WatchEventKind};

// Re-export poll types when the poll feature is enabled
#[cfg(feature = "poll")]
pub use poll::{Events, Interest, Poll, Registry, SourceFd, Token, Waker};

// Re-export task types when the task feature is enabled
#[cfg(feature = "task")]
pub use task::{EventBroadcaster, FdMonitorTask, FileWatcherTask};

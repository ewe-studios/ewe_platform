pub mod api;
pub mod error;
pub mod event;

#[cfg(feature = "poll")]
pub mod poll;

#[cfg(feature = "fd")]
pub mod fd;

#[cfg(feature = "watcher")]
pub mod watcher;

#[cfg(feature = "task")]
pub mod task;

#[cfg(all(feature = "task", feature = "fd"))]
mod task_fd;

// Re-export common types at the crate root
pub use api::{native_watcher, NativeAPI, WatcherBuilder};
pub use error::{Result, WatchError};
pub use event::{WatchEvent, WatchEventKind};

// Re-export poll types when the poll feature is enabled
#[cfg(feature = "poll")]
pub use poll::{Events, Interest, Poll, Registry, SourceFd, Token, Waker};

// Re-export task types when the task feature is enabled
#[cfg(feature = "task")]
pub use task::{EventBroadcaster, FileWatcherTask};

// Re-export FdMonitorTask when both task and fd features are enabled
#[cfg(all(feature = "task", feature = "fd"))]
pub use task_fd::fd_monitor::FdMonitorTask;

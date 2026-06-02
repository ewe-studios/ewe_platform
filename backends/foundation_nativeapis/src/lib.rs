pub mod api;
pub mod error;
pub mod event;

#[cfg(feature = "poll")]
pub mod poll;

#[cfg(feature = "watcher")]
pub mod watcher;

// Re-export common types at the crate root
pub use api::{native_watcher, NativeAPI, WatcherBuilder};
pub use error::{Result, WatchError};
pub use event::{WatchEvent, WatchEventKind};

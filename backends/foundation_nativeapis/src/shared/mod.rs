/// Cross-platform shared module — always compiled, no platform-specific dependencies.
///
/// Contains:
/// - `error` — `WatchError` and `Result<T>`
/// - `event` — `WatchEvent` and `WatchEventKind`
/// - `watcher` — `NativeWatcher` trait, `PollWatcher` (stdlib-only), `SharedNativeWatcher<T>`, `SharedWatcher`
/// - `api` — `WatcherBuilder`, `NativeAPI`, `native_watcher()`

pub mod error;
pub mod event;
pub mod watcher;
pub mod api;
pub mod fd_state;

/// Virtual filesystem abstraction — traits, types, error, in-memory impls.
#[cfg(feature = "vfs")]
pub mod vfs;

/// Shareable IPC types — message traits, errors, labels, selectors, versioning.
#[cfg(feature = "ipc")]
pub mod ipc;

// Re-export common types at module level
pub use api::{native_watcher, NativeAPI, WatcherBuilder};
pub use error::{Result, WatchError};
pub use event::{WatchEvent, WatchEventKind};
pub use fd_state::FdState;
pub use watcher::{NativeWatcher, PollWatcher, SharedNativeWatcher, SharedWatcher};

/// Cross-platform shared module — always compiled, no platform-specific dependencies.
pub mod shared;

/// Platform-specific native module — feature-gated, requires OS-specific APIs.
pub mod native;

/// Valtron executor integration — shareable, platform-agnostic task adapters.
pub mod valtron;

/// Interprocess message bus (IPC) — adapted from ipmb.
/// Feature-gated: requires `ipc` feature. Only available on Linux, macOS, and Windows.
#[cfg(all(feature = "ipc", any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub mod ipc;

// ---------------------------------------------------------------------------
// Crate-root re-exports (backward-compatible API)
// ---------------------------------------------------------------------------

// Shared types are always available at the crate root.
pub use shared::{
    native_watcher, NativeAPI, NativeWatcher, PollWatcher, SharedNativeWatcher, SharedWatcher,
    WatchError, WatchEvent, WatchEventKind, WatcherBuilder, Result,
};

// Poll types when the poll feature is enabled.
#[cfg(feature = "poll")]
pub use native::poll::{Events, Interest, Poll, Registry, SourceFd, Token, Waker};

// Valtron types — always available (shareable, no native deps).
pub use valtron::{Broadcaster as EventBroadcaster, FileWatcherTask, FileWatcherBuilder, StopSignal, CompositeReadiness};

// VFS task types — requires vfs feature.
#[cfg(feature = "vfs")]
pub use valtron::{VfsTask, VfsTaskBuilder, VfsEventReadiness};

// FdState from shared (always available).
pub use shared::FdState;

// FdMonitorTask requires native::fd.
#[cfg(feature = "fd")]
pub use valtron::FdMonitorTask;

// VFS types when the vfs feature is enabled.
#[cfg(feature = "vfs")]
pub use shared::vfs::{
    ObservableFs, ObservableFile, ObservableSeekableFile, VfsEvent,
    VfsError, VfsResult, VfsFileSystem, MemoryFs,
};

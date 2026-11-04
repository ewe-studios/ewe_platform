// The `turso` local engine (vfs-turso) has deeply nested async fn bodies. The
// async_trait `dyn Future + Send` coercions in `turso_delta` exceed rustc's
// default recursion limit (128) while proving `Send`, so raise it for the crate.
#![recursion_limit = "256"]

/// Cross-platform shared module — always compiled, no platform-specific dependencies.
pub mod shared;

/// Platform-specific native module — feature-gated, requires OS-specific APIs.
pub mod native;

/// Valtron executor integration — shareable, platform-agnostic task adapters.
pub mod valtron;

/// WireGuard overlay data plane (spec-55, feature 00).
///
/// The sans-I/O `DataPlane` trait plus its two implementations — a smoltcp userspace
/// TCP/IP netstack (`netstack` feature, cross-target) and a kernel TUN device
/// (`tun` feature, native-only) — that convert overlay sockets to/from raw IP packets
/// on `foundation_boringtun::Tunn`'s inner boundary.
#[cfg(feature = "netstack")]
pub mod dataplane;

/// Interprocess message bus (IPC) — adapted from ipmb.
/// Feature-gated: requires `ipc` feature. Only available on Linux, macOS, and Windows.
#[cfg(all(
    feature = "ipc",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
pub mod ipc;

/// Cross-platform signal handling (SIGINT, SIGTERM, SIGHUP, SIGQUIT).
/// Feature-gated: requires `signal` feature (enabled by default).
#[cfg(feature = "signal")]
pub mod signal;

// ---------------------------------------------------------------------------
// Crate-root re-exports (backward-compatible API)
// ---------------------------------------------------------------------------

// Shared types are always available at the crate root.
pub use shared::{
    native_watcher, NativeAPI, NativeWatcher, PollWatcher, Result, SharedNativeWatcher,
    SharedWatcher, WatchError, WatchEvent, WatchEventKind, WatcherBuilder,
};

// Poll types when the poll feature is enabled.
#[cfg(all(feature = "poll", unix))]
pub use native::poll::SourceFd;
#[cfg(feature = "poll")]
pub use native::poll::{Events, Interest, Poll, Registry, Token, Waker};

// Valtron types — always available (shareable, no native deps).
pub use valtron::{
    Broadcaster as EventBroadcaster, CompositeReadiness, FileWatcherBuilder, FileWatcherTask,
    StopSignal,
};

// VFS task types — requires vfs feature.
#[cfg(feature = "vfs")]
pub use valtron::{VfsEventReadiness, VfsTask, VfsTaskBuilder};

// FdState from shared (always available).
pub use shared::FdState;

// FdMonitorTask requires native::fd (unix-only).
#[cfg(all(feature = "fd", unix))]
pub use valtron::FdMonitorTask;

// VFS types when the vfs feature is enabled.
#[cfg(feature = "vfs")]
pub use shared::vfs::{
    AsyncVfsFileSystem, MemoryFs, ObservableFile, ObservableFs, ObservableSeekableFile, VfsError,
    VfsEvent, VfsFileSystem, VfsResult,
};

// The sync bridge exists only where valtron can block (not JS-hosted wasm).
#[cfg(feature = "vfs")]
foundation_core::blocking_boundary! {
    pub use shared::vfs::SyncFs;
}

// VFS search types when the vfs-search feature is enabled.
#[cfg(feature = "vfs-search")]
pub use shared::vfs::{
    vfs_searcher, CascadingVfsSearcher, InCodeVfsSearcher, VfsSearchKind, VfsSearchMatch,
    VfsSearcher,
};

#[cfg(all(feature = "vfs-search", not(target_family = "wasm")))]
pub use shared::vfs::{native_vfs_searcher, CliSearcher, SearchConfig};

#[cfg(feature = "vfs-fjall")]
pub use shared::vfs::{DurabilityWriteConfig, FjallDocumentStore};

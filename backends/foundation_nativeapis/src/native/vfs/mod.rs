//! Native-side VFS implementations. The filesystem passthrough, the delta
//! backends and the host-process search backends live here. This module is
//! declared only for the features that use it: `vfs-native` and `vfs-search`.

/// Host-process search backends: `CliSearcher` (rg / grep / find) and
/// `FffSearcher` (feature `vfs-search-fff`).
#[cfg(feature = "vfs-search")]
pub mod search;

#[cfg(feature = "vfs-native")]
pub mod dir_delta;
#[cfg(feature = "vfs-native")]
pub mod native_fs;

#[cfg(all(feature = "vfs-native", target_os = "linux", feature = "vfs-fuse"))]
pub mod fuse;

#[cfg(all(
    feature = "vfs-native",
    target_os = "linux",
    feature = "vfs-ptrace-nix"
))]
pub mod ptrace;

#[cfg(all(
    feature = "vfs-native",
    feature = "vfs-ipc",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
pub mod ipc_bus;

#[cfg(all(feature = "vfs-native", feature = "vfs-nfs"))]
pub mod nfs;

#[cfg(all(feature = "vfs-native", feature = "vfs-nfs"))]
pub use nfs::VfsNfs;

#[cfg(all(feature = "vfs-native", target_os = "linux", feature = "vfs-preload"))]
pub mod shim;

#[cfg(all(feature = "vfs-native", feature = "vfs-sqlite"))]
pub mod libsql_delta;

#[cfg(all(feature = "vfs-native", feature = "vfs-turso"))]
pub mod turso_delta;

#[cfg(all(feature = "vfs-native", any(feature = "vfs-d1", feature = "vfs-r2")))]
pub mod blob_delta;

#[cfg(all(feature = "vfs-native", feature = "vfs-d1"))]
pub mod d1_delta;

#[cfg(all(feature = "vfs-native", feature = "vfs-r2"))]
pub mod r2_delta;

#[cfg(feature = "vfs-native")]
pub use dir_delta::DirectoryDelta;
#[cfg(feature = "vfs-native")]
pub use native_fs::{NativeFs, NativeFsConfig};

#[cfg(all(feature = "vfs-native", target_os = "linux", feature = "vfs-fuse"))]
pub use fuse::{FuseMount, FuseMountOptions};

#[cfg(all(
    feature = "vfs-native",
    feature = "vfs-ipc",
    any(target_os = "linux", target_os = "macos", target_os = "windows")
))]
pub use ipc_bus::{
    run_daemon_loop, ClientTransport, DaemonTransport, VfsIpcConfig, VFS_BUS_IDENTIFIER,
};

#[cfg(all(feature = "vfs-native", feature = "vfs-sqlite"))]
pub use libsql_delta::LibsqlDelta;

#[cfg(all(feature = "vfs-native", feature = "vfs-turso"))]
pub use turso_delta::TursoDelta;

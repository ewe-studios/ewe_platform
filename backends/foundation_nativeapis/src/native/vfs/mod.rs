pub mod dir_delta;
pub mod native_fs;

#[cfg(all(target_os = "linux", feature = "vfs-fuse"))]
pub mod fuse;

#[cfg(all(target_os = "linux", feature = "vfs-ptrace-nix"))]
pub mod ptrace;

#[cfg(all(feature = "vfs-ipc", any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub mod ipc_bus;

#[cfg(feature = "vfs-fjall")]
pub mod fjall_fs;

#[cfg(feature = "vfs-preload")]
pub mod shim;

#[cfg(feature = "vfs-sqlite")]
pub mod libsql_delta;

#[cfg(feature = "vfs-turso")]
pub mod turso_delta;

#[cfg(feature = "vfs-d1")]
pub mod d1_delta;

#[cfg(feature = "vfs-r2")]
pub mod r2_delta;

pub use dir_delta::DirectoryDelta;
pub use native_fs::NativeFs;

#[cfg(all(target_os = "linux", feature = "vfs-fuse"))]
pub use fuse::{FuseMount, FuseMountOptions};

#[cfg(all(feature = "vfs-ipc", any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub use ipc_bus::{ClientTransport, DaemonTransport, VFS_BUS_IDENTIFIER, run_daemon_loop};

#[cfg(feature = "vfs-sqlite")]
pub use libsql_delta::{LibsqlDelta, SyncLibsqlDelta};

#[cfg(feature = "vfs-turso")]
pub use turso_delta::TursoDelta;

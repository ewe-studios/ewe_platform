pub mod dir_delta;
pub mod native_fs;

#[cfg(all(target_os = "linux", feature = "vfs-fuse"))]
pub mod fuse;

#[cfg(all(target_os = "linux", feature = "vfs-ptrace-nix"))]
pub mod ptrace;

#[cfg(all(feature = "vfs-ipc", any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub mod ipc_bus;

pub use dir_delta::DirectoryDelta;
pub use native_fs::NativeFs;

#[cfg(all(target_os = "linux", feature = "vfs-fuse"))]
pub use fuse::{FuseMount, FuseMountOptions};

#[cfg(all(feature = "vfs-ipc", any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub use ipc_bus::{ClientTransport, DaemonTransport, VFS_BUS_IDENTIFIER, run_daemon_loop};

//! Native adapters.
//!
//! Feature 01 adds the slave engine, feature 03 adds the guest-artifact store,
//! feature 05 adds the Firecracker driver, feature 09 adds libkrun and feature
//! 10 adds Docker.

/// The guest-artifact store: the cache of kernels, root file systems and
/// initramfs archives.
#[cfg(feature = "artifacts")]
pub mod artifacts;

/// The real host file system behind `HostFs`.
pub mod host;

/// The slave: the engine that serves `SlaveService` in the guest. It is a Linux
/// program: process control, signals and the PID 1 duties have no meaning on
/// another target.
#[cfg(all(target_os = "linux", feature = "slave"))]
pub mod slave;

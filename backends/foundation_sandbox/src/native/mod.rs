//! Native adapters.
//!
//! Feature 01 adds the slave engine, feature 03 adds the guest-artifact store,
//! feature 04 adds the Firecracker API client, feature 05 adds the Firecracker
//! driver, feature 09 adds libkrun and feature 10 adds Docker.

/// The guest-artifact store: the cache of kernels, root file systems and
/// initramfs archives.
#[cfg(feature = "artifacts")]
pub mod artifacts;

/// The Firecracker API client (feature 04). Feature 05 adds the driver here.
#[cfg(feature = "firecracker")]
pub mod firecracker;

/// The real host file system behind `HostFs`.
pub mod host;

/// The native platform wrapper (feature 05).
#[cfg(feature = "control")]
pub mod platform;

/// The client of a remote control plane (feature 06).
#[cfg(feature = "control")]
pub mod connect;

/// The `SandboxService` server on a Unix domain socket or TCP (feature 06).
#[cfg(feature = "control")]
pub mod serve;

/// The slave: the engine that serves `SlaveService` in the guest. It is a Linux
/// program: process control, signals and the PID 1 duties have no meaning on
/// another target.
#[cfg(all(target_os = "linux", feature = "slave"))]
pub mod slave;

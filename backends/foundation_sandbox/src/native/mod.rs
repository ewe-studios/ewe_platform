//! Native adapters.
//!
//! Feature 01 adds the slave engine, feature 05 adds the Firecracker driver,
//! feature 09 adds libkrun and feature 10 adds Docker.

/// The slave: the engine that serves `SlaveService` in the guest. It is a Linux
/// program: process control, signals and the PID 1 duties have no meaning on
/// another target.
#[cfg(all(target_os = "linux", feature = "slave"))]
pub mod slave;

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

/// The Docker driver and the container control (feature 10).
#[cfg(feature = "docker")]
pub mod docker;

/// The real host file system behind `HostFs`.
pub mod host;

/// The sandbox image store (feature 12). It starts `mkfs.ext4` and reads and
/// writes real files, so it is native Unix only.
#[cfg(all(unix, feature = "control"))]
pub mod images;

/// The libkrun driver and its runner (feature 09). The driver is native Unix
/// only; the runner compiles where `libkrun-runner` selects it.
#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    any(feature = "libkrun", feature = "libkrun-runner")
))]
pub mod libkrun;

/// The record store on disk (feature 10).
#[cfg(feature = "control")]
pub mod record;

/// The tar reader and applier of an image unpack (feature 12). It is portable
/// Unix code, so a test applies a tar to a temporary directory without root.
#[cfg(unix)]
pub mod unpack;

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

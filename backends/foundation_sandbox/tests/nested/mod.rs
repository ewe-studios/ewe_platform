//! The nested sandbox end-to-end tests (feature 11).
//!
//! They need Linux, Docker, Kernel-based Virtual Machine (KVM) and the nested
//! host image. A host without them prints the probe report and returns.

/// The real nested tests need Linux with Firecracker.
#[cfg(all(target_os = "linux", feature = "firecracker"))]
mod e2e;

/// The no-KVM host form starts a parent without the device. It needs no
/// Firecracker program of its own, so it also builds on macOS.
mod no_kvm;

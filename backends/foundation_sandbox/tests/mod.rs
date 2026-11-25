//! The test binary.
//!
//! Every test is a module of this binary, registered here.

mod pool;

mod contract;

/// The guest-artifact tests. They need the native store: the HTTP client, the
/// gzip decoder and the tar reader have no wasm form.
#[cfg(all(feature = "artifacts", not(target_family = "wasm")))]
mod artifacts;

/// The control-plane simulation tests. They need the `testing` helpers.
#[cfg(all(feature = "control", feature = "testing"))]
mod control;

/// The slave process tests. They need Linux: the slave is a Linux guest
/// program, and it controls processes, signals and mounts.
#[cfg(all(target_os = "linux", feature = "slave"))]
mod slave;

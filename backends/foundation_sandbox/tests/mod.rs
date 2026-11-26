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

/// The Firecracker API client tests. They need the `firecracker` feature and a
/// Unix domain socket.
#[cfg(all(feature = "firecracker", unix))]
mod firecracker;

/// The slave process tests. They need Linux: the slave is a Linux guest
/// program, and it controls processes, signals and mounts.
#[cfg(all(target_os = "linux", feature = "slave"))]
mod slave;

/// The Cloudflare tests. They need the `cloudflare` and `testing` features.
#[cfg(all(feature = "cloudflare", feature = "testing"))]
mod cloudflare;
/// The native side-effect tests of feature 05. They start real processes and
/// dial real Unix sockets, with no virtual machine.
#[cfg(all(unix, feature = "control"))]
mod native_host;

/// The `SandboxService` server and client on a real socket (feature 06). They
/// need the control plane and the simulation helpers; no virtual machine runs.
#[cfg(all(unix, feature = "control", feature = "testing"))]
mod serve;

/// The command-line tests of feature 06.
#[cfg(all(not(target_family = "wasm"), feature = "cli"))]
mod cli;

/// The source scans of invariants I10 and I12 (feature 06).
#[cfg(not(target_family = "wasm"))]
mod boundary;

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

/// The sandbox image tests (feature 12). They need the control plane and its
/// simulation helpers; the store reads real files.
#[cfg(all(unix, feature = "control", feature = "testing"))]
mod images;

/// The Firecracker API client tests. They need the `firecracker` feature and a
/// Unix domain socket.
#[cfg(all(feature = "firecracker", unix))]
mod firecracker;

/// The slave process tests. They need Linux: the slave is a Linux guest
/// program, and it controls processes, signals and mounts.
#[cfg(all(target_os = "linux", feature = "slave"))]
mod slave;

/// The native side-effect tests of feature 05. They start real processes and
/// dial real Unix sockets, with no virtual machine.
#[cfg(all(unix, feature = "control"))]
mod native_host;

/// The libkrun driver tests (feature 09). They need the simulation helpers and
/// a Unix host.
#[cfg(all(unix, feature = "libkrun", feature = "testing"))]
mod libkrun;

/// The libkrun runner tests (feature 09). They run the real `sandbox-krun`
/// program, which needs no libkrun for these two cases.
#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    feature = "libkrun-runner"
))]
mod libkrun_runner;

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

/// The Docker driver and container-control tests (feature 10). They need the
/// simulation helpers and a Unix host for the fake process spawner.
#[cfg(all(feature = "docker", feature = "testing", unix))]
mod docker;

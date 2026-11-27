//! The Firecracker API client tests (feature 04).
//!
//! Every test talks to a mock HTTP server on a Unix domain socket. One test
//! needs the real `firecracker` program; it returns with a message when the
//! program is absent.

mod body;
#[cfg(target_os = "linux")]
mod driver;
/// The real microVM tests. They need Linux with KVM and the programs.
#[cfg(all(target_os = "linux", feature = "control", feature = "artifacts"))]
mod e2e;
mod errors;
mod mock_socket;
mod real;
mod source;
mod support;

//! The Firecracker API client (feature 04).
//!
//! The generated request functions sit in [`generated`]. The hand-written
//! [`client`] adds the Unix domain socket transport, the time limit and the
//! error mapping. Feature 05 uses the client; this module starts no virtual
//! machine.
//!
//! The client is Unix only: the Firecracker API listens on a Unix domain
//! socket. The generated code builds on every native target.

/// The generated request functions, one module for each operation group.
pub mod generated;

/// The client over a Unix domain socket.
#[cfg(unix)]
pub mod client;

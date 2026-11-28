//! The Firecracker API client and driver (features 04 and 05).
//!
//! The generated request functions sit in [`generated`]. The hand-written
//! [`client`] adds the Unix domain socket transport, the time limit and the
//! error mapping. Feature 05 uses the client from the [`driver`].
//!
//! The client is Unix only: the Firecracker API listens on a Unix domain
//! socket. The generated code builds on every native target. The driver is
//! Linux only: it starts `firecracker` and needs Kernel-based Virtual Machine
//! (KVM).

/// The generated request functions, one module for each operation group.
pub mod generated;

/// The client over a Unix domain socket.
#[cfg(unix)]
pub mod client;

/// The driver: one `firecracker` process for one sandbox.
#[cfg(target_os = "linux")]
pub mod driver;

/// The test doubles of the driver (fake API, fake host).
#[cfg(all(target_os = "linux", feature = "testing"))]
pub mod testing;

#[cfg(target_os = "linux")]
pub use crate::native::firecracker_network::{
    GuestNetworkConfig, NetworkRuntime, TcpForward, UdpForward,
};
#[cfg(target_os = "linux")]
pub use driver::{boot_args, network_interface, FcPlan, FirecrackerDriver};

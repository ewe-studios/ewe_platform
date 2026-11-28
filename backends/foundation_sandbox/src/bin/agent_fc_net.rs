//! The `agent-fc-net` helper for the Firecracker guest network.
//!
//! This program makes the private user and network namespace of one micro virtual
//! machine (microVM), creates its TAP device and bridges the device frames to the
//! control plane over a Unix datagram pair. The control plane starts it with the
//! Firecracker program after `--` (spec 92, feature 02, task T11).
//!
//! The helper needs Linux, `/dev/net/tun` and unprivileged user namespaces. It needs
//! no root: the new user namespace gives it `CAP_NET_ADMIN` over its own network
//! namespace.

#[cfg(target_os = "linux")]
fn main() {
    std::process::exit(foundation_sandbox::native::firecracker_network::run_helper());
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("agent-fc-net: the Firecracker guest network needs Linux");
    std::process::exit(1);
}

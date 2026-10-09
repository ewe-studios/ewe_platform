//! Data plane wrapper — smoltcp or kernel TUN behind one concrete type (spec-55, F11).
//!
//! WHY: `TunnelDriver<D: DataPlane>` needs a single concrete type at construction
//! time. `MeshDataPlane` wraps `NetStack` (smoltcp, default) or `TunDataPlane`
//! (kernel, privileged) into one non-generic enum that implements `DataPlane`.
//!
//! WHAT: [`MeshDataPlane`] delegates `DataPlane` methods to the inner variant.
//! In TUN mode, the overlay socket API returns `Unsupported` — apps use OS sockets.
//! [`add_tun_route`] sets up the kernel overlay subnet route on Linux.
//!
//! HOW: Each `DataPlane` method matches on the enum. `tcp_connect`/`tcp_listen`/
//! `udp_bind` work in smoltcp mode; return `Unsupported` in TUN mode (kernel
//! owns TCP/IP — apps use `std::net::TcpStream`).

use std::io;
use std::net::SocketAddr;
use std::time::Instant;

use std::net::IpAddr;

use foundation_nativeapis::dataplane::netstack::{NetStack, NetStackConfig, OverlayListener, OverlayStream, OverlayUdp};
use foundation_nativeapis::dataplane::tun::{TunConfig, TunDataPlane};
use foundation_nativeapis::dataplane::DataPlane;

use crate::shared::config::DataPlaneMode;

/// Unified data plane — smoltcp userspace netstack or kernel TUN device.
pub enum MeshDataPlane {
    /// smoltcp userspace TCP/IP stack (default, no privileges, cross-platform).
    Stack(NetStack),
    /// Kernel TUN device (Linux/Darwin, requires CAP_NET_ADMIN or TUN ownership).
    #[allow(dead_code)]
    Tun(TunDataPlane),
}

impl MeshDataPlane {
    /// Create a data plane from config. Returns an error if TUN open fails
    /// (missing device, no permissions, etc.).
    pub fn from_config(mode: &DataPlaneMode, my_ip: IpAddr, prefix_len: u8) -> Result<Self, crate::shared::error::WgError> {
        Ok(match mode {
            #[cfg(not(target_family = "wasm"))]
            DataPlaneMode::Tun { name, mtu } => {
                let tun = TunDataPlane::open(&TunConfig {
                    name: name.clone(),
                    mtu: *mtu,
                    ipv6: false,
                })
                .map_err(|e| crate::shared::error::WgError::Config(
                    format!("TUN device '{}': {e}", name)
                ))?;
                #[cfg(target_os = "linux")]
                add_tun_route(name);
                tracing::info!(name = name, mtu = mtu, "kernel TUN data plane active");
                Self::Tun(tun)
            }
            #[cfg(target_family = "wasm")]
            DataPlaneMode::Tun { .. } => {
                panic!("kernel TUN is not available on wasm targets");
            }
            DataPlaneMode::Netstack => {
                Self::Stack(NetStack::new(NetStackConfig {
                    address: my_ip,
                    prefix_len,
                    mtu: 1380,
                }))
            }
        })
    }

    /// Whether this is a kernel TUN data plane (apps use OS sockets).
    pub fn is_tun(&self) -> bool {
        matches!(self, Self::Tun(_))
    }

    /// Access the smoltcp NetStack, if this is the Stack variant.
    pub fn netstack(&self) -> Option<&NetStack> {
        match self {
            Self::Stack(ns) => Some(ns),
            Self::Tun(_) => None,
        }
    }
}

impl DataPlane for MeshDataPlane {
    fn inject_inbound_ip(&mut self, packet: &[u8]) {
        match self {
            Self::Stack(dp) => dp.inject_inbound_ip(packet),
            Self::Tun(dp) => dp.inject_inbound_ip(packet),
        }
    }

    fn poll(&mut self, now: Instant) -> Option<Instant> {
        match self {
            Self::Stack(dp) => dp.poll(now),
            Self::Tun(dp) => dp.poll(now),
        }
    }

    fn drain_outbound_ip(&mut self, f: &mut dyn FnMut(&[u8])) {
        match self {
            Self::Stack(dp) => dp.drain_outbound_ip(f),
            Self::Tun(dp) => dp.drain_outbound_ip(f),
        }
    }

    fn tcp_listen(&mut self, addr: SocketAddr) -> io::Result<OverlayListener> {
        match self {
            Self::Stack(dp) => dp.tcp_listen(addr),
            Self::Tun(dp) => dp.tcp_listen(addr),
        }
    }

    fn tcp_connect(&mut self, addr: SocketAddr) -> io::Result<OverlayStream> {
        match self {
            Self::Stack(dp) => dp.tcp_connect(addr),
            Self::Tun(dp) => dp.tcp_connect(addr),
        }
    }

    fn udp_bind(&mut self, addr: SocketAddr) -> io::Result<OverlayUdp> {
        match self {
            Self::Stack(dp) => dp.udp_bind(addr),
            Self::Tun(dp) => dp.udp_bind(addr),
        }
    }
}

/// Add the overlay subnet route on Linux so the kernel routes 10.x.y.z
/// through the TUN device. No-op on non-Linux platforms.
///
/// Equivalent to: `ip route add 10.0.0.0/8 dev <name>`
#[cfg(target_os = "linux")]
pub fn add_tun_route(dev_name: &str) {
    use std::process::Command;
    let output = Command::new("ip")
        .args(["route", "add", "10.0.0.0/8", "dev", dev_name])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            tracing::info!(dev = dev_name, "added overlay route 10.0.0.0/8");
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            tracing::warn!(dev = dev_name, %stderr, "failed to add overlay route (may already exist)");
        }
        Err(e) => {
            tracing::warn!(dev = dev_name, error = %e, "ip route add command failed — route may need manual setup");
        }
    }
}

/// Stub for non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn add_tun_route(_dev_name: &str) {}

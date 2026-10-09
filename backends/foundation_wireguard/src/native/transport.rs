//! Transport adapters bridging smoltcp overlay sockets and kernel UDP for
//! the gossip and relay paths (spec-55, F11 TUN support).
//!
//! WHY: In NetStack mode, SWIM gossip uses `OverlayUdp` (smoltcp socket).
//! In TUN mode, the kernel owns TCP/IP so gossip must use a normal UDP
//! socket — the kernel routes packets through the TUN device. This enum
//! unifies both behind a `recv_from`/`send_to` interface so `WgMeshTask`
//! doesn't care which data plane is active.

use std::io;
use std::net::SocketAddr;

use foundation_nativeapis::dataplane::netstack::OverlayUdp;
use foundation_nativeapis::native::net::UdpSocket;

/// Gossip transport: either an smoltcp overlay socket (NetStack mode) or
/// a kernel UDP socket (TUN mode — kernel routes through the TUN device).
pub enum GossipTransport {
    /// smoltcp overlay UDP bound to the gossip port on the virtual interface.
    Overlay(OverlayUdp),
    /// Kernel UDP socket — used in TUN mode where `ip route add 10.0.0.0/8`
    /// sends overlay traffic through the TUN device.
    Kernel(UdpSocket),
}

impl GossipTransport {
    /// Wrap an smoltcp overlay UDP socket.
    pub fn overlay(sock: OverlayUdp) -> Self {
        Self::Overlay(sock)
    }

    /// Wrap a kernel UDP socket (TUN mode).
    pub fn kernel(sock: UdpSocket) -> Self {
        Self::Kernel(sock)
    }

    /// Receive a datagram from either transport.
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        match self {
            Self::Overlay(o) => o.recv_from(buf),
            Self::Kernel(k) => k.recv_from(buf),
        }
    }

    /// Send a datagram through either transport.
    pub fn send_to(&self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        match self {
            Self::Overlay(o) => o.send_to(buf, addr),
            Self::Kernel(k) => k.send_to(buf, addr),
        }
    }
}

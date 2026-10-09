//! WireGuard overlay data plane (spec-55, feature 00).
//!
//! WHY: `boringtun::Tunn` produces and consumes **raw IP packets** on its inner
//! boundary. To turn that boundary into usable sockets we need a TCP/IP stack. This
//! module provides the sans-I/O [`DataPlane`] trait plus two implementations — a
//! smoltcp userspace netstack ([`netstack::NetStack`], cross-target, unprivileged) and
//! a kernel TUN device ([`tun::TunDataPlane`], native-only, privileged) — so
//! `foundation_wireguard` can select the right one per platform.
//!
//! WHAT: A single trait that both data planes satisfy: feed decapsulated inbound IP
//! packets in, drain outbound IP packets out (to feed `Tunn::encapsulate`), advance
//! internal timers, and hand out overlay sockets ([`OverlayStream`],
//! [`OverlayListener`], [`OverlayUdp`]).
//!
//! HOW: The plane performs **no I/O of its own**. A single valtron task drives it:
//! it calls [`DataPlane::poll`] whenever the outer UDP socket is readable or a timer
//! expires, [`DataPlane::inject_inbound_ip`] with each plaintext packet returned by
//! `Tunn::decapsulate`, and [`DataPlane::drain_outbound_ip`] to collect packets to
//! encrypt and send. The overlay socket handles register wakers that fire when their
//! readiness changes, so application tasks can be re-scheduled.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

#[cfg(feature = "netstack")]
pub mod netstack;

#[cfg(feature = "tun")]
pub mod tun;

#[cfg(feature = "netstack")]
pub use netstack::{NetStack, NetStackConfig, OverlayListener, OverlayStream, OverlayUdp};

/// WHY: The data plane is sans-I/O and driven by a valtron task; when a socket's
/// readiness changes the plane must be able to wake the task(s) blocked on it without
/// knowing anything about how they are scheduled.
///
/// WHAT: A cheap, cloneable, thread-safe wake callback registered on an overlay socket.
///
/// HOW: The driver constructs a `WakeFn` that nudges its reactor/task; the plane
/// invokes it (one-shot) when the associated socket transitions to readable/writable.
pub type WakeFn = Arc<dyn Fn() + Send + Sync>;

/// WHY: `boringtun::Tunn`'s inner boundary speaks raw L3 packets; something must be the
/// TCP/IP stack that produces and consumes them. Both a userspace netstack and a kernel
/// TUN device fill that gap, and callers should not care which.
///
/// WHAT: A sans-I/O L3 data plane that converts overlay sockets to/from raw IP packets.
///
/// HOW: Implementors keep all protocol state in-process and expose it through this
/// trait. They never touch the network directly — the driver moves bytes between this
/// trait and `Tunn` + the UDP socket.
///
/// # Errors
/// The socket-construction methods return [`io::Error`]; the TUN implementation returns
/// [`io::ErrorKind::Unsupported`] from them because in TUN mode applications use the
/// kernel's own sockets, not overlay sockets.
///
/// # Panics
/// Implementations must not panic; malformed inbound packets are dropped.
pub trait DataPlane {
    /// Feed one decrypted inbound IP packet (as returned by `Tunn::decapsulate`).
    ///
    /// # Panics
    /// Never panics; oversized or malformed packets are dropped.
    fn inject_inbound_ip(&mut self, packet: &[u8]);

    /// Advance internal state (timers, TCP state machines) as of `now`, firing any
    /// socket wakers whose readiness changed. Returns the next instant at which the
    /// plane wants to be polled again, or `None` if it is idle.
    fn poll(&mut self, now: Instant) -> Option<Instant>;

    /// Drain outbound IP packets (to feed `Tunn::encapsulate`), invoking `f` once per
    /// packet in FIFO order.
    fn drain_outbound_ip(&mut self, f: &mut dyn FnMut(&[u8]));

    /// Open an overlay TCP listener bound to `addr`.
    ///
    /// # Errors
    /// Returns [`io::Error`] if the address is invalid or the plane does not support
    /// overlay sockets (kernel TUN mode).
    fn tcp_listen(&mut self, addr: SocketAddr) -> io::Result<OverlayListener>;

    /// Open an overlay TCP connection to `addr`.
    ///
    /// # Errors
    /// Returns [`io::Error`] if the address is invalid or the plane does not support
    /// overlay sockets (kernel TUN mode).
    fn tcp_connect(&mut self, addr: SocketAddr) -> io::Result<OverlayStream>;

    /// Bind an overlay UDP socket to `addr`.
    ///
    /// # Errors
    /// Returns [`io::Error`] if the address is invalid or the plane does not support
    /// overlay sockets (kernel TUN mode).
    fn udp_bind(&mut self, addr: SocketAddr) -> io::Result<OverlayUdp>;
}

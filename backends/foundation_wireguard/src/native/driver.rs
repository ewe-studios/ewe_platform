//! The tunnel driver — one valtron task couples `Tunn`(s) + a `DataPlane` + a UDP socket
//! (spec-55, feature 01).
//!
//! WHY: `Tunn` and smoltcp are both sans-I/O state machines. Exactly **one** task must
//! drive both (Tunn timers + the data-plane poll loop) so there is no cross-executor
//! waking, moving packets app ⇄ Tunn ⇄ UDP.
//!
//! WHAT: [`TunnelDriver`] holds the UDP socket, the overlay [`NetStack`], and one
//! [`WgTunnel`] per peer. [`TunnelDriver::drive_once`] performs one full pump. The valtron
//! wrapper [`TunnelDriverTask`] runs that pump on a schedule until stopped.
//!
//! HOW: Each pump (1) drains inbound UDP → `decapsulate` → inject decrypted packets into
//! the overlay (or reply to a handshake), (2) polls the overlay and `encapsulate`s
//! outbound IP packets to the right peer, and (3) advances every tunnel's timers. It
//! returns the next wake deadline (earlier of the overlay's `poll_at` and a periodic
//! WireGuard timer tick).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};
use foundation_nativeapis::dataplane::{DataPlane, NetStack};
use foundation_nativeapis::native::net::UdpSocket;

use crate::shared::tunnel::{WgOutcome, WgTunnel};

/// How often (ms) the driver re-runs to service WireGuard timers when otherwise idle.
/// WireGuard expects `update_timers` roughly every 100–250 ms.
const TIMER_TICK_MS: u64 = 250;

/// Maximum UDP datagram we will read in one recv.
const RECV_BUF: usize = 65_535;

/// One peer's tunnel plus its UDP endpoint and overlay address.
struct PeerLink {
    tunnel: WgTunnel,
    endpoint: SocketAddr,
    tunnel_ip: IpAddr,
}

impl std::fmt::Debug for PeerLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeerLink")
            .field("endpoint", &self.endpoint)
            .field("tunnel_ip", &self.tunnel_ip)
            .finish()
    }
}

/// Couples a set of WireGuard tunnels with one overlay data plane and one UDP socket.
pub struct TunnelDriver {
    socket: UdpSocket,
    dataplane: NetStack,
    peers: Vec<PeerLink>,
    recv_buf: Vec<u8>,
}

impl std::fmt::Debug for TunnelDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TunnelDriver")
            .field("peers", &self.peers)
            .finish_non_exhaustive()
    }
}

impl TunnelDriver {
    /// WHY: The mesh orchestrator builds a driver per WireGuard interface.
    ///
    /// WHAT: Create a driver over a bound UDP socket and an overlay data plane, with no
    /// peers yet.
    ///
    /// HOW: Stores both; peers are added with [`Self::add_peer`].
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn new(socket: UdpSocket, dataplane: NetStack) -> Self {
        Self {
            socket,
            dataplane,
            peers: Vec::new(),
            recv_buf: vec![0u8; RECV_BUF],
        }
    }

    /// WHY: Each discovered peer becomes a WireGuard tunnel with a known UDP endpoint and
    /// overlay IP.
    ///
    /// WHAT: Register a peer tunnel, its UDP endpoint, and its overlay (tunnel) address.
    ///
    /// HOW: Appends to the peer table; inbound datagrams and outbound packets are routed
    /// by endpoint and destination IP respectively.
    pub fn add_peer(&mut self, tunnel: WgTunnel, endpoint: SocketAddr, tunnel_ip: IpAddr) {
        self.peers.push(PeerLink {
            tunnel,
            endpoint,
            tunnel_ip,
        });
    }

    /// The overlay data plane (to open overlay sockets from application code).
    #[must_use]
    pub fn dataplane(&self) -> NetStack {
        self.dataplane.clone()
    }

    /// Whether a peer with the given overlay (tunnel) IP is already attached.
    #[must_use]
    pub fn has_peer_ip(&self, tunnel_ip: IpAddr) -> bool {
        self.peers.iter().any(|p| p.tunnel_ip == tunnel_ip)
    }

    /// Remove the peer with the given overlay IP (e.g. on `PeerDown`).
    pub fn remove_peer_ip(&mut self, tunnel_ip: IpAddr) {
        self.peers.retain(|p| p.tunnel_ip != tunnel_ip);
    }

    /// The overlay IPs of all attached peers.
    #[must_use]
    pub fn peer_ips(&self) -> Vec<IpAddr> {
        self.peers.iter().map(|p| p.tunnel_ip).collect()
    }

    /// Number of attached peers.
    #[must_use]
    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// The local UDP address the driver is bound to.
    ///
    /// # Errors
    /// Returns [`std::io::Error`] if the socket address cannot be read.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.get_ref().local_addr()
    }

    /// WHY: The heart of the crate — one pump of the couple `Tunn`↔overlay↔UDP.
    ///
    /// WHAT: Service inbound UDP, drive the overlay, encrypt outbound packets, and advance
    /// tunnel timers; return the next wake deadline.
    ///
    /// HOW: See the module docs. Returns the earlier of the overlay's `poll_at` and a
    /// periodic WireGuard timer tick.
    ///
    /// # Panics
    /// Never panics.
    pub fn drive_once(&mut self, now: Instant) -> Option<Instant> {
        self.pump_inbound_udp();
        let dp1 = self.pump_outbound_overlay(now);
        self.pump_timers();
        // Poll once more so packets injected this tick generate their ACK/data segments.
        let dp2 = self.dataplane.poll(now);

        let tick = now + Duration::from_millis(TIMER_TICK_MS);
        [dp1, dp2]
            .into_iter()
            .flatten()
            .chain(std::iter::once(tick))
            .min()
    }

    /// Drain all pending inbound datagrams, decapsulating each into the overlay.
    fn pump_inbound_udp(&mut self) {
        loop {
            let (n, src) = match self.socket.recv_from(&mut self.recv_buf) {
                Ok(v) => v,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    tracing::debug!(error = %e, "udp recv_from failed");
                    break;
                }
            };
            let datagram = self.recv_buf[..n].to_vec();
            let Some(idx) = self.route_by_endpoint(src) else {
                tracing::debug!(%src, "datagram from unknown endpoint dropped");
                continue;
            };
            handle_inbound(
                &mut self.peers[idx],
                &self.socket,
                &mut self.dataplane,
                &datagram,
                src,
            );
        }
    }

    /// Poll the overlay and encapsulate every outbound IP packet to the right peer.
    fn pump_outbound_overlay(&mut self, now: Instant) -> Option<Instant> {
        let deadline = self.dataplane.poll(now);
        let mut outbound: Vec<Vec<u8>> = Vec::new();
        self.dataplane.drain_outbound_ip(&mut |p| outbound.push(p.to_vec()));
        for ip_packet in outbound {
            let Some(idx) = self.route_by_dest_ip(&ip_packet) else {
                tracing::debug!("outbound packet with unroutable destination dropped");
                continue;
            };
            match self.peers[idx].tunnel.encapsulate(&ip_packet) {
                WgOutcome::WriteToNetwork(ciphertext) => {
                    let _ = self.socket.send_to(&ciphertext, self.peers[idx].endpoint);
                }
                WgOutcome::Error(err) => tracing::debug!(error = %err, "encapsulate failed"),
                // encapsulate never yields WriteToTunnel; Done means queued behind a handshake.
                WgOutcome::WriteToTunnel(_) | WgOutcome::Done => {}
            }
        }
        deadline
    }

    /// Advance every tunnel's timers, sending any keepalive/handshake packets produced.
    fn pump_timers(&mut self) {
        for peer in &mut self.peers {
            if let WgOutcome::WriteToNetwork(packet) = peer.tunnel.update_timers() {
                let _ = self.socket.send_to(&packet, peer.endpoint);
            }
        }
    }

    /// Find the peer a datagram came from (by UDP endpoint; single-peer fallback).
    fn route_by_endpoint(&self, src: SocketAddr) -> Option<usize> {
        if let Some(idx) = self.peers.iter().position(|p| p.endpoint == src) {
            return Some(idx);
        }
        (self.peers.len() == 1).then_some(0)
    }

    /// Find the peer an outbound IP packet is destined for (by overlay IP; single-peer
    /// fallback).
    fn route_by_dest_ip(&self, ip_packet: &[u8]) -> Option<usize> {
        if let Some(dest) = parse_dest_ip(ip_packet) {
            if let Some(idx) = self.peers.iter().position(|p| p.tunnel_ip == dest) {
                return Some(idx);
            }
        }
        (self.peers.len() == 1).then_some(0)
    }
}

/// Decapsulate one datagram into the overlay, replying to handshake network writes.
fn handle_inbound(
    peer: &mut PeerLink,
    socket: &UdpSocket,
    dataplane: &mut NetStack,
    datagram: &[u8],
    src: SocketAddr,
) {
    match peer.tunnel.decapsulate(Some(src.ip()), datagram) {
        WgOutcome::WriteToNetwork(ciphertext) => {
            let _ = socket.send_to(&ciphertext, peer.endpoint);
            // Drain any queued packets per boringtun's decapsulate contract.
            while let Some(more) = peer.tunnel.flush_network() {
                let _ = socket.send_to(&more, peer.endpoint);
            }
        }
        WgOutcome::WriteToTunnel(ip_packet) => dataplane.inject_inbound_ip(&ip_packet),
        WgOutcome::Error(err) => {
            tracing::debug!(endpoint = %peer.endpoint, error = %err, "decapsulate failed");
        }
        WgOutcome::Done => {}
    }
}

/// Parse the destination IP address out of a raw IPv4/IPv6 packet header.
fn parse_dest_ip(packet: &[u8]) -> Option<IpAddr> {
    match packet.first()? >> 4 {
        4 if packet.len() >= 20 => Some(IpAddr::V4(Ipv4Addr::new(
            packet[16], packet[17], packet[18], packet[19],
        ))),
        6 if packet.len() >= 40 => {
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&packet[24..40]);
            Some(IpAddr::V6(Ipv6Addr::from(octets)))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// valtron task wrapper
// ---------------------------------------------------------------------------

/// A valtron task that runs one [`TunnelDriver`] on a schedule until stopped.
///
/// Yields no values; it parks between pumps via [`TaskStatus::Delayed`] using the driver's
/// reported next-wake deadline (bounded by a periodic WireGuard timer tick).
pub struct TunnelDriverTask {
    driver: TunnelDriver,
    stop: Arc<AtomicBool>,
}

impl std::fmt::Debug for TunnelDriverTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TunnelDriverTask")
            .field("driver", &self.driver)
            .field("stopped", &self.stop.load(Ordering::Relaxed))
            .finish()
    }
}

impl TunnelDriverTask {
    /// WHY: Run the driver as a first-class valtron task, cancellable from elsewhere.
    ///
    /// WHAT: Wrap a [`TunnelDriver`], returning the task and a stop flag that ends it.
    ///
    /// HOW: Sets `stop` to signal `next_status` to return `None` (task completes).
    #[must_use]
    pub fn new(driver: TunnelDriver) -> (Self, Arc<AtomicBool>) {
        let stop = Arc::new(AtomicBool::new(false));
        (
            Self {
                driver,
                stop: Arc::clone(&stop),
            },
            stop,
        )
    }
}

impl TaskIterator for TunnelDriverTask {
    type Ready = ();
    type Pending = ();
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if self.stop.load(Ordering::Relaxed) {
            return None;
        }
        let now = Instant::now();
        let deadline = self.driver.drive_once(now);
        let delay = deadline
            .map(|d| d.saturating_duration_since(now))
            .unwrap_or_else(|| Duration::from_millis(TIMER_TICK_MS))
            .max(Duration::from_millis(1));
        Some(TaskStatus::Delayed(delay))
    }
}

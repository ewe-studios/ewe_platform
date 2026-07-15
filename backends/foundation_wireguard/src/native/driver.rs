//! The tunnel driver — one valtron task couples `Tunn`(s) + a `DataPlane` + a UDP socket
//! (spec-55, feature 01).
//!
//! WHY: `Tunn` and smoltcp are both sans-I/O state machines. Exactly **one** task must
//! drive both (Tunn timers + the data-plane poll loop) so there is no cross-executor
//! waking, moving packets app ⇄ Tunn ⇄ UDP.
//!
//! WHAT: [`TunnelDriver`] holds the UDP socket, the overlay [`NetStack`], and one
//! [`WgTunnel`] per peer. [`TunnelDriver::drive_once`] performs one full pump and reports
//! whether any real work was done (inbound decap, outbound encap, or timer fire). The
//! valtron wrapper [`TunnelDriverTask`] runs that pump once per wake; when idle it parks
//! on the **reactor** (epoll/kqueue) via the UDP socket's fd — no fixed-interval
//! polling. The executor wakes the task only when a datagram arrives.
//!
//! HOW: Each pump (1) drains inbound UDP → `decapsulate` → inject decrypted packets into
//! the overlay (or reply to a handshake), (2) polls the overlay and `encapsulate`s
//! outbound IP packets to the right peer, and (3) advances every tunnel's timers. It
//! returns a [`DriveOutcome`] with a `had_work` flag and the next WG timer deadline.
//! The task registers the UDP fd with the shared reactor once at construction; after a
//! pump that processed nothing it parks on `TaskStatus::Depends(SharedReadiness)`, waking
//! only when the selector latches readable events.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_core::valtron::{BoxedSendExecutionAction, EventReadiness, TaskIterator, TaskStatus};
use foundation_nativeapis::dataplane::{DataPlane, NetStack};
use foundation_nativeapis::native::fd::{Reactor, SharedReadiness};
use foundation_nativeapis::native::net::UdpSocket;
use foundation_nativeapis::native::poll::{Interest, Token};
use crate::shared::tunnel::{WgOutcome, WgTunnel};

/// How often (ms) the driver re-checks WireGuard timers when otherwise idle.
/// WG keepalive and rekey timeouts are on the order of seconds, so a 1 s floor
/// is safe — the reactor (epoll/kqueue) wakes the task sooner when data arrives.
const TIMER_TICK_MS: u64 = 1_000;

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
///
/// Generic over `D: DataPlane` — smoltcp `NetStack` (default) or kernel `TunDataPlane`.
/// Monomorphized at compile time: `TunnelDriver<NetStack>` has zero overhead vs the
/// non-generic version.
pub struct TunnelDriver<D: DataPlane = NetStack> {
    socket: UdpSocket,
    dataplane: D,
    peers: Vec<PeerLink>,
    recv_buf: Vec<u8>,
}

impl<D: DataPlane> std::fmt::Debug for TunnelDriver<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TunnelDriver")
            .field("peers", &self.peers)
            .finish_non_exhaustive()
    }
}

impl<D: DataPlane> TunnelDriver<D> {
    #[must_use]
    pub fn new(socket: UdpSocket, dataplane: D) -> Self {
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

    /// Clone the data plane (available when D: Clone). For `NetStack` this
    /// is a cheap `Arc` bump; for `TunDataPlane` it is unavailable.
    pub fn dataplane_cloned(&self) -> D
    where
        D: Clone,
    {
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

    /// The raw file descriptor of the underlying UDP socket, for reactor
    /// registration (`epoll`/`kqueue`).
    #[must_use]
    pub fn udp_fd(&self) -> std::os::unix::io::RawFd {
        self.socket.as_raw_fd()
    }

    /// WHY: The heart of the crate — one pump of the couple `Tunn`↔overlay↔UDP.
    ///
    /// WHAT: Service inbound UDP, drive the overlay, encrypt outbound packets, and advance
    /// tunnel timers; return whether the pump processed anything and the next wake deadline.
    ///
    /// HOW: See the module docs. Returns `(had_work, deadline)`: `had_work` is true when at
    /// least one datagram was decapsulated, one overlay packet was encapsulated, or a timer
    /// produced a keepalive. The deadline is the earlier of the overlay's `poll_at` and the
    /// periodic WireGuard timer tick.
    ///
    /// # Panics
    /// Never panics.
    pub fn drive_once(&mut self, now: Instant) -> (bool, Option<Instant>) {
        let inbound = self.pump_inbound_udp();
        let dp1 = self.pump_outbound_overlay(now);
        let timer_sent = self.pump_timers();
        // Poll once more so packets injected this tick generate their ACK/data segments.
        let dp2 = self.dataplane.poll(now);

        let had_work = inbound > 0 || timer_sent;
        let tick = now + Duration::from_millis(TIMER_TICK_MS);
        let deadline = [dp1, dp2]
            .into_iter()
            .flatten()
            .chain(std::iter::once(tick))
            .min();
        (had_work, deadline)
    }

    /// Drain all pending inbound datagrams, decapsulating each into the overlay.
    /// Returns the number of datagrams processed.
    fn pump_inbound_udp(&mut self) -> usize {
        let mut count = 0;
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
            count += 1;
        }
        count
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
    /// Returns true when at least one timer produced a network write.
    fn pump_timers(&mut self) -> bool {
        let mut sent = false;
        for peer in &mut self.peers {
            if let WgOutcome::WriteToNetwork(packet) = peer.tunnel.update_timers() {
                let _ = self.socket.send_to(&packet, peer.endpoint);
                sent = true;
            }
        }
        sent
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
fn handle_inbound<D: DataPlane>(
    peer: &mut PeerLink,
    socket: &UdpSocket,
    dataplane: &mut D,
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

/// A valtron task that runs one [`TunnelDriver`], parking on reactor fd readiness
/// instead of a fixed timer tick (spec-55 F01 must-do: reactor parking).
///
/// WHY: the old design polled every 250 ms regardless of traffic. Now the UDP
/// socket fd is registered with the shared reactor (epoll/kqueue); the executor
/// parks this task entirely and wakes it only when a datagram arrives or the WG
/// timer deadline passes. Zero CPU spin, zero idle wakeups.
///
/// WHAT: on each wake, [`TunnelDriver::drive_once`] drains all pending UDP,
/// services the overlay, and fires timers. If no work was done the task parks on
/// `TaskStatus::Depends(readiness)` — a composite signal that unparks on either
/// UDP data (reactor latch) or the next WG timer deadline.
///
/// HOW: the UDP fd is registered once at construction with `Interest::READABLE`.
/// After each pump the latched readiness is cleared so the next edge fires.
pub struct TunnelDriverTask {
    driver: TunnelDriver,
    stop: Arc<AtomicBool>,
    /// Reactor-backed readiness for the UDP socket — wakes the task when a
    /// datagram arrives. `None` when reactor initialisation failed; the task
    /// falls back to a 1 s `Delayed` poll in that case.
    readiness: Option<Arc<SharedReadiness>>,
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
    /// WHAT: Wrap a [`TunnelDriver`], registering its UDP socket with the shared
    /// reactor so the task parks on fd readiness instead of polling.
    ///
    /// HOW: calls [`Reactor::get`] to obtain the shared epoll/kqueue singleton,
    /// registers the UDP fd for `READABLE`, and stores a [`SharedReadiness`].
    /// Returns the task and a stop flag that ends it.
    #[must_use]
    pub fn new(driver: TunnelDriver) -> (Self, Arc<AtomicBool>) {
        let stop = Arc::new(AtomicBool::new(false));
        let readiness = Reactor::get()
            .ok()
            .and_then(|reactor| {
                let fd = driver.socket.get_ref().as_raw_fd();
                let token = Token(0); // single-fd task — token 0 is fine
                SharedReadiness::new(fd, reactor, token, Interest::READABLE)
                    .ok()
                    .map(Arc::new)
            });
        (
            Self {
                driver,
                stop: Arc::clone(&stop),
                readiness,
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
        let (had_work, deadline) = self.driver.drive_once(now);

        // Clear the latched readiness edge so the next event fires a fresh wake.
        if let Some(ref r) = self.readiness {
            r.clear(foundation_nativeapis::native::fd::Ready::READABLE);
        }

        // If we processed real work, yield Ready so the scheduler re-runs us
        // immediately — there may be queued datagrams or overlay packets.
        if had_work {
            return Some(TaskStatus::Ready(()));
        }

        // No work this pump. Park until the reactor signals data or the WG timer
        // deadline arrives. If reactor init failed, fall back to a timed poll.
        if let Some(ref readiness) = self.readiness {
            // Compute how long we can park before a WG timer needs service.
            let max_park = deadline
                .map(|d| d.saturating_duration_since(now))
                .unwrap_or_else(|| Duration::from_millis(TIMER_TICK_MS))
                .max(Duration::from_millis(1));

            // If the deadline has already passed, re-run immediately.
            if let Some(d) = deadline {
                if d <= now {
                    return Some(TaskStatus::Ready(()));
                }
            }

            // Composite park: ready if either the fd is readable OR the timeout elapsed.
            let composite = TimedReadiness {
                fd: Arc::clone(readiness),
                deadline: now + max_park,
            };
            Some(TaskStatus::Depends(Arc::new(composite)))
        } else {
            // No reactor — fall back to a 1 s poll.
            let delay = deadline
                .map(|d| d.saturating_duration_since(now))
                .unwrap_or_else(|| Duration::from_millis(TIMER_TICK_MS))
                .max(Duration::from_millis(1));
            Some(TaskStatus::Delayed(delay))
        }
    }
}

/// Composite `EventReadiness`: ready when either the reactor fd is readable OR
/// a wall-clock deadline has passed. Used by [`TunnelDriverTask`] to park until
/// the next datagram arrives while still servicing WG timers on schedule.
pub(crate) struct TimedReadiness {
    pub(crate) fd: Arc<SharedReadiness>,
    pub(crate) deadline: Instant,
}

impl std::fmt::Debug for TimedReadiness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimedReadiness")
            .field("deadline", &self.deadline)
            .finish()
    }
}

impl EventReadiness for TimedReadiness {
    fn is_ready(&self, _dur: Option<Duration>) -> bool {
        self.fd.is_ready(None) || Instant::now() >= self.deadline
    }
}

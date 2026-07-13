//! Native relay server, client, and hole-punch coordinator (spec-55, F05).
//!
//! WHY: Every relay-capable node runs a [`RelayServer`] that forwards encrypted frames
//! between peers that cannot reach each other directly (NAT, browser, …). The server
//! never decrypts — WG crypto is end-to-end (decision 07, trustless).
//!
//! WHAT: [`RelayServer`] (session management, forwarding, rate limits, GC),
//! [`RelayClient`] (encapsulates frames for relay forwarding), and
//! [`HolePuncher`] (gossip-driven NAT traversal coordinator).
//!
//! HOW: The relay server runs in the mesh runtime thread alongside the tunnel driver
//! and SWIM machine. Sessions are authenticated (the client is already a mesh peer).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crate::shared::membership::PeerId;
use crate::shared::relay::{ConnectivityPath, RelayFrame, RelaySelector, RelaySession};

// ---------------------------------------------------------------------------
// RelayServer
// ---------------------------------------------------------------------------

/// An active relay session — one peer attached to this relay.
struct RelaySessionEntry {
    /// When this session was last active.
    last_active: Instant,
    /// Total bytes forwarded for this session.
    byte_count: AtomicU64,
    /// Packet count for rate limiting (reset every second).
    packet_count: AtomicU64,
}

/// A trustless ciphertext relay server (decision 07).
///
/// WHY: Makes connectivity work when direct UDP is impossible — NAT, browser,
/// or firewall.
///
/// WHAT: Receives [`RelayFrame`]s from authenticated peers, looks up the
/// destination peer's session, and forwards the encrypted bytes. Never decrypts.
///
/// HOW: One instance per node (if relay is enabled). Runs within the mesh
/// runtime thread — `service` is called once per tick.
pub struct RelayServer {
    /// Active sessions, keyed by peer id.
    sessions: HashMap<PeerId, RelaySessionEntry>,
    /// Per-peer rate limit (packets per second).
    rate_limit_pps: u32,
    /// Session idle timeout.
    idle_timeout_secs: u64,
    /// Maximum concurrent sessions.
    max_sessions: u32,
}

/// A frame received and ready to be forwarded to the destination peer.
#[derive(Debug)]
pub struct ForwardedFrame {
    /// The ciphertext bytes to deliver to `dst`.
    pub ciphertext: Vec<u8>,
    /// The destination peer id.
    pub dst: PeerId,
    /// The source peer id (who sent it).
    pub src: PeerId,
}

impl RelayServer {
    /// WHY: A relay needs an upper bound on resource usage.
    ///
    /// WHAT: Create a new relay server with rate limiting and session caps.
    pub fn new(
        max_sessions: u32,
        rate_limit_pps: u32,
        idle_timeout_secs: u64,
    ) -> Self {
        Self {
            sessions: HashMap::new(),
            rate_limit_pps,
            idle_timeout_secs,
            max_sessions,
        }
    }

    /// WHY: Only mesh-authenticated peers may use the relay.
    ///
    /// WHAT: Register (or refresh) a relay session for `peer_id`.
    ///
    /// HOW: If the session already exists, bump `last_active`. If at capacity,
    /// reject the oldest idle session.
    pub fn attach(&mut self, peer_id: PeerId) -> bool {
        if let Some(entry) = self.sessions.get_mut(&peer_id) {
            entry.last_active = Instant::now();
            return true;
        }
        if self.sessions.len() >= self.max_sessions as usize {
            // Evict the oldest idle session.
            self.gc_one();
            if self.sessions.len() >= self.max_sessions as usize {
                return false; // still full
            }
        }
        self.sessions.insert(
            peer_id,
            RelaySessionEntry {
                last_active: Instant::now(),
                byte_count: AtomicU64::new(0),
                packet_count: AtomicU64::new(0),
            },
        );
        true
    }

    /// WHY: The relay receives a frame from a peer and must forward it to the
    /// destination without decrypting.
    ///
    /// WHAT: Decode the [`RelayFrame`], check the rate limit, and return a
    /// [`ForwardedFrame`] if the destination has an active session.
    ///
    /// HOW: Lookup the dst session; if present, bump the src's packet counter
    /// and return the ciphertext. The caller delivers it to the dst endpoint.
    pub fn forward(&mut self, src: PeerId, raw: &[u8]) -> Option<ForwardedFrame> {
        let frame = RelayFrame::decode(raw)?;

        // Rate-limit the source.
        if let Some(entry) = self.sessions.get(&src) {
            let count = entry.packet_count.fetch_add(1, Ordering::Relaxed) + 1;
            if count > self.rate_limit_pps as u64 {
                return None;
            }
        } else {
            return None; // not attached
        }

        // Lookup destination.
        let dst_entry = self.sessions.get(&frame.dst_peer_id)?;
        dst_entry.byte_count.fetch_add(frame.ciphertext.len() as u64, Ordering::Relaxed);

        Some(ForwardedFrame {
            ciphertext: frame.ciphertext,
            dst: frame.dst_peer_id,
            src,
        })
    }

    /// WHY: Stale sessions leak memory and relay slots.
    ///
    /// WHAT: Drop sessions that have been idle for longer than `idle_timeout_secs`.
    ///
    /// HOW: Called every tick; iterates once, removing stale entries.
    pub fn gc(&mut self) {
        let timeout = self.idle_timeout_secs;
        let now = Instant::now();
        self.sessions.retain(|_, e| {
            now.duration_since(e.last_active).as_secs() < timeout
        });
    }

    /// Evict the single oldest idle session (for capacity management).
    fn gc_one(&mut self) {
        if let Some((&id, _)) = self
            .sessions
            .iter()
            .min_by_key(|(_, e)| e.last_active)
        {
            self.sessions.remove(&id);
        }
    }

    /// Reset per-second rate-limit counters.
    pub fn reset_rate_limits(&mut self) {
        for entry in self.sessions.values() {
            entry.packet_count.store(0, Ordering::Relaxed);
        }
    }

    /// Whether `peer_id` has an active relay session.
    #[must_use]
    pub fn is_attached(&self, peer_id: &PeerId) -> bool {
        self.sessions.contains_key(peer_id)
    }

    /// Number of active sessions.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Detach a peer (e.g. on disconnect).
    pub fn detach(&mut self, peer_id: &PeerId) {
        self.sessions.remove(peer_id);
    }
}

// ---------------------------------------------------------------------------
// RelayClient — sends traffic through a relay
// ---------------------------------------------------------------------------

/// A client that wraps outbound WG packets in [`RelayFrame`]s and sends them
/// through the selected relay.
pub struct RelayClient {
    selector: RelaySelector,
    /// Per-peer relay sessions tracking the connectivity ladder.
    peers: HashMap<PeerId, RelaySession>,
}

impl RelayClient {
    /// WHY: The mesh needs to know which relay to use.
    ///
    /// WHAT: Create a new client with the given selection strategy.
    #[must_use]
    pub fn new(selector: RelaySelector) -> Self {
        Self {
            selector,
            peers: HashMap::new(),
        }
    }

    /// WHY: The SWIM membership changed; refresh relay candidates.
    pub fn on_membership_update(
        &mut self,
        members: &[crate::shared::membership::PeerRecord],
    ) {
        self.selector.update(members);
    }

    /// The currently selected relay peer, if any.
    #[must_use]
    pub fn selected_relay(&mut self) -> Option<PeerId> {
        self.selector.select().map(|c| c.peer_id)
    }

    /// WHY: The mesh driver needs to know whether to send a packet direct or
    /// via relay.
    ///
    /// WHAT: Return the peer's current connectivity path, creating the session
    /// if it doesn't exist.
    #[must_use]
    pub fn peer_path(&mut self, peer: PeerId) -> ConnectivityPath {
        self.peers.entry(peer).or_default().path
    }

    /// Record a successful direct probe to `peer`.
    pub fn on_direct_probe_success(&mut self, peer: PeerId) {
        self.peers.entry(peer).or_default().on_direct_success();
    }

    /// Direct connectivity to `peer` failed; step down the ladder.
    pub fn on_direct_probe_failure(&mut self, peer: PeerId) {
        let relay = self.selected_relay();
        self.peers.entry(peer).or_default().on_direct_failure(relay);
    }

    /// Hole-punch succeeded; upgrade to direct.
    pub fn on_hole_punch_success(&mut self, peer: PeerId) {
        self.peers.entry(peer).or_default().on_hole_punch_success();
    }

    /// Record an RTT measurement for `peer`.
    pub fn record_rtt(&mut self, peer: PeerId, rtt_us: u64) {
        self.selector.record_rtt(peer, rtt_us);
    }

    /// The mutable selector (for failover from outside).
    #[must_use]
    pub fn selector_mut(&mut self) -> &mut RelaySelector {
        &mut self.selector
    }
}

// ---------------------------------------------------------------------------
// HolePuncher — gossip-driven NAT traversal
// ---------------------------------------------------------------------------

/// Stage of a hole-punch attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HolePunchStage {
    /// Gathering endpoints from gossip.
    Gathering,
    /// Sent SYN to the peer's observed endpoint; waiting for response.
    SynSent(Instant),
    /// Received a response; punch succeeded.
    Confirmed,
    /// Timed out or refused.
    Failed,
}

/// An in-progress hole-punch attempt.
struct HolePunchAttempt {
    peer: PeerId,
    stage: HolePunchStage,
    learned_endpoints: Vec<SocketAddr>,
}

/// WHY: Two peers behind NAT can often reach each other by simultaneously sending
/// UDP packets to the endpoint the other side observed (classic hole-punch).
///
/// WHAT: Coordinates the "learn endpoints from gossip → simultaneous open → confirm"
/// flow for a set of peers.
///
/// HOW: The `initiate` method starts the punch; `on_punch_packet` processes inbound
/// punch SYN/ACK packets; `tick` drives timeouts. Punch packets are small tagged
/// datagrams sent over the same WG UDP socket.
pub struct HolePuncher {
    /// Pending hole-punch attempts, keyed by peer id.
    pending: HashMap<PeerId, HolePunchAttempt>,
    /// Timeout for a hole-punch attempt.
    punch_timeout_secs: u64,
}

/// Magic bytes prepended to hole-punch packets for demux.
const PUNCH_MAGIC: &[u8; 4] = b"HPv1";
/// SYN packet (initiator → target).
const PUNCH_SYN: u8 = 0x01;
/// ACK packet (target → initiator).
const PUNCH_ACK: u8 = 0x02;

impl HolePuncher {
    /// WHY: Hole-punch attempts should not hang forever.
    ///
    /// WHAT: Create a new coordinator with the given timeout.
    #[must_use]
    pub fn new(punch_timeout_secs: u64) -> Self {
        Self {
            pending: HashMap::new(),
            punch_timeout_secs,
        }
    }

    /// WHY: A peer cannot be reached directly — try hole-punch.
    ///
    /// WHAT: Start a hole-punch attempt for `peer` using endpoints observed via gossip.
    pub fn initiate(&mut self, peer: PeerId, gossip_endpoints: &[SocketAddr]) {
        if self.pending.contains_key(&peer) {
            return; // already in progress
        }
        self.pending.insert(
            peer,
            HolePunchAttempt {
                peer,
                stage: HolePunchStage::Gathering,
                learned_endpoints: gossip_endpoints.to_vec(),
            },
        );
    }

    /// WHY: The punch loop needs to know what to send.
    ///
    /// WHAT: If a SYN should be sent to the target peer, return its endpoint.
    /// Caller sends a small tagged datagram.
    #[must_use]
    pub fn drain_syns(&mut self, now: Instant) -> Vec<(PeerId, SocketAddr)> {
        let mut out = Vec::new();
        for attempt in self.pending.values_mut() {
            if let HolePunchStage::Gathering = attempt.stage {
                if let Some(&ep) = attempt.learned_endpoints.first() {
                    attempt.stage = HolePunchStage::SynSent(now);
                    out.push((attempt.peer, ep));
                } else {
                    attempt.stage = HolePunchStage::Failed;
                }
            }
        }
        // Remove failed attempts.
        self.pending.retain(|_, a| a.stage != HolePunchStage::Failed);
        out
    }

    /// Build a hole-punch SYN datagram for sending over the WG socket.
    #[must_use]
    pub fn build_syn(target: PeerId) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + 1 + 32);
        buf.extend_from_slice(PUNCH_MAGIC);
        buf.push(PUNCH_SYN);
        buf.extend_from_slice(target.as_bytes());
        buf
    }

    /// Build a hole-punch ACK datagram.
    #[must_use]
    pub fn build_ack(target: PeerId) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4 + 1 + 32);
        buf.extend_from_slice(PUNCH_MAGIC);
        buf.push(PUNCH_ACK);
        buf.extend_from_slice(target.as_bytes());
        buf
    }

    /// WHY: We received a hole-punch packet — is it a SYN or ACK?
    ///
    /// WHAT: Process an inbound punch packet. Returns `Some(endpoint)` if the
    /// punch succeeded (peer is now reachable at that endpoint), or `None` if
    /// more steps are needed.
    pub fn on_punch_packet(
        &mut self,
        from: PeerId,
        from_endpoint: SocketAddr,
        data: &[u8],
    ) -> Option<SocketAddr> {
        if data.len() < 5 || &data[..4] != PUNCH_MAGIC {
            return None;
        }
        match data[4] {
            PUNCH_SYN => {
                // Received SYN: record the endpoint and respond with ACK.
                // (The caller sends the ACK.)
                if let Some(attempt) = self.pending.get_mut(&from) {
                    attempt.learned_endpoints.push(from_endpoint);
                }
                None // Caller should send ACK.
            }
            PUNCH_ACK => {
                // Received ACK: punch succeeded.
                if let Some(attempt) = self.pending.get_mut(&from) {
                    attempt.stage = HolePunchStage::Confirmed;
                }
                Some(from_endpoint)
            }
            _ => None,
        }
    }

    /// WHY: Stale hole-punch attempts should not hang.
    ///
    /// WHAT: Remove timed-out attempts, return any confirmed ones.
    pub fn tick(&mut self, now: Instant) -> Vec<PeerId> {
        let mut confirmed = Vec::new();
        let timeout = self.punch_timeout_secs;
        self.pending.retain(|_, a| match a.stage {
            HolePunchStage::Confirmed => {
                confirmed.push(a.peer);
                false // remove confirmed
            }
            HolePunchStage::SynSent(sent) => {
                now.duration_since(sent).as_secs() < timeout
            }
            _ => true,
        });
        confirmed
    }

    /// Whether we have a pending punch attempt for `peer`.
    #[must_use]
    pub fn has_pending(&self, peer: &PeerId) -> bool {
        self.pending.contains_key(peer)
    }

    /// Number of pending hole-punch attempts.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn test_peer(id: u8) -> PeerId {
        PeerId([id; 32])
    }

    // ── RelayServer ──

    #[test]
    fn server_attach_and_forward() {
        let mut server = RelayServer::new(16, 1000, 60);

        let src = test_peer(1);
        let dst = test_peer(2);

        assert!(server.attach(src));
        assert!(server.attach(dst));

        let frame = RelayFrame::new(dst, b"hello".to_vec());
        let raw = frame.encode();
        let forwarded = server.forward(src, &raw).expect("forward");
        assert_eq!(forwarded.ciphertext, b"hello");
        assert_eq!(forwarded.dst, dst);
        assert_eq!(forwarded.src, src);
    }

    #[test]
    fn server_rejects_unattached_sender() {
        let mut server = RelayServer::new(16, 1000, 60);

        server.attach(test_peer(2)); // only dst attached
        let frame = RelayFrame::new(test_peer(2), b"data".to_vec());
        let raw = frame.encode();
        assert!(server.forward(test_peer(1), &raw).is_none());
    }

    #[test]
    fn server_gc_removes_stale_sessions() {
        let mut server = RelayServer::new(16, 1000, 0); // 0s timeout
        server.attach(test_peer(1));
        assert_eq!(server.session_count(), 1);
        server.gc();
        assert_eq!(server.session_count(), 0);
    }

    // ── RelayClient / RelaySession (shared tests already cover the state machine) ──

    #[test]
    fn client_tracks_peer_paths() {
        let selector = RelaySelector::new(
            crate::shared::relay::RelayStrategy::LowestLoad,
        );
        let mut client = RelayClient::new(selector);

        client.on_direct_probe_success(test_peer(1));
        assert_eq!(client.peer_path(test_peer(1)), ConnectivityPath::Direct);

        let relay = test_peer(99);
        client.selector_mut().update(&[]); // no real relays, but path still tracks
        client.on_direct_probe_failure(test_peer(1));
        // Falls to HolePunching since no relay candidates
        assert_eq!(client.peer_path(test_peer(1)), ConnectivityPath::HolePunching);
    }

    // ── HolePuncher ──

    #[test]
    fn hole_punch_syn_ack_flow() {
        let mut puncher = HolePuncher::new(30);
        let peer = test_peer(42);
        let ep: SocketAddr = "127.0.0.1:9999".parse().expect("addr");

        puncher.initiate(peer, &[ep]);
        assert!(puncher.has_pending(&peer));

        // Drain SYN.
        let syns = puncher.drain_syns(Instant::now());
        assert_eq!(syns.len(), 1);
        assert_eq!(syns[0].1, ep);

        // Process ACK from peer.
        let ack = HolePuncher::build_ack(test_peer(42));
        let result = puncher.on_punch_packet(peer, ep, &ack);
        assert_eq!(result, Some(ep));

        // Tick should confirm.
        let confirmed = puncher.tick(Instant::now());
        assert_eq!(confirmed, vec![peer]);
        assert!(!puncher.has_pending(&peer));
    }

    #[test]
    fn hole_punch_build_syn_contains_magic() {
        let syn = HolePuncher::build_syn(test_peer(7));
        assert_eq!(&syn[..4], PUNCH_MAGIC);
        assert_eq!(syn[4], PUNCH_SYN);
        assert_eq!(&syn[5..37], &[7u8; 32]);
    }

    #[test]
    fn hole_punch_build_ack_contains_magic() {
        let ack = HolePuncher::build_ack(test_peer(9));
        assert_eq!(&ack[..4], PUNCH_MAGIC);
        assert_eq!(ack[4], PUNCH_ACK);
    }
}

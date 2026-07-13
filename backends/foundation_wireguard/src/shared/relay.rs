//! Relay framing, selection, and connectivity-ladder session model (spec-55, F05).
//!
//! WHY: The relay is a distributed capability — any authenticated native node can opt in
//! (decision 07). Peers need shared types to frame relayed ciphertext, select the best
//! relay from the SWIM membership, and track their connectivity-ladder state
//! (direct → hole-punch → relay fallback).
//!
//! WHAT: [`RelayFrame`] + codec, [`RelaySelector`] (scores relay candidates by health +
//! RTT), and [`RelaySession`] (per-peer connectivity-ladder state machine).
//!
//! HOW: Sans-I/O — no sockets, no timers. The mesh layer (feature 04) and the native
//! relay server feed events in; we hand back decisions.

use std::collections::VecDeque;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use super::membership::{PeerId, PeerRecord};

/// Wire version for relay frames. Must be bumped if the format changes.
pub const RELAY_FRAME_VERSION: u8 = 1;

/// Maximum size of a single relay frame's ciphertext payload.
pub const RELAY_MAX_PAYLOAD: usize = 2048;

// ---------------------------------------------------------------------------
// RelayFrame — what the relay forwards
// ---------------------------------------------------------------------------

/// A frame sent through a relay: destination peer id + opaque WireGuard bytes.
///
/// WHY: The relay **never decrypts** — WireGuard crypto is end-to-end between the
/// real peers. The relay only needs the destination id to do a table lookup
/// (decision 07, trustless).
///
/// WHAT: `{ version: u8, dst_peer_id: [u8; 32], payload_len: u16, payload: [u8] }`.
///
/// HOW: Simple length-prefixed binary encoding; no varints, no protobuf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayFrame {
    /// Wire format version.
    pub version: u8,
    /// The destination peer (the peer the relay must forward this to).
    pub dst_peer_id: PeerId,
    /// Opaque WireGuard transport data — **never** decrypted by the relay.
    pub ciphertext: Vec<u8>,
}

impl RelayFrame {
    /// WHY: Every relay client needs to wrap WG packets for forwarding.
    ///
    /// WHAT: Construct a new frame targeting `dst` with `ciphertext`.
    ///
    /// HOW: Stores the fields; `version` is pinned to [`RELAY_FRAME_VERSION`].
    ///
    /// # Panics
    /// Panics if `ciphertext.len() > RELAY_MAX_PAYLOAD`.
    #[must_use]
    pub fn new(dst: PeerId, ciphertext: Vec<u8>) -> Self {
        assert!(
            ciphertext.len() <= RELAY_MAX_PAYLOAD,
            "relay payload {} exceeds max {}",
            ciphertext.len(),
            RELAY_MAX_PAYLOAD
        );
        Self {
            version: RELAY_FRAME_VERSION,
            dst_peer_id: dst,
            ciphertext,
        }
    }

    /// Encode to wire format: `version (1) + dst (32) + len (2, big-endian) + payload`.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + 32 + 2 + self.ciphertext.len());
        buf.push(self.version);
        buf.extend_from_slice(self.dst_peer_id.as_bytes());
        let len = self.ciphertext.len() as u16;
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&self.ciphertext);
        buf
    }

    /// Decode from wire format. Returns `None` if the buffer is too short or version
    /// is unsupported.
    #[must_use]
    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < 35 {
            return None; // version(1) + dst(32) + len(2)
        }
        let version = buf[0];
        if version != RELAY_FRAME_VERSION {
            return None;
        }
        let dst = PeerId(buf[1..33].try_into().ok()?);
        let payload_len = u16::from_be_bytes([buf[33], buf[34]]) as usize;
        if buf.len() < 35 + payload_len {
            return None;
        }
        let ciphertext = buf[35..35 + payload_len].to_vec();
        Some(Self {
            version,
            dst_peer_id: dst,
            ciphertext,
        })
    }
}

// ---------------------------------------------------------------------------
// RelaySelector — picks the best relay from SWIM membership
// ---------------------------------------------------------------------------

/// A candidate relay node drawn from the gossip membership.
#[derive(Debug, Clone)]
pub struct RelayCandidate {
    /// The relay's identity.
    pub peer_id: PeerId,
    /// The relay's advertised endpoint (WG UDP or bootstrap addr).
    pub endpoint: SocketAddr,
    /// Approximate RTT in microseconds, if measured.
    pub rtt_us: Option<u64>,
    /// Advertised relay load (active sessions / max sessions), 0.0–1.0.
    pub load: f64,
}

/// Strategy for relay selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayStrategy {
    /// Pick the relay with the lowest advertised load.
    LowestLoad,
    /// Pick the relay with the lowest measured RTT.
    LowestRtt,
    /// Pick randomly among viable candidates (spread load).
    Random,
}

/// WHY: The mesh may have many relay-capable members; we want the best one.
///
/// WHAT: Scores candidates from the SWIM membership and returns the best one,
/// with failover to the next-best when the current relay degrades.
///
/// HOW: Lexicographic ranking: healthy (Alive) first, then by strategy
/// (load / RTT / random). Candidates are refreshed on membership updates.
pub struct RelaySelector {
    strategy: RelayStrategy,
    candidates: Vec<RelayCandidate>,
    current: Option<PeerId>,
    /// Ring buffer of recent RTT samples for the current relay, for degradation detection.
    recent_rtts: VecDeque<u64>,
}

impl RelaySelector {
    /// Maximum number of RTT samples kept for degradation detection.
    const RTT_WINDOW: usize = 8;

    /// Threshold multiplier: if the current relay's avg RTT exceeds the best
    /// candidate's by this factor, trigger failover.
    const FAILOVER_RTT_FACTOR: f64 = 3.0;

    /// WHY: Select a relay strategy: lowest load, lowest RTT, or random.
    ///
    /// WHAT: Construct a new selector with the given strategy, initially empty.
    #[must_use]
    pub fn new(strategy: RelayStrategy) -> Self {
        Self {
            strategy,
            candidates: Vec::new(),
            current: None,
            recent_rtts: VecDeque::with_capacity(Self::RTT_WINDOW),
        }
    }

    /// WHY: The SWIM membership changed; rebuild the candidate pool.
    ///
    /// WHAT: Scan `members` for `Alive` peers with `caps.relay` enabled, extract
    /// their first advertised endpoint as the relay endpoint, and sort by the current
    /// strategy.
    ///
    /// HOW: Relays must have at least one advertised endpoint. If a candidate is the
    /// current selection, preserve its RTT history for degradation detection.
    pub fn update(&mut self, members: &[PeerRecord]) {
        self.candidates = members
            .iter()
            .filter(|r| r.caps.relay && r.state == super::membership::MemberState::Alive)
            .filter_map(|r| {
                r.endpoints.first().map(|ep| RelayCandidate {
                    peer_id: r.id,
                    endpoint: *ep,
                    rtt_us: None,
                    load: 0.0,
                })
            })
            .collect();

        // Sort: by strategy (load ascending, RTT ascending, or shuffle).
        match self.strategy {
            RelayStrategy::LowestLoad => {
                self.candidates.sort_by(|a, b| a.load.partial_cmp(&b.load).unwrap_or(std::cmp::Ordering::Equal));
            }
            RelayStrategy::LowestRtt => {
                self.candidates.sort_by_key(|c| c.rtt_us.unwrap_or(u64::MAX));
            }
            RelayStrategy::Random => {
                // Deterministic-ish shuffle: sort by peer_id bytes for reproducibility.
                self.candidates.sort_by_key(|c| c.peer_id.0);
            }
        }
    }

    /// WHY: The mesh needs one relay, not all of them.
    ///
    /// WHAT: Return the best candidate (first in the sorted list).
    ///
    /// HOW: If a relay is already selected and still viable, keep it unless
    /// degradation is detected. Otherwise pick the top candidate.
    #[must_use]
    pub fn select(&mut self) -> Option<&RelayCandidate> {
        // If we have a current relay and it's still in the list, check for degradation.
        if let Some(current_id) = self.current {
            if let Some(pos) = self.candidates.iter().position(|c| c.peer_id == current_id) {
                // Degradation check: if first candidate is much better, fail over.
                if pos > 0 {
                    if let (Some(best_rtt), Some(cur_rtt)) =
                        (self.candidates[0].rtt_us, self.candidates[pos].rtt_us)
                    {
                        if cur_rtt as f64 > best_rtt as f64 * Self::FAILOVER_RTT_FACTOR {
                            self.current = Some(self.candidates[0].peer_id);
                            return self.candidates.first();
                        }
                    }
                }
                return Some(&self.candidates[pos]);
            }
            // Current relay disappeared; pick the best.
        }
        self.current = self.candidates.first().map(|c| c.peer_id);
        self.candidates.first()
    }

    /// WHY: When a relay fails, quickly switch to the next one.
    ///
    /// WHAT: Remove the current relay from the candidate pool and select the next.
    #[must_use]
    pub fn failover(&mut self) -> Option<&RelayCandidate> {
        if let Some(id) = self.current.take() {
            self.candidates.retain(|c| c.peer_id != id);
        }
        self.select()
    }

    /// Record an RTT measurement for the supplied peer (if it is the current relay).
    pub fn record_rtt(&mut self, peer: PeerId, rtt_us: u64) {
        if self.current == Some(peer) {
            self.recent_rtts.push_back(rtt_us);
            if self.recent_rtts.len() > Self::RTT_WINDOW {
                self.recent_rtts.pop_front();
            }
            // Update the candidate's RTT.
            if let Some(c) = self.candidates.iter_mut().find(|c| c.peer_id == peer) {
                let avg: u64 = self.recent_rtts.iter().sum::<u64>()
                    / self.recent_rtts.len().max(1) as u64;
                c.rtt_us = Some(avg);
            }
        }
    }

    /// The currently selected relay, if any.
    #[must_use]
    pub fn current(&self) -> Option<PeerId> {
        self.current
    }

    /// Number of viable relay candidates.
    #[must_use]
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// Whether there are any candidates.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Connectivity-ladder state machine
// ---------------------------------------------------------------------------

/// The current connectivity path to a peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectivityPath {
    /// Direct WireGuard tunnel over the peer's advertised UDP endpoint.
    Direct,
    /// Attempting UDP hole-punch; waiting for the simultaneous-open to complete.
    HolePunching,
    /// Traffic is being relayed through a third member.
    Relayed(PeerId),
    /// No path established yet.
    Disconnected,
}

/// Per-peer session tracking the connectivity ladder (decision 07).
///
/// WHY: The mesh doesn't just pick direct-or-relay; it tries direct first,
/// attempts hole-punch when direct fails, and falls back to relay last.
///
/// WHAT: A small state machine per peer that records the current path and any
/// in-flight transition.
///
/// HOW: The mesh driver calls `try_upgrade` periodically and `on_probe_failure`
/// when direct connectivity drops; the session decides the next state.
#[derive(Debug, Clone)]
pub struct RelaySession {
    /// Current connectivity path.
    pub path: ConnectivityPath,
    /// The relay peer selected for this session (None if direct works).
    pub relay: Option<PeerId>,
}

impl RelaySession {
    /// WHY: Start fresh with no path.
    #[must_use]
    pub fn new() -> Self {
        Self {
            path: ConnectivityPath::Disconnected,
            relay: None,
        }
    }

    /// WHY: A direct probe succeeded — upgrade from relayed/disconnected.
    ///
    /// WHAT: Set the path to `Direct`, clear the relay reference.
    pub fn on_direct_success(&mut self) {
        self.path = ConnectivityPath::Direct;
        self.relay = None;
    }

    /// WHY: Direct connectivity failed — move down the ladder.
    ///
    /// WHAT: Transition from `Direct` → `HolePunching` if endpoints are available,
    /// otherwise → `Relayed`. If already `Relayed`, stay there.
    pub fn on_direct_failure(&mut self, relay: Option<PeerId>) {
        match self.path {
            ConnectivityPath::Direct => {
                // Try hole-punch next; if we have a relay, pre-select it.
                self.path = ConnectivityPath::HolePunching;
                self.relay = relay;
            }
            ConnectivityPath::HolePunching => {
                // Hole-punch failed; fall back to relay.
                let r = relay.expect("relay peer id required for relay fallback");
                self.path = ConnectivityPath::Relayed(r);
                self.relay = Some(r);
            }
            ConnectivityPath::Disconnected => {
                // Not yet connected — go straight to relay if we have one.
                if let Some(r) = relay {
                    self.path = ConnectivityPath::Relayed(r);
                    self.relay = Some(r);
                }
            }
            ConnectivityPath::Relayed(_) => {
                // Already relayed; update the relay if a new one is provided.
                if let Some(r) = relay {
                    self.path = ConnectivityPath::Relayed(r);
                    self.relay = Some(r);
                }
            }
        }
    }

    /// WHY: Hole-punch succeeded — upgrade back to direct.
    ///
    /// WHAT: Set the path to `Direct`, clear the relay reference.
    pub fn on_hole_punch_success(&mut self) {
        self.path = ConnectivityPath::Direct;
        self.relay = None;
    }

    /// Whether traffic should currently flow through a relay.
    #[must_use]
    pub fn is_relayed(&self) -> bool {
        matches!(self.path, ConnectivityPath::Relayed(_))
    }

    /// The relay peer id, if currently relayed.
    #[must_use]
    pub fn relay_peer(&self) -> Option<PeerId> {
        self.relay
    }
}

impl Default for RelaySession {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::membership::{Capabilities, MemberState};
    use std::net::{IpAddr, Ipv4Addr};

    fn peer(id: u8, relay: bool, ip_octet: u8) -> PeerRecord {
        let mut caps = Capabilities::default();
        caps.relay = relay;
        PeerRecord {
            id: PeerId([id; 32]),
            tunnel_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, ip_octet)),
            endpoints: vec![SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                51000 + id as u16,
            )],
            caps,
            incarnation: 1,
            state: MemberState::Alive,
            heartbeat: 1,
        }
    }

    #[test]
    fn relay_frame_encode_decode_round_trip() {
        let frame = RelayFrame::new(
            PeerId([0xAB; 32]),
            vec![1, 2, 3, 4],
        );
        let encoded = frame.encode();
        let decoded = RelayFrame::decode(&encoded).expect("decode");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn relay_frame_decode_short_buffer_returns_none() {
        assert!(RelayFrame::decode(&[0; 10]).is_none());
    }

    #[test]
    fn relay_frame_decode_wrong_version_returns_none() {
        let frame = RelayFrame::new(PeerId([0xCD; 32]), vec![5, 6]);
        let mut encoded = frame.encode();
        encoded[0] = 99; // wrong version
        assert!(RelayFrame::decode(&encoded).is_none());
    }

    #[test]
    fn relay_frame_reject_oversize_payload() {
        let huge = vec![0u8; RELAY_MAX_PAYLOAD + 1];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            RelayFrame::new(PeerId([0; 32]), huge);
        }));
        assert!(result.is_err());
    }

    #[test]
    fn selector_picks_only_relay_capable_peers() {
        let records = vec![
            peer(1, true, 1),
            peer(2, false, 2),
            peer(3, true, 3),
        ];
        let mut sel = RelaySelector::new(RelayStrategy::LowestLoad);
        sel.update(&records);
        assert_eq!(sel.len(), 2);
    }

    #[test]
    fn selector_filters_dead_peers() {
        let mut dead = peer(1, true, 1);
        dead.state = MemberState::Dead;
        let records = vec![dead, peer(2, true, 2)];
        let mut sel = RelaySelector::new(RelayStrategy::LowestLoad);
        sel.update(&records);
        assert_eq!(sel.len(), 1);
    }

    #[test]
    fn selector_failover_removes_current() {
        let records = vec![peer(1, true, 1), peer(2, true, 2)];
        let mut sel = RelaySelector::new(RelayStrategy::LowestLoad);
        sel.update(&records);
        let first = sel.select().expect("first").peer_id;
        let second = sel.failover().expect("second").peer_id;
        assert_ne!(first, second);
        assert_eq!(sel.len(), 1);
    }

    #[test]
    fn selector_empty_when_no_relays() {
        let records = vec![peer(1, false, 1)];
        let mut sel = RelaySelector::new(RelayStrategy::LowestLoad);
        sel.update(&records);
        assert!(sel.is_empty());
        assert!(sel.select().is_none());
    }

    #[test]
    fn session_ladder_direct_to_relayed() {
        let mut s = RelaySession::new();
        assert!(!s.is_relayed());

        // Establish direct connection first.
        s.on_direct_success();
        assert_eq!(s.path, ConnectivityPath::Direct);

        // Direct fails — move to hole-punch.
        let relay = PeerId([0xBB; 32]);
        s.on_direct_failure(Some(relay));
        assert_eq!(s.path, ConnectivityPath::HolePunching);

        // Hole-punch fails — fall back to relay.
        s.on_direct_failure(Some(relay));
        assert!(s.is_relayed());
        assert_eq!(s.relay_peer(), Some(relay));
    }

    #[test]
    fn session_upgrades_from_relay_to_direct() {
        let mut s = RelaySession::new();
        let relay = PeerId([0xBB; 32]);
        // From Disconnected with a relay, go straight to relayed.
        s.on_direct_failure(Some(relay));
        assert!(s.is_relayed());

        // Then upgrade to direct.
        s.on_direct_success();
        assert!(!s.is_relayed());
        assert_eq!(s.relay, None);
    }

    #[test]
    fn session_hole_punch_success_goes_direct() {
        let mut s = RelaySession::new();
        let relay = PeerId([0xBB; 32]);
        // Start from Direct, then first failure goes to HolePunching.
        s.on_direct_success();
        s.on_direct_failure(Some(relay));
        assert_eq!(s.path, ConnectivityPath::HolePunching);

        // Hole-punch succeeds — back to direct.
        s.on_hole_punch_success();
        assert_eq!(s.path, ConnectivityPath::Direct);
        assert!(!s.is_relayed());
    }
}

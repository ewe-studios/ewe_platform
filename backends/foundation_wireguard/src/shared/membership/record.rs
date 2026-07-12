//! Membership records — the gossiped unit (spec-55, feature 03; decision 05).
//!
//! WHY: Full-membership gossip means every node holds, for every peer, exactly what
//! WireGuard configuration needs: identity key, overlay IP, and current endpoint(s) —
//! plus the SWIM bookkeeping (incarnation, state, heartbeat) that makes merges a CRDT.
//!
//! WHAT: [`PeerId`], [`Capabilities`], [`MemberState`], and [`PeerRecord`].
//!
//! HOW: Records are plain serializable data (no timers held inside), so they gossip and
//! merge deterministically; suspicion deadlines live in the [`Swim`](super::swim::Swim)
//! state machine, not the record.

use std::net::{IpAddr, SocketAddr};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

/// A node identity: its post-handoff x25519 public key. This is the membership map key.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PeerId(pub [u8; 32]);

impl PeerId {
    /// The raw 32-byte identity key.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// WireGuard-style base64 rendering.
    #[must_use]
    pub fn to_base64(&self) -> String {
        B64.encode(self.0)
    }
}

impl std::fmt::Debug for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Short prefix keeps logs readable.
        write!(f, "PeerId({}…)", &self.to_base64()[..8.min(self.to_base64().len())])
    }
}

impl std::fmt::Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_base64())
    }
}

/// Advertised node capabilities (decision 07). Extensible; unknown future bits are simply
/// carried as additional booleans as the struct grows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// This node can relay opaque ciphertext for others (DERP-style).
    pub relay: bool,
    /// This node can act as an overlay→internet gateway.
    pub gateway: bool,
    /// This node has IPv6 reachability.
    pub ipv6: bool,
}

/// SWIM liveness state for a member.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberState {
    /// Reachable and healthy.
    Alive,
    /// Failed a probe; awaiting refutation before being declared dead.
    Suspect,
    /// Declared dead; tombstoned with a TTL until garbage-collected.
    Dead,
}

impl MemberState {
    /// Severity rank used to break ties at equal incarnation (`Dead` > `Suspect` > `Alive`).
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            MemberState::Alive => 0,
            MemberState::Suspect => 1,
            MemberState::Dead => 2,
        }
    }
}

/// One peer's full membership record — the unit that gossips and merges.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerRecord {
    /// Node identity (post-handoff public key); the map key.
    pub id: PeerId,
    /// Overlay (tunnel) address — WireGuard `allowed_ips` (decision 12).
    pub tunnel_ip: IpAddr,
    /// Observed reachable UDP endpoints (last-writer-wins).
    pub endpoints: Vec<SocketAddr>,
    /// Advertised capabilities (decision 07).
    pub caps: Capabilities,
    /// Owner-incremented refutation counter (SWIM incarnation).
    pub incarnation: u64,
    /// Liveness state.
    pub state: MemberState,
    /// Monotonic heartbeat, for last-writer-wins on endpoints/caps at equal incarnation.
    pub heartbeat: u64,
}

impl PeerRecord {
    /// WHY: A node describes itself before announcing to the mesh.
    ///
    /// WHAT: Build an `Alive` record at incarnation 0, heartbeat 0.
    ///
    /// HOW: Stores the given identity/address/endpoints/caps.
    #[must_use]
    pub fn new(
        id: PeerId,
        tunnel_ip: IpAddr,
        endpoints: Vec<SocketAddr>,
        caps: Capabilities,
    ) -> Self {
        Self {
            id,
            tunnel_ip,
            endpoints,
            caps,
            incarnation: 0,
            state: MemberState::Alive,
            heartbeat: 0,
        }
    }

    /// WHY: The CRDT merge must decide, deterministically, whether `other` supersedes
    /// `self` — independent of arrival order.
    ///
    /// WHAT: Return `true` if `other` should replace `self`.
    ///
    /// HOW: Higher `incarnation` wins; at equal incarnation a more severe state wins
    /// (`Dead` > `Suspect` > `Alive`); at equal incarnation and state, a higher
    /// `heartbeat` wins (LWW on endpoints/caps). Deterministic and total.
    #[must_use]
    pub fn supersedes(&self, other: &PeerRecord) -> bool {
        if other.incarnation != self.incarnation {
            return other.incarnation > self.incarnation;
        }
        if other.state.rank() != self.state.rank() {
            return other.state.rank() > self.state.rank();
        }
        other.heartbeat > self.heartbeat
    }
}

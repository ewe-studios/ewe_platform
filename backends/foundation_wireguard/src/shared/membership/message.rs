//! SWIM wire messages + a compact, versioned codec (spec-55, feature 03).
//!
//! WHY: The mesh transports SWIM over the TLS-PSK join channel (bootstrap) and the
//! in-tunnel UDP channel (steady state); both need a compact, versioned encoding.
//!
//! WHAT: [`SwimMessage`] (the payloads), [`SwimOutbound`] (a message addressed to a peer),
//! [`DigestEntry`] (anti-entropy summary), and [`encode`]/[`decode`].
//!
//! HOW: `bincode` for compactness, prefixed with a single version byte so future changes
//! are detectable.

use serde::{Deserialize, Serialize};

use super::record::{PeerId, PeerRecord};
use crate::shared::error::{WgError, WgResult};

/// Wire-format version byte prefixed to every encoded [`SwimMessage`].
pub const SWIM_WIRE_VERSION: u8 = 1;

/// One entry in an anti-entropy digest: what version of a peer the sender holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DigestEntry {
    /// The peer this entry summarises.
    pub id: PeerId,
    /// The sender's incarnation for that peer.
    pub incarnation: u64,
    /// The sender's heartbeat for that peer.
    pub heartbeat: u64,
    /// The sender's state rank for that peer (0 alive, 1 suspect, 2 dead).
    pub state_rank: u8,
}

/// A SWIM protocol message payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwimMessage {
    /// Direct liveness probe.
    Ping {
        /// Probe sequence number (echoed in the ack).
        seq: u64,
    },
    /// Acknowledgement of a `Ping` (direct or forwarded on behalf of a `PingReq`).
    Ack {
        /// The acknowledged probe sequence number.
        seq: u64,
    },
    /// Ask the recipient to probe `target` on the sender's behalf (indirect probe).
    PingReq {
        /// Probe sequence number to use and echo back.
        seq: u64,
        /// The peer to probe.
        target: PeerId,
    },
    /// Epidemic membership updates (new members, refutations, state changes).
    Gossip {
        /// The records being spread.
        updates: Vec<PeerRecord>,
    },
    /// Anti-entropy: "here is what I hold; send me anything newer".
    SyncDigest {
        /// Summary of every member the sender knows.
        entries: Vec<DigestEntry>,
    },
    /// Anti-entropy reply: records the requester was missing or holds stale.
    SyncResponse {
        /// Fresher records for the requester to merge.
        updates: Vec<PeerRecord>,
    },
}

/// A [`SwimMessage`] addressed to a specific peer. The mesh layer maps `to` onto a
/// concrete transport endpoint (TLS-PSK channel or in-tunnel UDP address).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwimOutbound {
    /// The intended recipient.
    pub to: PeerId,
    /// The message to deliver.
    pub message: SwimMessage,
}

impl SwimOutbound {
    /// Construct an outbound message.
    #[must_use]
    pub fn new(to: PeerId, message: SwimMessage) -> Self {
        Self { to, message }
    }
}

/// WHY: The transport carries opaque bytes; the codec bounds the format.
///
/// WHAT: Encode a message as `[version][bincode(message)]`.
///
/// HOW: `bincode` standard config, version-prefixed.
///
/// # Panics
/// Never panics.
#[must_use]
pub fn encode(message: &SwimMessage) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    out.push(SWIM_WIRE_VERSION);
    let body = bincode::serde::encode_to_vec(message, bincode::config::standard())
        .expect("SwimMessage is always serializable");
    out.extend_from_slice(&body);
    out
}

/// WHY: Inbound bytes must be validated before dispatch.
///
/// WHAT: Decode a version-prefixed [`SwimMessage`].
///
/// HOW: Checks the version byte, then `bincode`-decodes the remainder.
///
/// # Errors
/// [`WgError::Protocol`] on an empty buffer, a version mismatch, or a malformed body.
pub fn decode(bytes: &[u8]) -> WgResult<SwimMessage> {
    let (&version, body) = bytes
        .split_first()
        .ok_or_else(|| WgError::Protocol("empty SWIM message".into()))?;
    if version != SWIM_WIRE_VERSION {
        return Err(WgError::Protocol(format!(
            "unsupported SWIM wire version {version}"
        )));
    }
    let (message, _len) =
        bincode::serde::decode_from_slice(body, bincode::config::standard())
            .map_err(|e| WgError::Protocol(format!("malformed SWIM message: {e}")))?;
    Ok(message)
}

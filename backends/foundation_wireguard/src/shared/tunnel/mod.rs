//! Sans-I/O per-peer WireGuard tunnel wrapper (spec-55, feature 01).
//!
//! WHY: `boringtun::noise::Tunn` is a pure crypto state machine with no I/O and a
//! lifetime-bound output buffer. We wrap it so callers get owned, ergonomic outcomes and
//! the buffer management is internal — keeping the driver loop clean.
//!
//! WHAT: [`WgTunnel`] wraps exactly one `Tunn` (one peer). Its methods mirror
//! `encapsulate` / `decapsulate` / `update_timers`, returning an owned [`WgOutcome`].
//!
//! HOW: Each call runs `Tunn` against an internal scratch buffer and copies the relevant
//! slice out. The documented "repeat `decapsulate` with an empty datagram until `Done`"
//! contract is exposed via [`WgTunnel::flush_network`].

use std::net::IpAddr;

use boringtun::noise::{Tunn, TunnResult};
use boringtun::x25519::{PublicKey, StaticSecret};

/// Scratch buffer size — large enough for any IP packet plus WireGuard overhead
/// (boringtun requires `dst >= src + 32` and `>= 148`).
const SCRATCH: usize = 65_535;

/// The owned result of driving a [`WgTunnel`] one step.
#[derive(Clone, PartialEq, Eq)]
pub enum WgOutcome {
    /// Nothing to do.
    Done,
    /// Ciphertext to `send_to` the peer's UDP endpoint.
    WriteToNetwork(Vec<u8>),
    /// A decrypted inbound IP packet to feed the data plane (`inject_inbound_ip`).
    WriteToTunnel(Vec<u8>),
    /// The Noise layer reported an error (non-fatal; the tunnel keeps running).
    Error(String),
}

impl std::fmt::Debug for WgOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WgOutcome::Done => write!(f, "Done"),
            WgOutcome::WriteToNetwork(b) => write!(f, "WriteToNetwork({} bytes)", b.len()),
            WgOutcome::WriteToTunnel(b) => write!(f, "WriteToTunnel({} bytes)", b.len()),
            WgOutcome::Error(e) => write!(f, "Error({e})"),
        }
    }
}

/// A single WireGuard tunnel to one peer.
pub struct WgTunnel {
    tunn: Tunn,
    peer_public: PublicKey,
    scratch: Vec<u8>,
}

impl WgTunnel {
    /// WHY: Each peer relationship is one Noise session.
    ///
    /// WHAT: Construct a tunnel from our static secret, the peer's public key, an optional
    /// preshared key, and a local index.
    ///
    /// HOW: Delegates to `Tunn::new` (infallible since boringtun 0.7.0) with no rate
    /// limiter.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn new(
        static_secret: StaticSecret,
        peer_public: PublicKey,
        preshared_key: Option<[u8; 32]>,
        persistent_keepalive: Option<u16>,
        index: u32,
    ) -> Self {
        let tunn = Tunn::new(
            static_secret,
            peer_public,
            preshared_key,
            persistent_keepalive,
            index,
            None,
        );
        Self {
            tunn,
            peer_public,
            scratch: vec![0u8; SCRATCH],
        }
    }

    /// WHY: Outbound app traffic (a raw IP packet from the data plane) must be encrypted.
    ///
    /// WHAT: Encapsulate one IP packet, yielding ciphertext to send (or a handshake
    /// initiation if no session is established yet).
    ///
    /// HOW: Runs `Tunn::encapsulate` against the scratch buffer.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn encapsulate(&mut self, ip_packet: &[u8]) -> WgOutcome {
        let result = self.tunn.encapsulate(ip_packet, &mut self.scratch);
        map_result(result)
    }

    /// WHY: Inbound ciphertext from the peer must be decrypted (or drive the handshake).
    ///
    /// WHAT: Decapsulate one UDP datagram from `src`, yielding a plaintext IP packet, a
    /// handshake response to send, or `Done`.
    ///
    /// HOW: Runs `Tunn::decapsulate`. After a [`WgOutcome::WriteToNetwork`] result the
    /// caller must drain queued packets via [`Self::flush_network`].
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn decapsulate(&mut self, src: Option<IpAddr>, datagram: &[u8]) -> WgOutcome {
        let result = self.tunn.decapsulate(src, datagram, &mut self.scratch);
        map_result(result)
    }

    /// WHY: boringtun queues packets during a handshake; after a network write from
    /// `decapsulate` the caller must poll with empty datagrams until drained.
    ///
    /// WHAT: Return the next queued ciphertext to send, or `None` when drained.
    ///
    /// HOW: Calls `decapsulate(None, &[])`; only a `WriteToNetwork` yields `Some`.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn flush_network(&mut self) -> Option<Vec<u8>> {
        match self.tunn.decapsulate(None, &[], &mut self.scratch) {
            TunnResult::WriteToNetwork(packet) => Some(packet.to_vec()),
            _ => None,
        }
    }

    /// WHY: WireGuard drives handshakes, keepalives, and rekeys off timers.
    ///
    /// WHAT: Advance this tunnel's timers, possibly yielding a keepalive/handshake packet.
    ///
    /// HOW: Runs `Tunn::update_timers` against the scratch buffer.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn update_timers(&mut self) -> WgOutcome {
        let result = self.tunn.update_timers(&mut self.scratch);
        map_result(result)
    }

    /// The peer's static public key.
    #[must_use]
    pub fn peer_public(&self) -> PublicKey {
        self.peer_public
    }
}

impl std::fmt::Debug for WgTunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgTunnel")
            .field("peer_public", &base64_public(&self.peer_public))
            .finish_non_exhaustive()
    }
}

/// Copy a borrowed `TunnResult` into an owned [`WgOutcome`].
fn map_result(result: TunnResult<'_>) -> WgOutcome {
    match result {
        TunnResult::Done => WgOutcome::Done,
        TunnResult::Err(err) => WgOutcome::Error(format!("{err:?}")),
        TunnResult::WriteToNetwork(packet) => WgOutcome::WriteToNetwork(packet.to_vec()),
        TunnResult::WriteToTunnelV4(packet, _addr) => WgOutcome::WriteToTunnel(packet.to_vec()),
        TunnResult::WriteToTunnelV6(packet, _addr) => WgOutcome::WriteToTunnel(packet.to_vec()),
    }
}

fn base64_public(key: &PublicKey) -> String {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    STANDARD.encode(key.as_bytes())
}

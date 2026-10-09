//! Bootstrap join-protocol messages (spec-55, feature 02; decision 06/11).
//!
//! WHY: Over the TLS-PSK channel a joiner runs `Join`/`PullMembership`/`Announce` to be
//! admitted and to receive the full membership. The message *types* are transport- and
//! platform-agnostic (native TLS-PSK today, wss in the browser at feature 07), so they
//! live in `shared`.
//!
//! WHAT: [`BootstrapRequest`], [`BootstrapResponse`], [`Admission`], and a compact
//! version-prefixed codec ([`encode_request`]/[`decode_request`] and the response pair).
//!
//! HOW: `bincode` behind a single version byte, mirroring the SWIM wire codec.

use serde::{Deserialize, Serialize};

use crate::shared::error::{WgError, WgResult};
use crate::shared::membership::PeerRecord;

/// Wire-format version byte for the bootstrap join protocol.
pub const BOOTSTRAP_WIRE_VERSION: u8 = 1;

/// A joiner→member request over the TLS-PSK channel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootstrapRequest {
    /// Announce self and request admission; carries the joiner's full record.
    Join(PeerRecord),
    /// Request the full membership set.
    PullMembership,
    /// Re-announce a (possibly updated) record for gossip fan-out.
    Announce(PeerRecord),
}

/// A member→joiner response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootstrapResponse {
    /// Result of a `Join`: the admission decision plus the current membership.
    JoinResult {
        /// Whether the joiner is admitted.
        decision: Admission,
        /// The full membership set (empty on rejection).
        members: Vec<PeerRecord>,
    },
    /// The full membership set (response to `PullMembership`).
    Membership(Vec<PeerRecord>),
    /// Acknowledgement (response to `Announce`).
    Ack,
}

/// Admission decision (decision 11).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Admission {
    /// The joiner is admitted into the mesh.
    Admit,
    /// The joiner is pending operator approval.
    Pending,
    /// The joiner is refused (expired/revoked seed or closed network).
    Reject,
}

/// Encode a request as `[version][bincode]`.
///
/// # Panics
/// Never panics.
#[must_use]
pub fn encode_request(request: &BootstrapRequest) -> Vec<u8> {
    encode(BOOTSTRAP_WIRE_VERSION, request)
}

/// Encode a response as `[version][bincode]`.
///
/// # Panics
/// Never panics.
#[must_use]
pub fn encode_response(response: &BootstrapResponse) -> Vec<u8> {
    encode(BOOTSTRAP_WIRE_VERSION, response)
}

/// Decode a version-prefixed request.
///
/// # Errors
/// [`WgError::Protocol`] on empty input, version mismatch, or a malformed body.
pub fn decode_request(bytes: &[u8]) -> WgResult<BootstrapRequest> {
    decode(bytes)
}

/// Decode a version-prefixed response.
///
/// # Errors
/// [`WgError::Protocol`] on empty input, version mismatch, or a malformed body.
pub fn decode_response(bytes: &[u8]) -> WgResult<BootstrapResponse> {
    decode(bytes)
}

fn encode<T: Serialize>(version: u8, value: &T) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    out.push(version);
    let body = bincode::serde::encode_to_vec(value, bincode::config::standard())
        .expect("bootstrap message is serializable");
    out.extend_from_slice(&body);
    out
}

fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> WgResult<T> {
    let (&version, body) = bytes
        .split_first()
        .ok_or_else(|| WgError::Protocol("empty bootstrap message".into()))?;
    if version != BOOTSTRAP_WIRE_VERSION {
        return Err(WgError::Protocol(format!(
            "unsupported bootstrap wire version {version}"
        )));
    }
    let (value, _) = bincode::serde::decode_from_slice(body, bincode::config::standard())
        .map_err(|e| WgError::Protocol(format!("malformed bootstrap message: {e}")))?;
    Ok(value)
}

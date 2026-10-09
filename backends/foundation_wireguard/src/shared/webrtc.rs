//! WebRTC signaling types — shared between native answerer and browser offerer
//! (spec-55, F07; decision 08).
//!
//! WHY: WebRTC signaling (SDP offer/answer + ICE candidates) is carried over
//! the existing SWIM gossip channel. No separate signaling server — the mesh
//! IS the signaling plane. These types are the message format.
//!
//! WHAT: `WebRtcSignal` enum (Offer/Answer/ICECandidate), serde-encoded into
//! `SwimMessage::WebRtcSignal { data }`. The offerer (browser) and answerer
//! (native) both use these types.

use serde::{Deserialize, Serialize};

/// A WebRTC signaling message carried over SWIM gossip.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebRtcSignal {
    /// An SDP offer from the browser to a native peer.
    Offer { sdp: String },
    /// An SDP answer from the native peer back to the browser.
    Answer { sdp: String },
    /// An ICE candidate from either side.
    IceCandidate { candidate: String, sdp_mid: Option<String>, sdp_mline_index: Option<u16> },
}

impl WebRtcSignal {
    /// Serialize to bytes for embedding in SwimMessage::WebRtcSignal.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("WebRtcSignal encode")
    }

    /// Deserialize from the SwimMessage payload.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        serde_json::from_slice(data).ok()
    }
}

/// The state of a WebRTC connection from the browser (offerer) perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffererState {
    /// Not yet started.
    Idle,
    /// SDP offer sent via gossip to the target native peer.
    OfferSent,
    /// Waiting for the answer.
    WaitingForAnswer,
    /// SDP answer received; ICE candidate exchange in progress.
    IceExchange,
    /// Data channel established — WebRTC transport is active.
    Connected,
    /// Failed or timed out.
    Failed,
}

/// The state of a WebRTC connection from the native (answerer) perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswererState {
    /// Waiting for an offer.
    Idle,
    /// Received offer, sent answer via gossip.
    Answered,
    /// ICE candidate exchange in progress.
    IceExchange,
    /// Data channel established.
    Connected,
    /// Failed or timed out.
    Failed,
}

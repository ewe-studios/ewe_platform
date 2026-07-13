//! WebRTC answerer — native side (spec-55, F07).
//!
//! WHY: When a browser peer offers a WebRTC data channel, a native peer on the
//! mesh must answer it — terminating ICE, DTLS, and SCTP.
//!
//! WHAT: [`WebRtcAnswerer`] — receives an SDP offer (via gossip), produces an
//! SDP answer, and manages the data channel lifecycle.
//!
//! HOW: The signaling messages (offer, answer, ICE candidates) are carried over
//! the same gossip RPC used for membership — no separate signaling server.
//! A production implementation would integrate a sans-I/O ICE/DTLS/SCTP stack;
//! this module provides the signaling protocol and answerer state machine.

use crate::shared::membership::PeerId;

/// State of a WebRTC answer session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswererState {
    /// Waiting for an SDP offer from a browser peer.
    Idle,
    /// Received an offer, generated an answer (sent via gossip).
    Answered,
    /// The data channel is open.
    Connected,
    /// The connection failed or timed out.
    Failed(String),
}

/// A native-side WebRTC answerer.
///
/// WHY: The native peer terminates the browser's WebRTC connection and exposes
/// a data channel that replaces the relay as the WG packet transport.
///
/// WHAT: State machine that processes incoming SDP offers, produces answers,
/// and tracks connection lifecycle.
pub struct WebRtcAnswerer {
    /// Current state.
    state: AnswererState,
    /// The browser peer we're answering.
    peer: Option<PeerId>,
    /// The SDP offer from the browser.
    offer_sdp: Option<String>,
    /// The SDP answer we generated.
    answer_sdp: Option<String>,
}

impl WebRtcAnswerer {
    /// Create a new answerer in Idle state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: AnswererState::Idle,
            peer: None,
            offer_sdp: None,
            answer_sdp: None,
        }
    }

    /// WHY: A browser peer sent an SDP offer via gossip.
    ///
    /// WHAT: Parse and store the offer, transition to `Answered` state.
    /// The caller should send the generated answer back via gossip.
    pub fn on_offer(&mut self, peer: PeerId, sdp: String) -> Option<String> {
        self.peer = Some(peer);
        self.offer_sdp = Some(sdp);

        // In a full implementation, this would set up ICE/DTLS/SCTP.
        // For now, generate a placeholder answer.
        let answer = format!(
            "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n\
             a=group:BUNDLE data\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n\
             c=IN IP4 0.0.0.0\r\na=mid:data\r\n"
        );
        self.answer_sdp = Some(answer.clone());
        self.state = AnswererState::Answered;
        Some(answer)
    }

    /// WHY: The data channel is established and ready for WG traffic.
    pub fn on_connected(&mut self) {
        self.state = AnswererState::Connected;
    }

    /// WHY: Connection failed.
    pub fn on_failed(&mut self, reason: String) {
        self.state = AnswererState::Failed(reason);
    }

    /// Current answerer state.
    #[must_use]
    pub fn state(&self) -> &AnswererState {
        &self.state
    }

    /// Whether the data channel is connected.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        self.state == AnswererState::Connected
    }

    /// The browser peer being answered.
    #[must_use]
    pub fn peer(&self) -> Option<PeerId> {
        self.peer
    }

    /// The generated SDP answer to send via gossip.
    #[must_use]
    pub fn answer_sdp(&self) -> Option<&str> {
        self.answer_sdp.as_deref()
    }
}

impl Default for WebRtcAnswerer {
    fn default() -> Self {
        Self::new()
    }
}


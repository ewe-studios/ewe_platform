//! WebRTC answerer state machine tests (spec-55, F07).
//!
//! Tests the native-side WebRTC answerer: SDP offer processing, state
//! transitions, and failure handling. These were moved from inline tests
//! in native/webrtc.rs per the project convention of tests in tests/.

#![cfg(not(target_family = "wasm"))]

use foundation_wireguard::native::webrtc::{AnswererState, WebRtcAnswerer};
use foundation_wireguard::shared::membership::PeerId;
use tracing_test::traced_test;

#[traced_test]
#[test]
fn answerer_processes_offer_and_transitions() {
    let mut answerer = WebRtcAnswerer::new();
    assert_eq!(*answerer.state(), AnswererState::Idle);

    let peer = PeerId([0xCC; 32]);
    let answer = answerer.on_offer(peer, "fake sdp offer".into());
    assert!(answer.is_some());
    assert_eq!(*answerer.state(), AnswererState::Answered);
    assert_eq!(answerer.peer(), Some(peer));

    answerer.on_connected();
    assert!(answerer.is_connected());
}

#[traced_test]
#[test]
fn answerer_failure() {
    let mut answerer = WebRtcAnswerer::new();
    answerer.on_offer(PeerId([0xDD; 32]), "offer".into());
    answerer.on_failed("timeout".into());
    assert!(matches!(
        answerer.state(),
        AnswererState::Failed(ref r) if r == "timeout"
    ));
}

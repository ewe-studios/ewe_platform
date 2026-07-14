//! WebRTC signaling tests (spec-55, F07).
//!
//! Tests the full WebRTC signaling flow: browser offer → SDP answer →
//! ICE candidate exchange → connected state. Signaling is carried over
//! SWIM gossip via SwimMessage::WebRtcSignal.

#![cfg(not(target_family = "wasm"))]

use foundation_wireguard::native::webrtc::WebRtcAnswerer;
use foundation_wireguard::shared::membership::PeerId;
use foundation_wireguard::shared::webrtc::{AnswererState, OffererState, WebRtcSignal};
use foundation_wireguard::wasm::webrtc::WebRtcOfferer;
use tracing_test::traced_test;

fn test_peer() -> PeerId {
    PeerId([0xCC; 32])
}

// ── Offerer tests ──

#[traced_test]
#[test]
fn offerer_initiates_and_processes_answer() {
    let mut offerer = WebRtcOfferer::new();
    assert_eq!(offerer.state, OffererState::Idle);

    let sdp = "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\na=ice-ufrag:browser\r\na=ice-pwd:browser-pwd\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n".to_string();
    offerer.initiate(test_peer(), sdp.clone());
    assert_eq!(offerer.state, OffererState::OfferSent);
    assert!(offerer.local_offer().is_some());

    // Receive answer.
    let answer = offerer.on_answer("v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n".into());
    assert!(answer.is_some());
    assert_eq!(offerer.state, OffererState::WaitingForAnswer);

    // Exchange ICE candidates.
    let sig = offerer.add_local_candidate("candidate:1 1 UDP 2130706431 10.0.0.1 9000 typ host".into(), Some("0".into()), Some(0));
    assert!(sig.is_some());
    assert_eq!(offerer.state, OffererState::IceExchange);

    offerer.on_ice_candidate("candidate:2 1 UDP 1694498815 10.0.0.2 9000 typ srflx".into(), Some("0".into()), Some(0));
    assert!(!offerer.local_candidates().is_empty());

    // Connected.
    offerer.on_connected();
    assert!(offerer.is_connected());
}

#[traced_test]
#[test]
fn offerer_failure() {
    let mut offerer = WebRtcOfferer::new();
    offerer.initiate(test_peer(), "offer".into());
    offerer.on_failure();
    assert_eq!(offerer.state, OffererState::Failed);
}

// ── Answerer tests ──

#[traced_test]
#[test]
fn answerer_receives_offer_and_generates_answer() {
    let mut answerer = WebRtcAnswerer::new();
    assert_eq!(answerer.state, AnswererState::Idle);

    let offer = "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\na=ice-ufrag:browser\r\na=ice-pwd:browser-pwd\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n".to_string();
    let answer = answerer.on_offer(test_peer(), offer);
    assert!(answer.is_some());
    assert!(answer.unwrap().contains("a=ice-ufrag:browser"));
    assert_eq!(answerer.state, AnswererState::Answered);
    assert!(answerer.peer().is_some());
}

#[traced_test]
#[test]
fn answerer_ice_exchange_and_connected() {
    let mut answerer = WebRtcAnswerer::new();
    answerer.on_offer(test_peer(), "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\na=ice-ufrag:b\r\na=ice-pwd:b\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n".to_string());

    // First ICE candidate from browser.
    let locals = answerer.on_ice_candidate("candidate:1 1 UDP 2130706431 10.0.0.1 9000 typ host".into(), Some("0".into()), Some(0));
    assert!(!locals.is_empty()); // generated local candidates
    assert_eq!(answerer.state, AnswererState::IceExchange);

    // More candidates arrive.
    answerer.on_ice_candidate("candidate:2 1 UDP 1694498815 10.0.0.1 9001 typ srflx".into(), Some("0".into()), Some(0));

    // Check connected.
    assert!(answerer.check_connected());
    assert_eq!(answerer.state, AnswererState::Connected);
}

#[traced_test]
#[test]
fn answerer_failure() {
    let mut answerer = WebRtcAnswerer::new();
    answerer.on_offer(test_peer(), "offer".into());
    answerer.on_failure("timeout");
    assert_eq!(answerer.state, AnswererState::Failed);
}

// ── Signaling codec round-trip ──

#[traced_test]
#[test]
fn webrtc_signal_codec_round_trip() {
    let signals = vec![
        WebRtcSignal::Offer { sdp: "v=0\r\no=- 0 0".into() },
        WebRtcSignal::Answer { sdp: "v=0\r\no=- 1 0".into() },
        WebRtcSignal::IceCandidate { candidate: "candidate:1 1 UDP 2130706431 10.0.0.1 9000 typ host".into(), sdp_mid: Some("0".into()), sdp_mline_index: Some(0) },
    ];

    for sig in signals {
        let bytes = sig.to_bytes();
        let decoded = WebRtcSignal::from_bytes(&bytes).expect("round-trip");
        assert_eq!(sig, decoded);
    }
}

// ── Browser peer e2e relay + WebRTC upgrade test ──

#[traced_test]
#[test]
fn browser_peer_ws_relay_and_webrtc_upgrade() {
    use foundation_wireguard::wasm::browser::BrowserWgNode;
    use foundation_wireguard::shared::tunnel::WgTunnel;
    use boringtun::x25519::{PublicKey, StaticSecret};
    use std::net::IpAddr;

    // Create a BrowserWgNode (simulates a browser peer).
    let sk_bytes = [0x42u8; 32];
    let sk = StaticSecret::from(sk_bytes);
    let pk = PublicKey::from(&sk);
    let tunnel = WgTunnel::new(sk, pk, Some([0xAB; 32]), Some(25), 0);
    let overlay_ip = IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, 1));
    let mut node = BrowserWgNode::new(tunnel, 1420, overlay_ip);

    // Register a peer (simulates SWIM membership from wss join).
    let native_peer = test_peer();
    let native_ip = IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, 2));
    node.add_peer(native_ip, native_peer);

    // Tick the browser node — should queue relay packets.
    node.tick();

    // Create a WebRTC offerer and attach it to the browser node.
    let mut offerer = WebRtcOfferer::new();
    let sdp = "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\na=ice-ufrag:browser\r\na=ice-pwd:browser-pwd\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n".to_string();
    offerer.initiate(native_peer, sdp);
    assert_eq!(offerer.state, OffererState::OfferSent);

    // Native answerer processes the offer.
    let mut answerer = WebRtcAnswerer::new();
    let answer = answerer.on_offer(native_peer, "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\na=ice-ufrag:browser\r\na=ice-pwd:browser-pwd\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n".to_string()).expect("answer");
    assert_eq!(answerer.state, AnswererState::Answered);

    // Answer fed back to offerer via gossip.
    offerer.on_answer(answer);
    assert_eq!(offerer.state, OffererState::WaitingForAnswer);

    // ICE exchange.
    let sig = offerer.add_local_candidate("candidate:1 1 UDP 2130706431 10.0.0.1 9000 typ host".into(), Some("0".into()), Some(0)).expect("ICE sig");
    let decoded = WebRtcSignal::from_bytes(&sig).expect("decode ICE");
    match decoded {
        WebRtcSignal::IceCandidate { candidate, .. } => {
            answerer.on_ice_candidate(candidate, Some("0".into()), Some(0));
        }
        _ => panic!("expected IceCandidate"),
    }

    // Both sides connected.
    assert!(answerer.check_connected());
    offerer.on_connected();
    assert!(offerer.is_connected());
    assert_eq!(answerer.state, AnswererState::Connected);
}

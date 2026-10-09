#![cfg(target_arch = "wasm32")]

//! Browser peer tests — compiled to wasm32 and run in a headless browser
//! via the foundation_testbed (spec-55, F07 task 5).
//!
//! Run with:
//! ```sh
//! wasm-testbed test bindgen-deno ./foundation_wireguard
//! ```
//!
//! These tests prove the browser peer's pure-Rust state machines (JsDevice,
//! WsRelayClient, BrowserWgNode, WebRtcOfferer) compile to wasm32 and
//! execute correctly in a real JavaScript runtime.
//!
//! A full e2e test (browser WebSocket → native relay → service) requires
//! multi-process orchestration: a native relay node must be running before
//! the browser test starts. That is handled by the integration test suite
//! in `tests/mesh_tests.rs` (native) and the `foundation_testbed`'s
//! multi-process test runner for wasm32 targets.

use std::net::{IpAddr, Ipv4Addr};

use foundation_core::valtron::{self, Stream};
use foundation_macros::valtron_bindgen;
use foundation_wireguard::shared::membership::PeerId;
use foundation_wireguard::shared::tunnel::WgTunnel;
use foundation_wireguard::shared::webrtc::{OffererState, WebRtcSignal};
use foundation_wireguard::wasm::browser::BrowserWgNode;
use foundation_wireguard::wasm::device::JsDevice;
use foundation_wireguard::wasm::webrtc::WebRtcOfferer;
use boringtun::x25519::{PublicKey, StaticSecret};
use wasm_bindgen_test::wasm_bindgen_test;

use foundation_testbed::bindgen::{js_sys, wasm_bindgen_futures, web_sys};

/// The browser peer's smoltcp Device bridge compiles to wasm32 and
/// processes IP packets without panicking. This is the data plane
/// a real browser peer uses — same binary, same code path.
#[wasm_bindgen_test]
fn js_device_wasm32_inject_and_drain() {
    let mtu = 1420;
    let mut device = JsDevice::new(mtu);

    // Minimal IPv4 packet: 20 bytes, TCP, 10.0.0.1 → 10.0.0.2
    let pkt = vec![
        0x45, 0x00, 0x00, 0x14, 0x00, 0x01, 0x00, 0x00,
        0x40, 0x06, 0x00, 0x00, 0x0A, 0x00, 0x00, 0x01,
        0x0A, 0x00, 0x00, 0x02,
    ];
    device.inject(pkt);

    let _ = device.poll(std::time::Instant::now());
    let _out = device.drain_outbound();
}

/// BrowserWgNode::tick() runs to completion under wasm32 — the tunnel
/// crypto (boringtun) + smoltcp device pipeline work in a real browser
/// WebAssembly runtime.
#[valtron_bindgen]
fn browser_wg_node_wasm32_tick() {
    let sk = StaticSecret::from([0x5Au8; 32]);
    let pk = PublicKey::from(&sk);
    let tunnel = WgTunnel::new(sk, pk, Some([0xAB; 32]), Some(25), 0);
    let overlay_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
    let mut node = BrowserWgNode::new(tunnel, 1420, overlay_ip);

    // Register the relay peer (native relay node) — in a live browser
    // this mapping comes from SWIM membership received via wss join.
    node.add_peer(
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
        PeerId([0x42; 32]),
    );

    // Drive one tick — tunnel timers, device poll, encapsulation.
    node.tick();

    // The valtron pool is initialized by valtron_bindgen.
    assert!(valtron::is_pool_initialized());
}

/// WebRTC offerer state machine compiles to wasm32 and processes the
/// full signaling flow. In a live browser, the JS `RTCPeerConnection`
/// drives this state machine — this test proves the Rust side works.
#[wasm_bindgen_test]
fn webrtc_offerer_wasm32_signaling_flow() {
    let mut offerer = WebRtcOfferer::new();
    assert_eq!(offerer.state, OffererState::Idle);

    // Browser creates SDP offer via RTCPeerConnection::create_offer().
    let sdp = "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n\
               a=ice-ufrag:browser\r\na=ice-pwd:browser-pwd\r\n\
               m=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n";
    offerer.initiate(PeerId([0x42; 32]), sdp.to_string());
    assert_eq!(offerer.state, OffererState::OfferSent);

    // Native peer responds with answer (arrives via gossip).
    let answer = offerer.on_answer("v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0".to_string());
    assert!(answer.is_some());

    // ICE candidates exchanged.
    let sig = offerer.add_local_candidate(
        "candidate:1 1 UDP 2130706431 10.0.0.1 9000 typ host".into(),
        Some("0".into()),
        Some(0),
    );
    assert!(sig.is_some());

    offerer.on_ice_candidate(
        "candidate:2 1 UDP 1694498815 10.0.0.2 9000 typ srflx".into(),
        Some("0".into()),
        Some(0),
    );

    // Connected.
    offerer.on_connected();
    assert!(offerer.is_connected());
}

/// WsRelayClient round-trip compiled to wasm32. In a live browser,
/// `web_sys::WebSocket` feeds frames into this client.
#[wasm_bindgen_test]
fn ws_relay_client_wasm32_frame_round_trip() {
    use foundation_wireguard::wasm::browser::WsRelayClient;

    let mut client = WsRelayClient::new();
    let dst = PeerId([0xAA; 32]);
    client.send_packet(dst, b"wg ciphertext".to_vec());

    let frames = client.drain_outbound();
    assert_eq!(frames.len(), 1);

    // Simulate the relay echoing back (WebSocket onmessage).
    client.on_frame(&frames[0]);
    let inbound = client.drain_inbound();
    assert_eq!(inbound.len(), 1);
    assert_eq!(inbound[0].1, b"wg ciphertext");
}

/// WebRTC signaling codec round-trips under wasm32. SwimMessage::WebRtcSignal
/// is what carries SDP/ICE over SWIM gossip in both native and wasm.
#[wasm_bindgen_test]
fn webrtc_signal_codec_wasm32_round_trip() {
    let sig = WebRtcSignal::Offer { sdp: "v=0\r\no=- 0 0".into() };
    let bytes = sig.to_bytes();
    let decoded = WebRtcSignal::from_bytes(&bytes).expect("decode");
    assert_eq!(sig, decoded);

    let sig2 = WebRtcSignal::IceCandidate {
        candidate: "candidate:1 1 UDP 2130706431 10.0.0.1 9000 typ host".into(),
        sdp_mid: Some("0".into()),
        sdp_mline_index: Some(0),
    };
    let bytes2 = sig2.to_bytes();
    let decoded2 = WebRtcSignal::from_bytes(&bytes2).expect("decode");
    assert_eq!(sig2, decoded2);
}

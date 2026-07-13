//! Browser-peer bridge tests: JsDevice FIFO, WsRelayClient frames, BrowserWgNode tick (F07).
//!
//! These test the sans-I/O Rust side of the wasm module. Actual wasm32 I/O
//! (web_sys WebSocket/RtcPeerConnection) is tested via the wasm testbed
//! (see foundation_netio/tests/wasm/ for the pattern).

#![cfg(not(target_family = "wasm"))]

use std::net::{IpAddr, Ipv4Addr};

use foundation_wireguard::shared::keys::{NetworkId, SeedBits, WgSeed};
use foundation_wireguard::shared::membership::PeerId;
use foundation_wireguard::shared::tunnel::WgTunnel;
use foundation_wireguard::wasm::browser::{BrowserWgNode, WsRelayClient};
use foundation_wireguard::wasm::device::JsDevice;
use tracing_test::traced_test;

// ── JsDevice (FIFO bridge) ──

#[traced_test]
#[test]
fn js_device_inject_and_pop() {
    let mut dev = JsDevice::new(1420);
    dev.inject(vec![0x45, 0x00, 0x00, 0x14]);
    assert_eq!(dev.pending_inbound(), 1);
    let pkt = dev.pop_inbound().expect("pop");
    assert_eq!(pkt[0], 0x45);
    assert_eq!(dev.pending_inbound(), 0);
}

#[traced_test]
#[test]
fn js_device_push_and_drain_outbound() {
    let mut dev = JsDevice::new(1420);
    dev.push_outbound(b"pkt1".to_vec());
    dev.push_outbound(b"pkt2".to_vec());
    assert_eq!(dev.pending_outbound(), 2);
    let drained = dev.drain_outbound();
    assert_eq!(drained.len(), 2);
    assert_eq!(&drained[0][..], b"pkt1");
}

#[traced_test]
#[test]
fn js_device_mtu_is_stored() {
    let dev = JsDevice::new(1380);
    assert_eq!(dev.mtu(), 1380);
}

// ── WsRelayClient ──

#[traced_test]
#[test]
fn relay_client_send_recv_round_trip() {
    let mut client = WsRelayClient::new();
    let dst = PeerId([0xAA; 32]);
    client.send_packet(dst, b"wg data".to_vec());
    let frames = client.drain_outbound();
    assert_eq!(frames.len(), 1);
    client.on_frame(&frames[0]);
    let inbound = client.drain_inbound();
    assert_eq!(inbound.len(), 1);
    assert_eq!(inbound[0].1, b"wg data");
}

// ── BrowserWgNode ──

fn make_tunnel() -> WgTunnel {
    let seed = WgSeed::generate(SeedBits::Bits256).expect("seed");
    let net = NetworkId::from_bytes([0xAB; 16]);
    let boot = seed.derive_bootstrap(&net);
    WgTunnel::new(boot.static_secret, boot.public, None, None, 1)
}

#[traced_test]
#[test]
fn browser_node_tick_does_not_panic() {
    let tunnel = make_tunnel();
    let mut node = BrowserWgNode::new(
        tunnel,
        1380,
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
    );
    node.tick();
}

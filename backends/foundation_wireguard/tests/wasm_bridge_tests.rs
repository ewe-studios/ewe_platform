//! Browser-peer bridge tests: JsDevice smoltcp Device impl, WsRelayClient, BrowserWgNode tick (F07).

#![cfg(not(target_family = "wasm"))]

use std::net::{IpAddr, Ipv4Addr};

use foundation_wireguard::shared::keys::{NetworkId, SeedBits, WgSeed};
use foundation_wireguard::shared::membership::PeerId;
use foundation_wireguard::shared::tunnel::WgTunnel;
use foundation_wireguard::wasm::browser::{BrowserWgNode, WsRelayClient};
use foundation_wireguard::wasm::device::JsDevice;
use smoltcp::phy::{Device as _, RxToken as _, TxToken as _};
use smoltcp::time::Instant;
use tracing_test::traced_test;

// ── JsDevice smoltcp Device ──

#[traced_test]
#[test]
fn js_device_inject_and_receive() {
    let mut dev = JsDevice::new(1420);
    dev.inject(vec![0x45, 0x00, 0x00, 0x14]);
    let (rx, _tx) = dev.receive(Instant::from_millis(0)).expect("receive");
    rx.consume(|buf| assert_eq!(buf[0], 0x45));
}

#[traced_test]
#[test]
fn js_device_transmit_then_drain() {
    let mut dev = JsDevice::new(1420);
    let tx = dev.transmit(Instant::from_millis(0)).expect("transmit");
    tx.consume(5, |buf| buf.copy_from_slice(b"hello"));
    let out = dev.drain_outbound();
    assert_eq!(&out[0][..5], b"hello");
}

#[traced_test]
#[test]
fn js_device_capabilities() {
    let dev = JsDevice::new(1380);
    let c = dev.capabilities();
    assert_eq!(c.medium, smoltcp::phy::Medium::Ip);
    assert_eq!(c.max_transmission_unit, 1380);
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

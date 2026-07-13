//! WireGuard tunnel + driver tests (spec-55, feature 01).
//!
//! WHY: Proves success criterion 1 — two nodes on one host, sharing a network, form a
//! real WireGuard tunnel over loopback UDP and exchange application traffic over smoltcp
//! overlay sockets, with no kernel TUN and no privileges.
//!
//! HOW: Two [`TunnelDriver`]s bound to loopback UDP, each with the peer's identity public
//! key + a shared PSK. We drive both pumps in a loop; a raw WireGuard handshake completes,
//! then an overlay TCP connection carries bytes end to end.
#![cfg(not(target_family = "wasm"))]

use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use foundation_nativeapis::dataplane::netstack::{OverlayListener, OverlayStream};
use foundation_nativeapis::dataplane::{DataPlane, NetStack, NetStackConfig};
use foundation_nativeapis::native::net::UdpSocket;
use foundation_wireguard::native::TunnelDriver;
use foundation_wireguard::{IdentityKeypair, WgSeed, WgTunnel};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn wgtunnel_without_session_emits_handshake_initiation() {
    let alice = IdentityKeypair::generate().expect("alice");
    let bob = IdentityKeypair::generate().expect("bob");
    let mut tunnel = WgTunnel::new(alice.secret().clone(), bob.public(), None, None, 1);

    // With no established session, the first outbound packet triggers a handshake
    // initiation (>= 148 bytes per the WireGuard/boringtun contract) and queues the data.
    match tunnel.encapsulate(b"application data") {
        foundation_wireguard::WgOutcome::WriteToNetwork(packet) => {
            assert!(
                packet.len() >= 148,
                "handshake initiation is at least 148 bytes, got {}",
                packet.len()
            );
        }
        other => panic!("expected a handshake initiation, got {other:?}"),
    }
}

/// Drive both nodes one pump each, giving the loopback kernel a moment to deliver.
fn pump(a: &mut TunnelDriver, b: &mut TunnelDriver) {
    a.drive_once(Instant::now());
    b.drive_once(Instant::now());
    std::thread::sleep(Duration::from_millis(1));
}

#[test]
#[traced_test]
fn two_nodes_handshake_and_pass_tcp_over_overlay() {
    // Shared network material: identity keys are per-node, the PSK is shared (derived).
    let seed = WgSeed::from_bytes(&[0x55u8; 32]).expect("seed");
    let network = seed.derive_network_id();
    let psk = seed.derive_bootstrap(&network).psk;

    let alice = IdentityKeypair::generate().expect("alice id");
    let bob = IdentityKeypair::generate().expect("bob id");

    // Bind both UDP sockets first so each node knows the other's endpoint.
    let sock_a = UdpSocket::bind("127.0.0.1:0").expect("bind a");
    let sock_b = UdpSocket::bind("127.0.0.1:0").expect("bind b");
    let addr_a = sock_a.get_ref().local_addr().expect("addr a");
    let addr_b = sock_b.get_ref().local_addr().expect("addr b");

    let ip_a: IpAddr = "10.9.0.1".parse().unwrap();
    let ip_b: IpAddr = "10.9.0.2".parse().unwrap();

    let mut driver_a = TunnelDriver::new(sock_a, NetStack::new(NetStackConfig::new(ip_a, 24)));
    driver_a.add_peer(
        WgTunnel::new(alice.secret().clone(), bob.public(), Some(psk), Some(25), 1),
        addr_b,
        ip_b,
    );

    let mut driver_b = TunnelDriver::new(sock_b, NetStack::new(NetStackConfig::new(ip_b, 24)));
    driver_b.add_peer(
        WgTunnel::new(bob.secret().clone(), alice.public(), Some(psk), Some(25), 2),
        addr_a,
        ip_a,
    );

    // Overlay: B listens, A connects — over the encrypted tunnel.
    let mut net_b = driver_b.dataplane_cloned();
    let mut listener: OverlayListener = net_b
        .tcp_listen(SocketAddr::new(ip_b, 700))
        .expect("overlay listen");
    let mut net_a = driver_a.dataplane_cloned();
    let client: OverlayStream = net_a
        .tcp_connect(SocketAddr::new(ip_b, 700))
        .expect("overlay connect");

    // Drive the WireGuard handshake and the overlay TCP handshake together.
    let mut server: Option<OverlayStream> = None;
    for _ in 0..800 {
        pump(&mut driver_a, &mut driver_b);
        if server.is_none() {
            if let Ok(accepted) = listener.accept() {
                server = Some(accepted);
            }
        }
        if server.is_some() && client.may_send() {
            break;
        }
    }
    let server = server.expect("server accepted the overlay connection");
    assert!(client.may_send(), "client overlay connection established over WG");

    // client -> server
    let request = b"GET /wg HTTP/1.0\r\n\r\n";
    let mut sent = 0;
    for _ in 0..800 {
        if sent < request.len() {
            if let Ok(n) = client.write(&request[sent..]) {
                sent += n;
            }
        }
        pump(&mut driver_a, &mut driver_b);
        if sent >= request.len() {
            break;
        }
    }
    assert_eq!(sent, request.len(), "request fully sent");

    let mut received = Vec::new();
    let mut scratch = [0u8; 1024];
    for _ in 0..800 {
        match server.read(&mut scratch) {
            Ok(0) => {}
            Ok(n) => received.extend_from_slice(&scratch[..n]),
            Err(_) => {}
        }
        if received.len() >= request.len() {
            break;
        }
        pump(&mut driver_a, &mut driver_b);
    }
    assert_eq!(
        received, request,
        "server received the client's bytes intact over the WireGuard overlay"
    );

    // server -> client (reverse direction proves full duplex over the tunnel)
    let response = b"HTTP/1.0 204 No Content\r\n\r\n";
    let mut sent = 0;
    for _ in 0..800 {
        if sent < response.len() {
            if let Ok(n) = server.write(&response[sent..]) {
                sent += n;
            }
        }
        pump(&mut driver_a, &mut driver_b);
        if sent >= response.len() {
            break;
        }
    }
    let mut echoed = Vec::new();
    for _ in 0..800 {
        match client.read(&mut scratch) {
            Ok(0) => {}
            Ok(n) => echoed.extend_from_slice(&scratch[..n]),
            Err(_) => {}
        }
        if echoed.len() >= response.len() {
            break;
        }
        pump(&mut driver_a, &mut driver_b);
    }
    assert_eq!(echoed, response, "client received the server's reply over the tunnel");
}

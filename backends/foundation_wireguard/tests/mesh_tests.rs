//! End-to-end mesh tests (spec-55, feature 04).
//!
//! WHY: Proves the whole stack as a network — nodes form a mesh from one secret, a late
//! joiner is discovered by gossip (not static config) and reaches existing peers, and the
//! mesh survives the seed's death while a fresh node still joins from a survivor
//! (success criteria 2 & 3).
#![cfg(not(target_family = "wasm"))]

use std::io::ErrorKind;
use std::net::IpAddr;
use std::time::Duration;

use foundation_wireguard::native::{WgConfig, WgHandle, WgNode};
use foundation_wireguard::WgSeed;
use tracing_test::traced_test;

fn network() -> (WgSeed, foundation_wireguard::NetworkId) {
    let seed = WgSeed::from_bytes(&[0x5Au8; 32]).unwrap();
    let network_id = seed.derive_network_id();
    (seed, network_id)
}

/// Open an overlay TCP connection, retrying until the tunnel is up or the deadline passes.
fn connect_with_retry(handle: &WgHandle, peer_ip: IpAddr, port: u16) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    while std::time::Instant::now() < deadline {
        if let Ok(stream) = handle.tcp_connect(peer_ip, port) {
            // Drive the overlay TCP handshake for a moment.
            for _ in 0..400 {
                if stream.may_send() {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Send a request over an overlay stream and read the echoed reply.
fn exchange_over_overlay(handle: &WgHandle, peer_ip: IpAddr, port: u16, msg: &[u8]) -> Vec<u8> {
    let stream = handle
        .tcp_connect(peer_ip, port)
        .expect("overlay connect after tunnel up");
    // Establish.
    for _ in 0..400 {
        if stream.may_send() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // Write.
    let mut sent = 0;
    for _ in 0..400 {
        match stream.write(&msg[sent..]) {
            Ok(n) => sent += n,
            Err(e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(_) => break,
        }
        if sent >= msg.len() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    // Read the echo.
    let mut got = Vec::new();
    let mut buf = [0u8; 512];
    for _ in 0..800 {
        match stream.read(&mut buf) {
            Ok(0) => {}
            Ok(n) => got.extend_from_slice(&buf[..n]),
            Err(_) => {}
        }
        if got.len() >= msg.len() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    got
}

/// Run a tiny echo server on the overlay in a background thread until `stop`.
fn spawn_echo(handle: &WgHandle, port: u16, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let listener = handle.tcp_listen(port).expect("overlay listen");
    let mut listener = listener;
    std::thread::spawn(move || {
        use std::sync::atomic::Ordering;
        let mut conns: Vec<foundation_nativeapis::dataplane::netstack::OverlayStream> = Vec::new();
        while !stop.load(Ordering::Relaxed) {
            if let Ok(s) = listener.accept() {
                conns.push(s);
            }
            let mut buf = [0u8; 512];
            for c in &conns {
                if let Ok(n) = c.read(&mut buf) {
                    if n > 0 {
                        let _ = c.write(&buf[..n]);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
}

#[test]
#[traced_test]
fn three_node_mesh_gossip_discovery_and_overlay_traffic() {
    let (seed, network_id) = network();

    // Node A — the seed.
    let node_a = WgNode::from_config(WgConfig::seed(seed.clone(), network_id))
        .join()
        .expect("A joins");
    let a_boot = node_a.bootstrap_addr();
    let a_ip = node_a.overlay_ip();

    // Node B joins A.
    let node_b = WgNode::from_config(WgConfig::joiner(
        seed.clone(),
        network_id,
        vec![a_boot],
    ))
    .join()
    .expect("B joins");
    let b_ip = node_b.overlay_ip();
    let b_boot = node_b.bootstrap_addr();

    // Node C joins A too; it should learn about B purely via gossip/membership.
    let node_c = WgNode::from_config(WgConfig::joiner(seed.clone(), network_id, vec![a_boot]))
        .join()
        .expect("C joins");
    let c_ip = node_c.overlay_ip();

    // C discovers B (not statically configured) through membership.
    assert!(
        node_c.wait_for_peer(b_ip, Duration::from_secs(8)),
        "C discovered B via gossip"
    );
    assert!(
        node_c.wait_for_peer(a_ip, Duration::from_secs(8)),
        "C discovered A"
    );

    // A and B run overlay echo servers.
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    spawn_echo(&node_a, 7000, stop.clone());
    spawn_echo(&node_b, 7000, stop.clone());

    // C reaches both A and B over the encrypted overlay.
    assert!(connect_with_retry(&node_c, a_ip, 7000), "C tunnel to A up");
    let echo_a = exchange_over_overlay(&node_c, a_ip, 7000, b"hello-A");
    assert_eq!(echo_a, b"hello-A", "C↔A overlay echo");

    assert!(connect_with_retry(&node_c, b_ip, 7000), "C tunnel to B up");
    let echo_b = exchange_over_overlay(&node_c, b_ip, 7000, b"hello-B");
    assert_eq!(echo_b, b"hello-B", "C↔B overlay echo");

    // ---- Success criterion 3: kill the seed A; survivors carry on. ----
    node_a.shutdown();

    // B and C still see each other alive (their B↔C tunnel is independent of A).
    assert!(
        node_c.wait_for_peer(b_ip, Duration::from_secs(4)),
        "C still sees B after seed death"
    );

    // A fourth node D joins from a survivor (B), proving the seed's role ended at t=0.
    let node_d = WgNode::from_config(WgConfig::joiner(seed.clone(), network_id, vec![b_boot]))
        .join()
        .expect("D joins from survivor B");
    let d_ip = node_d.overlay_ip();
    assert!(
        node_d.wait_for_peer(c_ip, Duration::from_secs(8)),
        "D (joined via B) discovers C"
    );
    let _ = d_ip;

    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    node_b.shutdown();
    node_c.shutdown();
    node_d.shutdown();
}

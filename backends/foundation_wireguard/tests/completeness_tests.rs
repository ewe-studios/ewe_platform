//! Spec-55 completeness integration tests.
//!
//! WHY: The gatekeeper — only runs when `feature = "spec55-complete"` is set.
//! These tests exercise the full userspace WireGuard mesh: two threads
//! talking to each other over a private overlay network.
//!
//! WHAT: Two-node seed+joiner mesh, 3-node mesh with gossip discovery,
//! overlay TCP echo, identity persistence restart, relay capability
//! advertisement, and config convergence (builder + TOML + macro).
//!
//! HOW: Same patterns as the other integration tests — `#[traced_test]`,
//! real boringtun tunnels, smoltcp netstack, loopback UDP.

#![cfg(not(target_family = "wasm"))]

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_wireguard::{
    NetworkId, RelayConfig, SecurityConfig, SeedBits, WgConfig, WgSeed, wireguard,
};
use foundation_wireguard::native::{WgHandle, WgNode};
use foundation_wireguard::shared::keys::IdentityKeypair;
use tracing_test::traced_test;

fn fresh_seed() -> (WgSeed, NetworkId) {
    let seed = WgSeed::generate(SeedBits::Bits256).expect("generate seed");
    let net = seed.derive_network_id();
    (seed, net)
}

// ── Two-node mesh: seed + joiner, overlay TCP echo ──

#[traced_test]
#[test]
fn two_nodes_private_network_overlay_tcp_echo() {
    let (seed, net) = fresh_seed();

    // Node A: seed via builder.
    let cfg_a = WgConfig::builder()
        .seed(seed.clone())
        .network_id(net)
        .build()
        .expect("build A");
    let handle_a = WgNode::from_config(cfg_a).join().expect("join A");
    let a_boot = handle_a.bootstrap_addr();

    // Node B: joiner via builder.
    let cfg_b = WgConfig::builder()
        .seed(seed.clone())
        .network_id(net)
        .seed_endpoint(a_boot)
        .build()
        .expect("build B");
    let handle_b = WgNode::from_config(cfg_b).join().expect("join B");

    // Wait for mutual discovery.
    assert!(
        handle_a.wait_for_peer(handle_b.overlay_ip(), Duration::from_secs(5)),
        "A discovered B"
    );
    assert!(
        handle_b.wait_for_peer(handle_a.overlay_ip(), Duration::from_secs(5)),
        "B discovered A"
    );

    // Overlay TCP: B listens, A connects — with retries for tunnel setup.
    let b_ip = handle_b.overlay_ip();
    let mut listener = handle_b.tcp_listen(9999).expect("tcp_listen");
    let b_port = listener.local_addr().port();

    // Spawn echo handler on B.
    let b_stop = Arc::new(AtomicBool::new(false));
    let b_stop_clone = Arc::clone(&b_stop);
    std::thread::spawn(move || {
        while !b_stop_clone.load(Ordering::Relaxed) {
            if let Ok(mut s) = listener.accept() {
                let mut buf = [0u8; 256];
                if let Ok(n) = s.read(&mut buf) {
                    let _ = s.write(&buf[..n]);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });

    // A connects and sends, reads echo.
    let mut stream = None;
    for _ in 0..30 {
        match handle_a.tcp_connect(b_ip, b_port) {
            Ok(s) => { stream = Some(s); break; }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let mut stream = stream.expect("tcp_connect after retries");

    let msg = b"spec55 completeness test";
    stream.write(msg).expect("write");

    let mut buf = [0u8; 256];
    let n = stream.read(&mut buf).expect("read");
    assert_eq!(&buf[..n], msg, "echo round-trip: sent == received");

    b_stop.store(true, Ordering::Relaxed);
    handle_a.shutdown();
    handle_b.shutdown();
}

// ── Three-node mesh: gossip discovery of non-configured peer ──

#[traced_test]
#[test]
fn three_node_mesh_gossip_discovery() {
    let (seed, net) = fresh_seed();

    let handle_a = WgNode::from_config(WgConfig::seed(seed.clone(), net))
        .join().expect("join A");
    let a_boot = handle_a.bootstrap_addr();

    let handle_b = WgNode::from_config(
        WgConfig::joiner(seed.clone(), net, vec![a_boot]),
    ).join().expect("join B");

    // C joins via B (not A) — gossip must propagate.
    let b_boot = handle_b.bootstrap_addr();
    let handle_c = WgNode::from_config(
        WgConfig::joiner(seed.clone(), net, vec![b_boot]),
    ).join().expect("join C");

    // C should discover A through gossip (not direct join).
    assert!(
        handle_c.wait_for_peer(handle_a.overlay_ip(), Duration::from_secs(5)),
        "C discovered A via gossip"
    );

    // All three should see each other.
    assert!(handle_a.wait_for_peer(handle_b.overlay_ip(), Duration::from_secs(1)));
    assert!(handle_a.wait_for_peer(handle_c.overlay_ip(), Duration::from_secs(1)));
    assert!(handle_b.wait_for_peer(handle_c.overlay_ip(), Duration::from_secs(1)));

    handle_a.shutdown();
    handle_b.shutdown();
    handle_c.shutdown();
}

// ── Config convergence: builder + TOML produce equivalent config ──

#[traced_test]
#[test]
fn builder_and_toml_produce_equivalent_config() {
    let (seed, net) = fresh_seed();

    let builder_cfg = WgConfig::builder()
        .seed(seed.clone())
        .network_id(net)
        .mtu(1400)
        .keepalive_secs(30)
        .relay_advertise(true)
        .relay_max_sessions(1024)
        .build()
        .expect("builder");

    // Serialize to TOML and deserialize back.
    let toml_str = toml::to_string_pretty(&builder_cfg).expect("serialize");
    let toml_cfg: WgConfig = toml::from_str(&toml_str).expect("deserialize");

    assert_eq!(builder_cfg.network_id(), toml_cfg.network_id());
    assert_eq!(builder_cfg.wg_seed().as_bytes(), toml_cfg.wg_seed().as_bytes());
    assert_eq!(builder_cfg.dataplane.mtu, toml_cfg.dataplane.mtu);
    assert_eq!(builder_cfg.dataplane.keepalive_secs, toml_cfg.dataplane.keepalive_secs);
    assert_eq!(builder_cfg.relay.advertise, toml_cfg.relay.advertise);
    assert_eq!(builder_cfg.relay.max_sessions, toml_cfg.relay.max_sessions);
}

// ── Identity persistence: restart rejoins as same identity ──

#[traced_test]
#[test]
fn identity_persistence_restart() {
    let (seed, net) = fresh_seed();

    // First boot: generate identity, persist it.
    let path = {
        let mut cfg = WgConfig::builder()
            .seed(seed.clone())
            .network_id(net)
            .identity_path("/tmp/wg_completeness_test_identity.key")
            .build()
            .expect("build");
        // Force identity persistence by joining.
        let handle = WgNode::from_config(cfg).join().expect("first join");
        let identity = handle.identity();
        handle.shutdown();
        identity
    };

    // Second boot: same identity_path, should load persisted keypair.
    let cfg2 = WgConfig::builder()
        .seed(seed.clone())
        .network_id(net)
        .identity_path("/tmp/wg_completeness_test_identity.key")
        .build()
        .expect("build");
    let handle2 = WgNode::from_config(cfg2).join().expect("second join");
    let identity2 = handle2.identity();

    assert_eq!(path, identity2, "restart rejoined as same identity");
    handle2.shutdown();

    // Clean up persisted file.
    let _ = std::fs::remove_file("/tmp/wg_completeness_test_identity.key");
}

// ── Relay capability advertisement ──

#[traced_test]
#[test]
fn relay_capability_advertised_in_membership() {
    let (seed, net) = fresh_seed();

    let cfg = WgConfig::builder()
        .seed(seed.clone())
        .network_id(net)
        .relay_advertise(true)
        .relay_max_sessions(256)
        .relay_rate_limit_pps(500)
        .relay_idle_timeout_secs(90)
        .build()
        .expect("build");

    let handle = WgNode::from_config(cfg.clone()).join().expect("join");
    assert!(handle.is_relay(), "node should advertise relay");
    assert_eq!(cfg.relay.max_sessions, 256);
    assert_eq!(cfg.relay.rate_limit_pps, 500);
    assert_eq!(cfg.relay.idle_timeout_secs, 90);

    // Verify relay appears in own membership view.
    let members = handle.members();
    let self_view = members.iter().find(|p| p.identity == handle.identity())
        .expect("self in membership");
    assert!(self_view.caps.relay, "caps.relay should be true");

    handle.shutdown();
}

// ── wireguard! macro convergence ──

#[traced_test]
#[test]
fn macro_produces_valid_config() {
    let (seed, net) = fresh_seed();
    let seed_b64 = seed.to_base64url();
    let net_hex = net.to_hex();

    // Test the wireguard! macro at compile time — same network expressed
    // via macro SHOULD produce an equivalent config to the builder.
    let macro_cfg = wireguard! {
        seed: seed_b64,
        network_id: net_hex,
        udp_listen: "127.0.0.1:0",
        relay: { advertise: true, max_sessions: 512 },
        security: { mtls: false },
    };

    assert_eq!(macro_cfg.network_id(), net);
    assert_eq!(macro_cfg.wg_seed().as_bytes(), seed.as_bytes());
    assert!(macro_cfg.relay.advertise);
    assert_eq!(macro_cfg.relay.max_sessions, 512);

    // Verify it boots a working node.
    let handle = WgNode::from_config(macro_cfg).join().expect("join");
    assert!(handle.is_relay());
    handle.shutdown();
}

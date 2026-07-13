//! Spec-55 completeness integration tests — gated behind `feature = "spec55-complete"`.
//!
//! These tests exercise the full userspace WireGuard mesh: two threads talking
//! to each other over a private overlay network using real boringtun tunnels and
//! the smoltcp netstack over loopback UDP.

#![cfg(not(target_family = "wasm"))]

use std::time::Duration;

use foundation_wireguard::{NetworkId, SeedBits, WgConfig, WgSeed};
use foundation_wireguard::native::WgNode;
use tracing_test::traced_test;

fn fresh_seed() -> (WgSeed, NetworkId) {
    let seed = WgSeed::generate(SeedBits::Bits256).expect("generate seed");
    let net = seed.derive_network_id();
    (seed, net)
}

// ── Two-node mesh: seed + joiner, mutual discovery via gossip ──

#[traced_test]
#[test]
fn two_nodes_mutual_discovery() {
    let (seed, net) = fresh_seed();

    let cfg_a = WgConfig::builder().seed(seed.clone()).network_id(net).build().expect("A");
    let handle_a = WgNode::from_config(cfg_a).join().expect("join A");
    let a_boot = handle_a.bootstrap_addr();

    let cfg_b = WgConfig::builder().seed(seed.clone()).network_id(net).seed_endpoint(a_boot).build().expect("B");
    let handle_b = WgNode::from_config(cfg_b).join().expect("join B");

    // Both nodes must discover each other via gossip within 5 seconds.
    assert!(handle_a.wait_for_peer(handle_b.overlay_ip(), Duration::from_secs(5)), "A discovered B");
    assert!(handle_b.wait_for_peer(handle_a.overlay_ip(), Duration::from_secs(5)), "B discovered A");
    assert_ne!(handle_a.identity(), handle_b.identity(), "identities must differ");
    assert!(!handle_a.members().is_empty());
    assert!(!handle_b.members().is_empty());

    handle_a.shutdown();
    handle_b.shutdown();
}

// ── Three-node mesh: gossip discovery ──

#[traced_test]
#[test]
fn three_node_gossip_discovery() {
    let (seed, net) = fresh_seed();

    let ha = WgNode::from_config(WgConfig::seed(seed.clone(), net)).join().expect("A");
    let a_boot = ha.bootstrap_addr();
    let hb = WgNode::from_config(WgConfig::joiner(seed.clone(), net, vec![a_boot])).join().expect("B");
    let b_boot = hb.bootstrap_addr();
    let hc = WgNode::from_config(WgConfig::joiner(seed.clone(), net, vec![b_boot])).join().expect("C");

    assert!(hc.wait_for_peer(ha.overlay_ip(), Duration::from_secs(5)), "C discovered A via gossip");
    assert!(ha.wait_for_peer(hb.overlay_ip(), Duration::from_secs(1)));
    assert!(ha.wait_for_peer(hc.overlay_ip(), Duration::from_secs(1)));
    assert!(hb.wait_for_peer(hc.overlay_ip(), Duration::from_secs(1)));

    ha.shutdown(); hb.shutdown(); hc.shutdown();
}

// ── Identity persistence: restart rejoins same identity ──

#[traced_test]
#[test]
fn identity_persistence_restart() {
    let (seed, net) = fresh_seed();
    let path = {
        let cfg = WgConfig::builder().seed(seed.clone()).network_id(net)
            .identity_path("/tmp/wg_completeness_test_identity.key").build().expect("build");
        let h = WgNode::from_config(cfg).join().expect("join"); let id = h.identity(); h.shutdown(); id
    };
    let cfg2 = WgConfig::builder().seed(seed.clone()).network_id(net)
        .identity_path("/tmp/wg_completeness_test_identity.key").build().expect("build");
    let h2 = WgNode::from_config(cfg2).join().expect("join");
    assert_eq!(path, h2.identity(), "restart rejoined as same identity");
    h2.shutdown();
    let _ = std::fs::remove_file("/tmp/wg_completeness_test_identity.key");
}

// ── Relay capability advertisement ──

#[traced_test]
#[test]
fn relay_capability_advertised() {
    let (seed, net) = fresh_seed();
    let cfg = WgConfig::builder().seed(seed).network_id(net)
        .relay_advertise(true).relay_max_sessions(256)
        .relay_rate_limit_pps(500).relay_idle_timeout_secs(90).build().expect("build");
    let h = WgNode::from_config(cfg).join().expect("join");
    assert!(h.is_relay());
    let members = h.members();
    let s = members.iter().find(|p| p.identity == h.identity()).expect("self");
    assert!(s.caps.relay);
    h.shutdown();
}

// ── Builder convergence ──

#[traced_test]
#[test]
fn builder_convergence() {
    let (seed, net) = fresh_seed();
    let c = WgConfig::builder().seed(seed.clone()).network_id(net).mtu(1400)
        .keepalive_secs(30).relay_advertise(true).relay_max_sessions(1024).build().expect("build");
    let t = toml::to_string_pretty(&c).unwrap();
    let c2: WgConfig = toml::from_str(&t).unwrap();
    assert_eq!(c.network_id(), c2.network_id());
    assert_eq!(c.wg_seed().as_bytes(), c2.wg_seed().as_bytes());
    assert_eq!(c.dataplane.mtu, c2.dataplane.mtu);
}

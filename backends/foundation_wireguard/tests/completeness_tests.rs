//! Spec-55 completeness integration tests — gated behind `feature = "spec55-complete"`.

#![cfg(not(target_family = "wasm"))]

use std::time::Duration;

use foundation_wireguard::{NetworkId, SeedBits, WgConfig, WgSeed, wireguard};
use foundation_wireguard::native::WgNode;
use tracing_test::traced_test;

fn fresh_seed() -> (WgSeed, NetworkId) {
    let seed = WgSeed::generate(SeedBits::Bits256).expect("generate seed");
    let net = seed.derive_network_id();
    (seed, net)
}

#[traced_test]
#[test]
fn two_nodes_mutual_discovery() {
    let (seed, net) = fresh_seed();
    let ha = WgNode::from_config(WgConfig::seed(seed.clone(), net)).join().expect("A");
    let a_boot = ha.bootstrap_addr();
    let hb = WgNode::from_config(WgConfig::joiner(seed.clone(), net, vec![a_boot])).join().expect("B");

    assert!(ha.wait_for_peer(hb.overlay_ip(), Duration::from_secs(5)), "A→B");
    assert!(hb.wait_for_peer(ha.overlay_ip(), Duration::from_secs(5)), "B→A");
    assert_ne!(ha.identity(), hb.identity());
    assert!(!ha.members().is_empty());

    ha.shutdown(); hb.shutdown();
}

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

#[traced_test]
#[test]
fn relay_capability_advertised() {
    let (seed, net) = fresh_seed();
    let cfg = WgConfig::builder().seed(seed).network_id(net)
        .relay_advertise(true).relay_max_sessions(256)
        .relay_rate_limit_pps(500).relay_idle_timeout_secs(90).build().expect("build");
    let h = WgNode::from_config(cfg).join().expect("join");
    assert!(h.is_relay());
    let m = h.members();
    let s = m.iter().find(|p| p.identity == h.identity()).expect("self");
    assert!(s.caps.relay);
    h.shutdown();
}

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

#[traced_test]
#[test]
fn macro_produces_valid_config() {
    // The wireguard! macro expands to a builder chain with .build()? — so we
    // need to handle the Result. Wrap in a closure returning Result.
    let config = (|| -> foundation_wireguard::WgResult<WgConfig> {
        Ok(wireguard! {
            seed: "q6urq6urq6urq6urq6urq6urq6urq6urq6urq6urq6s",
            relay: { advertise: true, max_sessions: 512, rate_limit_pps: 500, idle_timeout_secs: 90 },
            security: { mtls: false },
            mtu: 1400,
            keepalive: 30,
        })
    })().expect("wireguard! macro expansion");

    // Verify the macro output matches what a builder with the same values produces.
    let seed = WgSeed::from_bytes(&[0xABu8; 32]).expect("seed");
    let expected = WgConfig::builder()
        .seed(seed.clone())
        .relay_advertise(true)
        .relay_max_sessions(512)
        .relay_rate_limit_pps(500)
        .relay_idle_timeout_secs(90)
        .mtu(1400)
        .keepalive_secs(30)
        .build()
        .expect("builder");

    assert_eq!(config.wg_seed().as_bytes(), expected.wg_seed().as_bytes());
    assert_eq!(config.network_id(), expected.network_id()); // both derived from seed
    assert_eq!(config.relay.advertise, expected.relay.advertise);
    assert_eq!(config.relay.max_sessions, expected.relay.max_sessions);
    assert_eq!(config.relay.rate_limit_pps, expected.relay.rate_limit_pps);
    assert_eq!(config.relay.idle_timeout_secs, expected.relay.idle_timeout_secs);
    assert_eq!(config.dataplane.mtu, expected.dataplane.mtu);
    assert_eq!(config.dataplane.keepalive_secs, expected.dataplane.keepalive_secs);

    // Verify it boots a working node.
    let handle = WgNode::from_config(config).join().expect("join");
    assert!(handle.is_relay(), "macro-built config boots with relay");
    handle.shutdown();
}

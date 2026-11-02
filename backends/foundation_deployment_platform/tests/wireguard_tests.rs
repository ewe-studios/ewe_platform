//! Tests for the `WireGuard` mesh helpers in `docker::wireguard` (rule 7: moved
//! out of the source file).

#![cfg(feature = "wireguard")]

use std::net::SocketAddr;

use foundation_deployment_platform::docker::wireguard::{
    ENV_WG_NETWORK, ENV_WG_RELAY, ENV_WG_SECRET, ENV_WG_SEED_ENDPOINTS, WireguardInjector,
    designate_relay, generate_network_secret,
};
use foundation_deployment_platform::docker::{ContainerConfig, PortProtocol};
use foundation_wireguard::SeedBits;

#[test]
fn generate_produces_valid_secret() {
    let secret = generate_network_secret();
    assert_eq!(secret.seed.bits(), SeedBits::Bits256);
    assert_eq!(secret.seed_base64url().len(), 43); // 32 bytes → 43 base64url chars
    assert_eq!(secret.network_id_hex().len(), 32); // 16 bytes → 32 hex chars
}

#[test]
fn injector_adds_env_vars() {
    let secret = generate_network_secret();
    let ep: SocketAddr = "10.0.0.1:51820".parse().expect("addr");

    let config = ContainerConfig::new("test-image").inject_wireguard(&secret, &[ep], false);

    let has_secret = config.env.iter().any(|(k, _)| k == ENV_WG_SECRET);
    let has_network = config.env.iter().any(|(k, _)| k == ENV_WG_NETWORK);
    let has_endpoints = config
        .env
        .iter()
        .any(|(k, v)| k == ENV_WG_SEED_ENDPOINTS && v.contains("51820"));

    assert!(has_secret, "WG_SECRET missing");
    assert!(has_network, "WG_NETWORK missing");
    assert!(has_endpoints, "WG_SEED_ENDPOINTS missing or wrong value");
}

#[test]
fn injector_no_endpoints_when_empty() {
    let secret = generate_network_secret();
    let config = ContainerConfig::new("test-image").inject_wireguard(&secret, &[], true);

    let has_endpoints = config.env.iter().any(|(k, _)| k == ENV_WG_SEED_ENDPOINTS);
    assert!(
        !has_endpoints,
        "WG_SEED_ENDPOINTS should not be set when empty"
    );
}

#[test]
fn designate_relay_adds_ports_and_env() {
    let secret = generate_network_secret();
    let mut config = ContainerConfig::new("relay-node").inject_wireguard(&secret, &[], true);

    designate_relay(&mut config, 51821, 51820);

    let has_relay_env = config
        .env
        .iter()
        .any(|(k, v)| k == ENV_WG_RELAY && v == "true");
    assert!(has_relay_env, "WG_RELAY=true missing");
    // Two UDP ports (relay + wg).
    let udp_ports: Vec<_> = config
        .ports
        .iter()
        .filter(|p| p.protocol == PortProtocol::Udp)
        .collect();
    assert_eq!(udp_ports.len(), 2);
}

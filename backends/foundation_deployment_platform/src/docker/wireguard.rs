//! WireGuard mesh injection for Docker containers (spec-55, F10).
//!
//! WHY: The owner's headline flow — `foundation_deployment_platform`'s Docker
//! capability generates a network secret once and injects it into every container
//! it launches, so a fleet self-assembles into one private mesh with no manual
//! key distribution.
//!
//! WHAT: [`WgNetworkSecret`] — generated seed + network id, persistable.
//! [`WireguardInjector`] — trait that adds mesh env vars to any [`ContainerConfig`].
//! [`generate_network_secret`] — generates a fresh B256 seed.
//! [`designate_relay`] / [`designate_seed`] — mark containers as relay/seed nodes.
//!
//! HOW: Containers boot via `#[wireguard_main]` (F09), read the env vars injected
//! here, call `WgConfig::from_env()`, and join the mesh — no coordinator.

use std::net::SocketAddr;

use foundation_wireguard::{NetworkId, SeedBits, WgSeed};


use super::config::{ContainerConfig, PortMapping, PortProtocol};

// ---------------------------------------------------------------------------
// Env var names (normative — must match WgConfig::from_env)
// ---------------------------------------------------------------------------

/// Environment variable: the bootstrap seed (base64url).
pub const ENV_WG_SECRET: &str = "WG_SECRET";
/// Environment variable: the network id (hex).
pub const ENV_WG_NETWORK: &str = "WG_NETWORK";
/// Environment variable: comma-separated seed endpoint addresses.
pub const ENV_WG_SEED_ENDPOINTS: &str = "WG_SEED_ENDPOINTS";
/// Environment variable: whether this container advertises relay capability.
pub const ENV_WG_RELAY: &str = "WG_RELAY";

// ---------------------------------------------------------------------------
// WgNetworkSecret
// ---------------------------------------------------------------------------

/// A generated WireGuard network secret — seed + network id, ready for injection.
///
/// WHY: Generated once at deploy time, persisted in deployment state, reused for
/// scale-ups and re-deploys so the fleet stays on the same network.
#[derive(Debug, Clone)]
pub struct WgNetworkSecret {
    /// The bootstrap seed.
    pub seed: WgSeed,
    /// The network id (derived from the seed if not explicitly set).
    pub network_id: NetworkId,
}

impl WgNetworkSecret {
    /// Serialise the seed as base64url for env injection.
    #[must_use]
    pub fn seed_base64url(&self) -> String {
        self.seed.to_base64url()
    }

    /// The network id as hex for env injection.
    #[must_use]
    pub fn network_id_hex(&self) -> String {
        self.network_id.to_hex()
    }

    /// Persist this secret to a file (seed as base64url, network as hex).
    ///
    /// The file format is one line per key: `KEY=VALUE`, same as the env vars
    /// injected into containers, so the file doubles as an env-file.
    ///
    /// # Errors
    /// Returns an error if the file cannot be written.
    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        let content = format!(
            "{}=\"{}\"\n{}=\"{}\"\n",
            ENV_WG_SECRET,
            self.seed_base64url(),
            ENV_WG_NETWORK,
            self.network_id_hex(),
        );
        if let Some(parent) = std::path::Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(path, content)
    }

    /// Load a previously persisted secret from a file.
    ///
    /// # Errors
    /// Returns `None` if the file doesn't exist, can't be read, or is malformed.
    #[must_use]
    pub fn load_from_file(path: &str) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        let mut seed_str = None;
        let mut net_str = None;
        for line in content.lines() {
            if let Some((key, val)) = line.split_once('=') {
                let val = val.trim().trim_matches('"');
                match key.trim() {
                    ENV_WG_SECRET => seed_str = Some(val.to_string()),
                    ENV_WG_NETWORK => net_str = Some(val.to_string()),
                    _ => {}
                }
            }
        }
        let seed_str = seed_str?;
        let seed = foundation_wireguard::WgSeed::from_base64url(&seed_str).ok()?;
        let network_id = if let Some(hex) = net_str {
            foundation_wireguard::NetworkId::from_hex(&hex).ok()?
        } else {
            seed.derive_network_id()
        };
        Some(Self { seed, network_id })
    }
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// WHY: Networks are bootstrapped from a freshly generated high-entropy seed
/// (decision 03 — B256 recommended for long-lived networks).
///
/// WHAT: Generate a new 256-bit seed and derive its network id.
///
/// HOW: `WgSeed::generate(SeedBits::Bits256)` + `seed.derive_network_id()`.
///
/// # Panics
/// Panics if the OS random source is unavailable (fatal at deploy time).
#[must_use]
pub fn generate_network_secret() -> WgNetworkSecret {
    let seed = WgSeed::generate(SeedBits::Bits256)
        .expect("OS random source unavailable — cannot generate network secret");
    let network_id = seed.derive_network_id();
    WgNetworkSecret { seed, network_id }
}

/// Like [`generate_network_secret`] but with an explicit network id.
#[must_use]
pub fn generate_network_secret_with_id(network_id: NetworkId) -> WgNetworkSecret {
    let seed = WgSeed::generate(SeedBits::Bits256)
        .expect("OS random source unavailable");
    WgNetworkSecret {
        seed,
        network_id,
    }
}

// ---------------------------------------------------------------------------
// WireguardInjector — trait that adds WG env to ContainerConfig
// ---------------------------------------------------------------------------

/// WHY: One method call injects all the env vars a container needs to join the mesh.
///
/// WHAT: Implemented for [`ContainerConfig`]. Adds `WG_SECRET`, `WG_NETWORK`,
/// and optionally `WG_SEED_ENDPOINTS` and `WG_RELAY`.
pub trait WireguardInjector {
    /// Inject the mesh bootstrap environment into this config.
    ///
    /// Sets `WG_SECRET` and `WG_NETWORK`. If `seed_endpoints` is non-empty,
    /// also sets `WG_SEED_ENDPOINTS` as a comma-separated list. If
    /// `relay_enabled`, sets `WG_RELAY=true`.
    fn inject_wireguard(
        self,
        secret: &WgNetworkSecret,
        seed_endpoints: &[SocketAddr],
        relay_enabled: bool,
    ) -> Self;
}

impl WireguardInjector for ContainerConfig {
    fn inject_wireguard(
        mut self,
        secret: &WgNetworkSecret,
        seed_endpoints: &[SocketAddr],
        relay_enabled: bool,
    ) -> Self {
        self.env.push((
            ENV_WG_SECRET.to_string(),
            secret.seed_base64url(),
        ));
        self.env.push((
            ENV_WG_NETWORK.to_string(),
            secret.network_id_hex(),
        ));

        if !seed_endpoints.is_empty() {
            let eps: Vec<String> = seed_endpoints.iter().map(|a| a.to_string()).collect();
            self.env.push((
                ENV_WG_SEED_ENDPOINTS.to_string(),
                eps.join(","),
            ));
        }

        if relay_enabled {
            self.env.push((ENV_WG_RELAY.to_string(), "true".to_string()));
        }

        self
    }
}

// ---------------------------------------------------------------------------
// Container role designation
// ---------------------------------------------------------------------------

/// WHY: Seed and relay-capable containers need published WG UDP ports so peers
/// can reach them.
///
/// WHAT: Designate this container as a seed node — publishes the WG port and
/// marks it as having no seed endpoints (it IS the first seed).
pub fn designate_seed(config: &mut ContainerConfig, wg_port: u16) {
    config.ports.push(super::config::PortMapping {
        container_port: wg_port,
        host_port: None, // auto-assign
        protocol: PortProtocol::Udp,
    });
}

/// Designate this container as a relay-capable node — publishes the relay
/// port and sets the WG_RELAY env var.
pub fn designate_relay(config: &mut ContainerConfig, relay_port: u16, wg_port: u16) {
    // Publish the relay port.
    config.ports.push(super::config::PortMapping {
        container_port: relay_port,
        host_port: None,
        protocol: PortProtocol::Udp,
    });
    // Publish the WG port (for direct peer connections).
    config.ports.push(super::config::PortMapping {
        container_port: wg_port,
        host_port: None,
        protocol: PortProtocol::Udp,
    });
    config.env.push((ENV_WG_RELAY.to_string(), "true".to_string()));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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

        let config = ContainerConfig::new("test-image")
            .inject_wireguard(&secret, &[ep], false);

        let has_secret = config.env.iter().any(|(k, _)| k == ENV_WG_SECRET);
        let has_network = config.env.iter().any(|(k, _)| k == ENV_WG_NETWORK);
        let has_endpoints = config.env.iter().any(|(k, v)| k == ENV_WG_SEED_ENDPOINTS && v.contains("51820"));

        assert!(has_secret, "WG_SECRET missing");
        assert!(has_network, "WG_NETWORK missing");
        assert!(has_endpoints, "WG_SEED_ENDPOINTS missing or wrong value");
    }

    #[test]
    fn injector_no_endpoints_when_empty() {
        let secret = generate_network_secret();
        let config = ContainerConfig::new("test-image")
            .inject_wireguard(&secret, &[], true);

        let has_endpoints = config.env.iter().any(|(k, _)| k == ENV_WG_SEED_ENDPOINTS);
        assert!(!has_endpoints, "WG_SEED_ENDPOINTS should not be set when empty");
    }

    #[test]
    fn designate_relay_adds_ports_and_env() {
        let secret = generate_network_secret();
        let mut config = ContainerConfig::new("relay-node")
            .inject_wireguard(&secret, &[], true);

        designate_relay(&mut config, 51821, 51820);

        let has_relay_env = config.env.iter().any(|(k, v)| k == ENV_WG_RELAY && v == "true");
        assert!(has_relay_env, "WG_RELAY=true missing");
        // Should have 2 UDP ports (relay + wg)
        let udp_ports: Vec<_> = config.ports.iter()
            .filter(|p| p.protocol == PortProtocol::Udp)
            .collect();
        assert_eq!(udp_ports.len(), 2);
    }
}

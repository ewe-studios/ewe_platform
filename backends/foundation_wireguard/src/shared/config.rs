//! Configuration types, builder, and TOML loader (spec-55, feature 09).
//!
//! WHY: Three converging paths to instantiate a mesh node — a `wireguard!` macro
//! (compile-time), a programmatic builder (runtime), and `wireguard.toml` (file-based,
//! reloadable) — mirroring `foundation_proxy`'s tri-config pattern (decision 13).
//!
//! WHAT: [`WgConfig`] and sub-configs ([`NetworkConfig`], [`NodeConfig`],
//! [`DataPlaneConfig`], [`SecurityConfig`], [`RelayConfig`]) plus a consuming
//! [`WgConfigBuilder`] and TOML loading.
//!
//! HOW: All types derive `serde::Serialize + Deserialize`. Endpoints accept a bare
//! string OR a full table via a hand-written `Deserialize` impl (same `#[serde(untagged)]`
//! pattern as `foundation_proxy::BackendTarget`). Secrets are never defaulted
//! (`feedback_no_silent_defaults`).

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::error::{WgError, WgResult};
use super::keys::{NetworkId, WgSeed};
use super::membership::Capabilities;

// ---------------------------------------------------------------------------
// Serde helpers
// ---------------------------------------------------------------------------

/// Deserialise a `SocketAddr` from a bare string or a `{ host, port }` table.
fn deser_socket_addr<'de, D: serde::Deserializer<'de>>(d: D) -> Result<SocketAddr, D::Error> {
    use serde::de;
    use std::net::ToSocketAddrs;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Bare(String),
        Full { host: String, port: u16 },
    }

    match Repr::deserialize(d)? {
        Repr::Bare(s) => s
            .parse::<SocketAddr>()
            .or_else(|_| {
                // Try "host:port" via DNS for named endpoints (common in TOML).
                (s.as_str(), 0).to_socket_addrs().ok().and_then(|mut it| it.next())
                    .ok_or_else(|| de::Error::custom(format!("invalid socket address: {s}")))
            }),
        Repr::Full { host, port } => {
            let addr: std::net::IpAddr = host
                .parse()
                .map_err(|_| de::Error::custom(format!("invalid IP: {host}")))?;
            Ok(SocketAddr::new(addr, port))
        }
    }
}

/// Deserialise a `Vec<SocketAddr>` from a TOML array of bare strings or tables.
fn deser_socket_addrs<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<SocketAddr>, D::Error> {
    use serde::de;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Item {
        Bare(String),
        Full { host: String, port: u16 },
    }

    let items: Vec<Item> = Deserialize::deserialize(d)?;
    items
        .into_iter()
        .map(|i| match i {
            Item::Bare(s) => s.parse::<SocketAddr>().map_err(|e| {
                de::Error::custom(format!("{s}: {e}"))
            }),
            Item::Full { host, port } => {
                let ip: std::net::IpAddr = host
                    .parse()
                    .map_err(|_| de::Error::custom(format!("invalid IP: {host}")))?;
                Ok(SocketAddr::new(ip, port))
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Sub-config types
// ---------------------------------------------------------------------------

/// Network identity and bootstrap secret.
///
/// WHY: Everything needed to derive keys and reach the initial mesh members.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// The bootstrap seed (16 or 32 bytes, base64url). Must be provided — no silent default.
    pub seed: WgSeed,

    /// Network id. When `None` it is derived from the seed via blake2s.
    pub network_id: Option<NetworkId>,

    /// Seed endpoints to dial for the initial join. Empty ⇒ this is the first node.
    #[serde(default, deserialize_with = "deser_socket_addrs")]
    pub seed_endpoints: Vec<SocketAddr>,

    /// Optional seed TTL (unix seconds). After it expires, this node refuses new joins
    /// with this seed (decision 11).
    #[serde(default)]
    pub seed_expires_at: Option<u64>,
}

impl NetworkConfig {
    /// Derive or return the resolved network id.
    #[must_use]
    pub fn resolve_network_id(&self) -> NetworkId {
        self.network_id.unwrap_or_else(|| self.seed.derive_network_id())
    }
}

/// Node-local settings (ports, capabilities, persistence).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Outer WireGuard UDP socket bind address.
    #[serde(
        default = "default_udp_listen",
        deserialize_with = "deser_socket_addr"
    )]
    pub udp_listen: SocketAddr,

    /// TLS-PSK bootstrap listener bind address.
    #[serde(
        default = "default_bootstrap_listen",
        deserialize_with = "deser_socket_addr"
    )]
    pub bootstrap_listen: SocketAddr,

    /// Advertised capabilities.
    #[serde(default)]
    pub caps: Capabilities,

    /// VFS path for persisting the identity keypair (F09/F10 interplay).
    #[serde(default)]
    pub identity_path: Option<String>,
}

fn default_udp_listen() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0))
}

fn default_bootstrap_listen() -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0))
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            udp_listen: default_udp_listen(),
            bootstrap_listen: default_bootstrap_listen(),
            caps: Capabilities::default(),
            identity_path: None,
        }
    }
}

/// Data plane mode — which TCP/IP stack backs the overlay.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum DataPlaneMode {
    /// smoltcp userspace netstack (default — no privileges, cross-platform).
    #[serde(alias = "smoltcp")]
    Netstack,
    /// Kernel TUN device (Linux/Darwin, requires CAP_NET_ADMIN or TUN ownership).
    #[serde(alias = "tun")]
    #[cfg(not(target_family = "wasm"))]
    Tun { name: String, mtu: u32 },
}

impl Default for DataPlaneMode {
    fn default() -> Self {
        Self::Netstack
    }
}

/// Data plane settings (smoltcp netstack, kernel TUN).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPlaneConfig {
    /// Which data plane to use. Default: smoltcp netstack.
    #[serde(default)]
    pub mode: DataPlaneMode,

    /// Overlay MTU (inner L3, after WG overhead). Default 1380.
    #[serde(default = "default_mtu")]
    pub mtu: u16,

    /// WireGuard persistent-keepalive interval in seconds. Default 25.
    #[serde(default = "default_keepalive")]
    pub keepalive_secs: u16,
}

const fn default_mtu() -> u16 { 1380 }
const fn default_keepalive() -> u16 { 25 }

impl Default for DataPlaneConfig {
    fn default() -> Self {
        Self {
            mode: DataPlaneMode::default(),
            mtu: default_mtu(),
            keepalive_secs: default_keepalive(),
        }
    }
}

/// Security settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable optional app-layer mTLS over overlay sockets. Default: off.
    #[serde(default)]
    pub mtls: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self { mtls: false }
    }
}

/// Relay settings (feature 05).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    /// Advertise this node as a relay-capable member.
    #[serde(default)]
    pub advertise: bool,

    /// Maximum concurrent relay sessions. Default: 4096.
    #[serde(default = "default_max_relay_sessions")]
    pub max_sessions: u32,

    /// Per-session rate limit (packets per second). Default: 1000.
    #[serde(default = "default_rate_limit_pps")]
    pub rate_limit_pps: u32,

    /// Session idle timeout in seconds. Default: 120.
    #[serde(default = "default_idle_timeout_secs")]
    pub idle_timeout_secs: u64,
}

const fn default_max_relay_sessions() -> u32 { 4096 }
const fn default_rate_limit_pps() -> u32 { 1000 }
const fn default_idle_timeout_secs() -> u64 { 120 }

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            advertise: false,
            max_sessions: default_max_relay_sessions(),
            rate_limit_pps: default_rate_limit_pps(),
            idle_timeout_secs: default_idle_timeout_secs(),
        }
    }
}

// ---------------------------------------------------------------------------
// WgConfig — the convergence type
// ---------------------------------------------------------------------------

/// Complete configuration for one mesh node. Converged by the macro, the builder,
/// and the TOML loader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WgConfig {
    /// Network identity + bootstrap secret.
    pub network: NetworkConfig,

    /// Node-local bind addresses and capabilities.
    #[serde(default)]
    pub node: NodeConfig,

    /// Data plane settings.
    #[serde(default)]
    pub dataplane: DataPlaneConfig,

    /// Security toggles.
    #[serde(default)]
    pub security: SecurityConfig,

    /// Relay capability advertisement.
    #[serde(default)]
    pub relay: RelayConfig,
}

impl WgConfig {
    // ------------------------------------------------------------------
    // Convenience constructors (backward-compatible with F04 node.rs)
    // ------------------------------------------------------------------

    /// WHY: The first node stands up the network with nothing to join.
    ///
    /// WHAT: A seed config on loopback ephemeral ports.
    ///
    /// HOW: No `seed_endpoints`; both listeners bind `127.0.0.1:0`.
    #[must_use]
    pub fn seed(seed: WgSeed, network_id: NetworkId) -> Self {
        Self {
            network: NetworkConfig {
                seed,
                network_id: Some(network_id),
                seed_endpoints: Vec::new(),
                seed_expires_at: None,
            },
            node: NodeConfig::default(),
            dataplane: DataPlaneConfig::default(),
            security: SecurityConfig::default(),
            relay: RelayConfig::default(),
        }
    }

    /// WHY: Later nodes join an existing member.
    ///
    /// WHAT: A joiner config dialing `seed_endpoints`.
    ///
    /// HOW: Ephemeral loopback listeners; `seed_endpoints` supplies the join target(s).
    #[must_use]
    pub fn joiner(
        seed: WgSeed,
        network_id: NetworkId,
        seed_endpoints: Vec<SocketAddr>,
    ) -> Self {
        Self {
            network: NetworkConfig {
                seed,
                network_id: Some(network_id),
                seed_endpoints,
                seed_expires_at: None,
            },
            node: NodeConfig::default(),
            dataplane: DataPlaneConfig::default(),
            security: SecurityConfig::default(),
            relay: RelayConfig::default(),
        }
    }

    // ------------------------------------------------------------------
    // Field accessors (matching the flat struct WgNode::join expects)
    // ------------------------------------------------------------------

    /// The bootstrap seed (accessor — distinct from the [`Self::seed`] constructor).
    #[must_use]
    pub fn wg_seed(&self) -> &WgSeed {
        &self.network.seed
    }

    /// The resolved network id.
    #[must_use]
    pub fn network_id(&self) -> NetworkId {
        self.network.resolve_network_id()
    }

    /// Seed bootstrap endpoints to dial.
    #[must_use]
    pub fn seed_endpoints(&self) -> &[SocketAddr] {
        &self.network.seed_endpoints
    }

    /// UDP listen address.
    #[must_use]
    pub fn udp_listen(&self) -> SocketAddr {
        self.node.udp_listen
    }

    /// Bootstrap listener address.
    #[must_use]
    pub fn bootstrap_listen(&self) -> SocketAddr {
        self.node.bootstrap_listen
    }

    /// Advertised capabilities.
    #[must_use]
    pub fn caps(&self) -> Capabilities {
        self.node.caps
    }

    /// Optional seed expiry.
    #[must_use]
    pub fn seed_expires_at(&self) -> Option<u64> {
        self.network.seed_expires_at
    }

    // ------------------------------------------------------------------
    // Builder entry point
    // ------------------------------------------------------------------

    /// WHY: Start a programmatic builder from a seed.
    ///
    /// WHAT: Returns a [`WgConfigBuilder`] initialised with the seed + network_id.
    ///
    /// HOW: The seed and optional network_id are the only truly required fields.
    #[must_use]
    pub fn builder() -> WgConfigBuilder {
        WgConfigBuilder::default()
    }

    // ------------------------------------------------------------------
    // TOML loading
    // ------------------------------------------------------------------

    /// WHY: `wireguard.toml` is the ops-friendly path — reloadable, no recompilation.
    ///
    /// WHAT: Read a TOML file and deserialise into a [`WgConfig`].
    ///
    /// HOW: `std::fs::read_to_string` + `toml::from_str`. All errors map to
    /// [`WgError::Config`].
    ///
    /// # Errors
    /// [`WgError::Config`] if the file cannot be read or the TOML is malformed.
    pub fn load_file(path: impl AsRef<Path>) -> WgResult<Self> {
        let contents = std::fs::read_to_string(path.as_ref()).map_err(|e| {
            WgError::Config(format!("cannot read {}: {e}", path.as_ref().display()))
        })?;
        toml::from_str(&contents)
            .map_err(|e| WgError::Config(format!("invalid TOML in {}: {e}", path.as_ref().display())))
    }

    /// WHY: Build a config from environment variables — the self-assembly path for
    /// containers (F10).
    ///
    /// WHAT: Reads `WG_SECRET`, `WG_NETWORK` (optional), `WG_SEED_ENDPOINTS`
    /// (optional), and `WG_RELAY` (optional, "true"/"1" enables relay) from the
    /// process environment.
    ///
    /// HOW: `std::env::var`. Missing `WG_SECRET` is a hard error (no silent default —
    /// `feedback_no_silent_defaults`).
    ///
    /// # Errors
    /// [`WgError::Config`] if `WG_SECRET` is missing or the seed cannot be parsed.
    pub fn from_env() -> WgResult<Self> {
        let seed_str = std::env::var("WG_SECRET").map_err(|_| {
            WgError::Config("WG_SECRET not set in environment".into())
        })?;
        let seed = WgSeed::from_base64url(&seed_str)?;

        let network_id = if let Ok(hex) = std::env::var("WG_NETWORK") {
            Some(NetworkId::from_hex(&hex).map_err(|_| {
                WgError::Config(format!("invalid WG_NETWORK hex: {hex}"))
            })?)
        } else {
            None
        };

        let seed_endpoints: Vec<SocketAddr> = if let Ok(eps) = std::env::var("WG_SEED_ENDPOINTS") {
            eps.split(',')
                .filter(|s| !s.is_empty())
                .map(|s| {
                    s.trim().parse::<SocketAddr>().map_err(|e| {
                        WgError::Config(format!("invalid WG_SEED_ENDPOINTS item '{s}': {e}"))
                    })
                })
                .collect::<WgResult<Vec<_>>>()?
        } else {
            Vec::new()
        };

        let relay_advertise = std::env::var("WG_RELAY")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        Ok(Self {
            network: NetworkConfig {
                seed,
                network_id,
                seed_endpoints,
                seed_expires_at: None,
            },
            node: NodeConfig::default(),
            dataplane: DataPlaneConfig::default(),
            security: SecurityConfig::default(),
            relay: RelayConfig {
                advertise: relay_advertise,
                ..RelayConfig::default()
            },
        })
    }
}

// ---------------------------------------------------------------------------
// Programmatic builder
// ---------------------------------------------------------------------------

/// WHY: Runtime construction — full Rust, dynamic values, no file or macro needed.
///
/// WHAT: A consuming builder that validates on [`build`](Self::build).
///
/// HOW: Every setter takes `mut self` and returns `Self`. No `&mut self` methods.
#[derive(Debug, Default)]
pub struct WgConfigBuilder {
    seed: Option<WgSeed>,
    network_id: Option<NetworkId>,
    seed_endpoints: Vec<SocketAddr>,
    seed_expires_at: Option<u64>,
    udp_listen: Option<SocketAddr>,
    bootstrap_listen: Option<SocketAddr>,
    caps: Option<Capabilities>,
    identity_path: Option<String>,
    mtu: Option<u16>,
    keepalive_secs: Option<u16>,
    mtls: bool,
    relay_advertise: bool,
    relay_max_sessions: u32,
    relay_rate_limit_pps: u32,
    relay_idle_timeout_secs: u64,
    dataplane_mode: Option<DataPlaneMode>,
}

impl WgConfigBuilder {
    /// Set the bootstrap seed (required).
    #[must_use]
    pub fn seed(mut self, seed: WgSeed) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Set the network id. If omitted, derived from the seed.
    #[must_use]
    pub fn network_id(mut self, id: NetworkId) -> Self {
        self.network_id = Some(id);
        self
    }

    /// Add a seed endpoint for the initial bootstrap join.
    #[must_use]
    pub fn seed_endpoint(mut self, addr: SocketAddr) -> Self {
        self.seed_endpoints.push(addr);
        self
    }

    /// Set all seed endpoints at once.
    #[must_use]
    pub fn seed_endpoints(mut self, list: Vec<SocketAddr>) -> Self {
        self.seed_endpoints = list;
        self
    }

    /// Set the seed TTL (unix seconds; decision 11).
    #[must_use]
    pub fn seed_expires_at(mut self, ts: u64) -> Self {
        self.seed_expires_at = Some(ts);
        self
    }

    /// Bind the WireGuard UDP socket to this address.
    #[must_use]
    pub fn udp_listen(mut self, addr: SocketAddr) -> Self {
        self.udp_listen = Some(addr);
        self
    }

    /// Bind the TLS-PSK bootstrap listener to this address.
    #[must_use]
    pub fn bootstrap_listen(mut self, addr: SocketAddr) -> Self {
        self.bootstrap_listen = Some(addr);
        self
    }

    /// Set advertised capabilities.
    #[must_use]
    pub fn caps(mut self, caps: Capabilities) -> Self {
        self.caps = Some(caps);
        self
    }

    /// Set the VFS path for identity persistence.
    #[must_use]
    pub fn identity_path(mut self, path: impl Into<String>) -> Self {
        self.identity_path = Some(path.into());
        self
    }

    /// Set the overlay MTU (inner L3, after WG overhead).
    #[must_use]
    pub fn mtu(mut self, mtu: u16) -> Self {
        self.mtu = Some(mtu);
        self
    }

    /// Set the WireGuard keepalive interval in seconds.
    #[must_use]
    pub fn keepalive_secs(mut self, secs: u16) -> Self {
        self.keepalive_secs = Some(secs);
        self
    }

    /// Enable optional app-layer mTLS.
    #[must_use]
    pub fn mtls(mut self, on: bool) -> Self {
        self.mtls = on;
        self
    }

    /// Advertise this node as a relay (feature 05).
    #[must_use]
    pub fn relay_advertise(mut self, on: bool) -> Self {
        self.relay_advertise = on;
        self
    }

    /// Set the maximum number of relay sessions.
    #[must_use]
    pub fn relay_max_sessions(mut self, n: u32) -> Self {
        self.relay_max_sessions = n;
        self
    }

    /// Set the per-session relay rate limit (packets per second).
    #[must_use]
    pub fn relay_rate_limit_pps(mut self, pps: u32) -> Self {
        self.relay_rate_limit_pps = pps;
        self
    }

    /// Set the relay session idle timeout in seconds.
    #[must_use]
    pub fn relay_idle_timeout_secs(mut self, secs: u64) -> Self {
        self.relay_idle_timeout_secs = secs;
        self
    }

    /// Use the smoltcp userspace netstack (default — no privileges, cross-platform).
    #[must_use]
    pub fn dataplane_smoltcp(mut self) -> Self {
        self.dataplane_mode = Some(DataPlaneMode::Netstack);
        self
    }

    /// Use a kernel TUN device. Requires CAP_NET_ADMIN or TUN ownership.
    #[must_use]
    pub fn dataplane_tun(mut self, name: &str, mtu: u32) -> Self {
        self.dataplane_mode = Some(DataPlaneMode::Tun {
            name: name.to_string(),
            mtu,
        });
        self
    }

    /// WHY: Validate and produce the final [`WgConfig`].
    ///
    /// WHAT: Checks that a seed was provided (no silent default — see
    /// `feedback_no_silent_defaults`), fills in defaults, and returns the
    /// assembled config.
    ///
    /// HOW: `build` is infallible except for the missing-seed check; defaults
    /// are filled with `..Default::default()` for each sub-config.
    ///
    /// # Errors
    /// [`WgError::Config`] if no seed was provided.
    pub fn build(self) -> WgResult<WgConfig> {
        let seed = self
            .seed
            .ok_or_else(|| WgError::Config("seed is required (no silent default)".into()))?;

        let node = NodeConfig {
            udp_listen: self.udp_listen.unwrap_or_else(default_udp_listen),
            bootstrap_listen: self
                .bootstrap_listen
                .unwrap_or_else(default_bootstrap_listen),
            caps: self.caps.unwrap_or_default(),
            identity_path: self.identity_path,
        };

        let dataplane = DataPlaneConfig {
            mode: self.dataplane_mode.unwrap_or_default(),
            mtu: self.mtu.unwrap_or_else(default_mtu),
            keepalive_secs: self.keepalive_secs.unwrap_or_else(default_keepalive),
        };

        let security = SecurityConfig { mtls: self.mtls };

        let relay = RelayConfig {
            advertise: self.relay_advertise,
            max_sessions: self.relay_max_sessions,
            rate_limit_pps: self.relay_rate_limit_pps,
            idle_timeout_secs: self.relay_idle_timeout_secs,
        };

        Ok(WgConfig {
            network: NetworkConfig {
                seed,
                network_id: self.network_id,
                seed_endpoints: self.seed_endpoints,
                seed_expires_at: self.seed_expires_at,
            },
            node,
            dataplane,
            security,
            relay,
        })
    }
}

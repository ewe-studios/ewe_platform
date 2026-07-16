//! Proxy configuration types — shared by all three config paths.
//!
//! WHY: The macro, programmatic builder, and `proxy.toml` file all converge on
//! these types. They must be ergonomic for hand-written Rust and forgiving for
//! TOML (a bare backend URL string must deserialize into a full `BackendTarget`).
//!
//! WHAT: `ProxyConfig`, `ServiceConfig`, `SslConfig`, `HealthCheckConfig`,
//! `BackendTarget`, `BackendState`, and the `BackendProtocol` inferred from a
//! backend URL scheme.
//!
//! HOW: Standard `serde` derives, plus a hand-written `Deserialize` for
//! `BackendTarget` that accepts either a bare string or a table.

use derive_more::Display;
use foundation_iogate::ServerIo;
use serde::{Deserialize, Deserializer, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Runtime hooks for ACME provisioning (Decision 18, F15).
///
/// Not part of the serialisable config: the DNS-01 setter is a callback (publish
/// a TXT record for a domain), typically backed by `foundation_deployment_cloudflare`.
#[derive(Clone)]
pub struct AcmeRuntime {
    /// Override the CA directory URL (defaults to Let's Encrypt production). A
    /// private CA or a local test server (Pebble / the F15 mock) sets this.
    pub directory_url: Option<String>,
    /// Publish a DNS-01 TXT record: `(record_name, txt_value) -> Result`.
    pub dns01_setter: crate::acme::Dns01Setter,
}

impl std::fmt::Debug for AcmeRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcmeRuntime")
            .field("directory_url", &self.directory_url)
            .field("dns01_setter", &"<callback>")
            .finish()
    }
}

/// Top-level proxy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub domain: String,
    pub public_ip: String,
    pub ssl: SslConfig,
    /// Address the proxy's HTTP front end binds to. Defaults to `0.0.0.0:80`
    /// when unset; tests bind `127.0.0.1:0` for an ephemeral port.
    #[serde(default)]
    pub bind_addr: Option<String>,
    #[serde(default)]
    pub services: Vec<ServiceConfig>,
    /// Path to a Unix-domain control socket (Decision 20). When set,
    /// `ProxyServer` binds it and serves the JSON-RPC admin commands
    /// (`list`/`status`/`drain`/`pause`/`activate`). `None` disables the admin
    /// interface.
    #[serde(default)]
    pub control_socket: Option<String>,
    /// Directory for persisted proxy state (Decision 21, F14). When set,
    /// `ProxyServer` restores backend drain/pause state on start and writes
    /// admin changes through so they survive a restart. `None` disables it.
    #[serde(default)]
    pub state_dir: Option<String>,
    /// Programmatic ACME runtime hooks (Decision 18, F15) — the DNS-01 setter
    /// and an optional directory-URL override. Required when the SSL provider is
    /// `LetsEncrypt`. Not serialised — set it via the builder.
    #[serde(skip)]
    pub acme: Option<AcmeRuntime>,
    /// UDP address for the HTTP/3 (QUIC) front end (Decision 27, F19). When set
    /// (and TLS is enabled), `ProxyServer` also serves H3 on this address using
    /// the same certificate. Requires the `quic` feature. `None` disables H3.
    #[serde(default)]
    pub h3_bind: Option<String>,
    /// I/O mode for both the accept path and the dialed upstream legs (F50). The
    /// upstream mode tracks the front-end mode: `Completion` reads both legs from
    /// the io_uring inbox and writes via `IORING_OP_SEND`. Defaults to `Std`
    /// (today's `read(2)`/`write(2)`), so nothing changes unless asked. Not
    /// serialised — set it via the programmatic builder.
    #[serde(skip)]
    pub io_mode: ServerIo,
}

impl ProxyConfig {
    #[must_use]
    pub fn new(domain: &str, public_ip: &str) -> Self {
        Self {
            domain: domain.to_string(),
            public_ip: public_ip.to_string(),
            ssl: SslConfig::default(),
            bind_addr: None,
            services: Vec::new(),
            control_socket: None,
            state_dir: None,
            acme: None,
            h3_bind: None,
            io_mode: ServerIo::default(),
        }
    }

    /// Set the I/O mode for the front end and the dialed upstream legs (F50).
    #[must_use]
    pub fn io_mode(mut self, mode: ServerIo) -> Self {
        self.io_mode = mode;
        self
    }

    /// Set the front-end bind address (e.g. `127.0.0.1:0`).
    #[must_use]
    pub fn bind(mut self, addr: &str) -> Self {
        self.bind_addr = Some(addr.to_string());
        self
    }

    /// Enable the Unix-domain control socket at `path` (Decision 20).
    #[must_use]
    pub fn control_socket(mut self, path: &str) -> Self {
        self.control_socket = Some(path.to_string());
        self
    }

    /// Enable state persistence under `dir` (Decision 21). Backend drain/pause
    /// state is restored on start and written through on admin changes.
    #[must_use]
    pub fn persist_to(mut self, dir: &str) -> Self {
        self.state_dir = Some(dir.to_string());
        self
    }

    /// Serve HTTP/3 (QUIC) on `addr` (UDP) as well (Decision 27, F19). Needs TLS
    /// and the `quic` feature.
    #[must_use]
    pub fn h3_bind(mut self, addr: &str) -> Self {
        self.h3_bind = Some(addr.to_string());
        self
    }

    /// Attach the ACME runtime hooks (Decision 18, F15) — required when the SSL
    /// provider is `LetsEncrypt`. `dns01_setter` publishes DNS-01 TXT records;
    /// `directory_url` overrides the CA (defaults to Let's Encrypt production).
    #[must_use]
    pub fn acme(
        mut self,
        dns01_setter: crate::acme::Dns01Setter,
        directory_url: Option<String>,
    ) -> Self {
        self.acme = Some(AcmeRuntime { directory_url, dns01_setter });
        self
    }

    #[must_use]
    pub fn ssl(mut self, ssl: SslConfig) -> Self {
        self.ssl = ssl;
        self
    }

    #[must_use]
    pub fn service(mut self, svc: ServiceConfig) -> Self {
        self.services.push(svc);
        self
    }

    #[must_use]
    pub fn build(self) -> Self {
        self
    }

    /// Load from a `proxy.toml` file.
    ///
    /// # Errors
    /// Returns `ProxyError::Config` if the file cannot be read or parsed.
    pub fn load_file(path: &str) -> Result<Self, ProxyError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| ProxyError::Config(format!("read {path}: {e}")))?;
        toml::from_str(&contents).map_err(|e| ProxyError::Config(format!("parse {path}: {e}")))
    }

    /// Start the proxy from this configuration.
    ///
    /// # Errors
    /// Returns `ProxyError` if validation fails or the listener cannot bind.
    pub fn start(self) -> Result<crate::server::ProxyServer, ProxyError> {
        crate::server::ProxyServer::start(self)
    }
}

/// SSL configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslConfig {
    pub provider: SslProvider,
}

impl SslConfig {
    #[must_use]
    pub fn lets_encrypt(email: &str) -> Self {
        Self {
            provider: SslProvider::LetsEncrypt {
                email: email.to_string(),
            },
        }
    }

    #[must_use]
    pub fn cloudflare(zone_id: &str) -> Self {
        Self {
            provider: SslProvider::Cloudflare {
                zone_id: zone_id.to_string(),
            },
        }
    }

    #[must_use]
    pub fn static_cert(cert: &str, key: &str) -> Self {
        Self {
            provider: SslProvider::Static {
                cert: PathBuf::from(cert),
                key: PathBuf::from(key),
            },
        }
    }
}

impl Default for SslConfig {
    fn default() -> Self {
        Self {
            provider: SslProvider::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SslProvider {
    LetsEncrypt { email: String },
    Cloudflare { zone_id: String },
    Static { cert: PathBuf, key: PathBuf },
    None,
}

/// A service fronted by the proxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub host: String,
    #[serde(default)]
    pub path_prefix: Option<String>,
    pub backends: Vec<BackendTarget>,
    #[serde(default)]
    pub health_check: Option<HealthCheckConfig>,
}

impl ServiceConfig {
    #[must_use]
    pub fn new(name: &str, host: &str) -> Self {
        Self {
            name: name.to_string(),
            host: host.to_string(),
            path_prefix: None,
            backends: Vec::new(),
            health_check: None,
        }
    }

    /// Set a path prefix (e.g. `/api`) this service matches under its host.
    #[must_use]
    pub fn path_prefix(mut self, prefix: &str) -> Self {
        self.path_prefix = Some(prefix.to_string());
        self
    }

    /// Add a backend by bare URL (default weight and `max_connections`).
    #[must_use]
    pub fn backend(mut self, url: &str) -> Self {
        self.backends.push(BackendTarget::new(url));
        self
    }

    /// Add a fully-specified backend target.
    #[must_use]
    pub fn backend_target(mut self, target: BackendTarget) -> Self {
        self.backends.push(target);
        self
    }

    #[must_use]
    pub fn health_check(mut self, path: &str, interval: Duration) -> Self {
        self.health_check = Some(HealthCheckConfig {
            path: path.to_string(),
            interval,
            ..HealthCheckConfig::default()
        });
        self
    }

    /// Set the full health-check configuration.
    #[must_use]
    pub fn health_check_config(mut self, config: HealthCheckConfig) -> Self {
        self.health_check = Some(config);
        self
    }
}

/// Which wire protocol a backend speaks, inferred from its URL scheme.
///
/// WHY: Decision 14 §"Protocol inference" — the scheme selects the protocol, so
/// there is no separate `proto` config field. `http`/`https` mean an HTTP
/// reverse proxy (headers, WebSocket upgrade, health checks); `tcp` means raw
/// byte-level passthrough (RDP, VNC, noVNC — no HTTP semantics).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum BackendProtocol {
    #[display("http")]
    Http,
    #[display("https")]
    Https,
    #[display("tcp")]
    Tcp,
    #[display("udp")]
    Udp,
}

/// Backend target — where traffic is routed.
///
/// Deserializes from either a bare string (`"http://host:3000"`) or a table
/// (`{ url = "...", weight = 2, max_connections = 64 }`). A bare string takes
/// the default weight (1) and `max_connections` (1024).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BackendTarget {
    pub url: String,
    pub weight: u32,
    pub max_connections: u32,
}

impl BackendTarget {
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            weight: default_weight(),
            max_connections: default_max_conns(),
        }
    }

    #[must_use]
    pub fn with_weight(mut self, weight: u32) -> Self {
        self.weight = weight;
        self
    }

    #[must_use]
    pub fn with_max_connections(mut self, max_connections: u32) -> Self {
        self.max_connections = max_connections;
        self
    }

    /// Infer the wire protocol from the URL scheme.
    ///
    /// WHAT: `http://` → `Http`, `https://` → `Https`, `tcp://` → `Tcp`,
    /// `udp://` → `Udp`. Anything with no recognised scheme defaults to `Http`.
    #[must_use]
    pub fn protocol(&self) -> BackendProtocol {
        let lower = self.url.to_ascii_lowercase();
        if lower.starts_with("udp://") {
            BackendProtocol::Udp
        } else if lower.starts_with("tcp://") {
            BackendProtocol::Tcp
        } else if lower.starts_with("https://") {
            BackendProtocol::Https
        } else {
            BackendProtocol::Http
        }
    }

    /// The `host:port` authority, stripped of scheme and path.
    ///
    /// WHAT: `tcp://127.0.0.1:9000/x` → `127.0.0.1:9000`. Used by the TCP
    /// passthrough path, which connects to a raw socket address.
    #[must_use]
    pub fn authority(&self) -> String {
        let without_scheme = self
            .url
            .split_once("://")
            .map_or(self.url.as_str(), |(_, rest)| rest);
        without_scheme
            .split(['/', '?'])
            .next()
            .unwrap_or(without_scheme)
            .to_string()
    }
}

fn default_weight() -> u32 {
    1
}
fn default_max_conns() -> u32 {
    1024
}

impl<'de> Deserialize<'de> for BackendTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Accept either a bare string or a full table. `untagged` tries each
        // variant in order, so a plain TOML string maps to `Bare` and a table
        // to `Full`.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Bare(String),
            Full {
                url: String,
                #[serde(default = "default_weight")]
                weight: u32,
                #[serde(default = "default_max_conns")]
                max_connections: u32,
            },
        }

        Ok(match Repr::deserialize(deserializer)? {
            Repr::Bare(url) => BackendTarget::new(url),
            Repr::Full {
                url,
                weight,
                max_connections,
            } => BackendTarget {
                url,
                weight,
                max_connections,
            },
        })
    }
}

/// Lifecycle state of a backend in the rotation.
///
/// WHY: Zero-downtime deploys (stage 3) drain old backends and pause misbehaving
/// ones. The load balancer only ever selects `Active` backends; `Draining` and
/// `Paused` are excluded from new traffic while in-flight requests finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display)]
#[serde(rename_all = "lowercase")]
pub enum BackendState {
    #[display("active")]
    Active,
    #[display("draining")]
    Draining,
    #[display("paused")]
    Paused,
}

impl Default for BackendState {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub path: String,
    #[serde(with = "duration_secs")]
    pub interval: Duration,
    #[serde(with = "duration_secs")]
    pub timeout: Duration,
    pub healthy_threshold: u32,
    pub unhealthy_threshold: u32,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            path: "/".to_string(),
            interval: Duration::from_secs(5),
            timeout: Duration::from_secs(2),
            healthy_threshold: 2,
            unhealthy_threshold: 3,
        }
    }
}

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;
    pub fn serialize<S>(d: &Duration, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_u64(d.as_secs())
    }
    pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs: u64 = Deserialize::deserialize(d)?;
        Ok(Duration::from_secs(secs))
    }
}

#[derive(Debug, Display)]
pub enum ProxyError {
    #[display("Configuration error: {_0}")]
    Config(String),
    #[display("DNS error: {_0}")]
    Dns(String),
    #[display("SSL error: {_0}")]
    Ssl(String),
    #[display("Startup error: {_0}")]
    Startup(String),
}

impl std::error::Error for ProxyError {}

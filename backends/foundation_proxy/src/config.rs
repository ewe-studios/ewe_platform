//! Proxy configuration types — shared by all three config paths.

use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Top-level proxy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub domain: String,
    pub public_ip: String,
    pub ssl: SslConfig,
    #[serde(default)]
    pub services: Vec<ServiceConfig>,
}

impl ProxyConfig {
    #[must_use]
    pub fn new(domain: &str, public_ip: &str) -> Self {
        Self {
            domain: domain.to_string(),
            public_ip: public_ip.to_string(),
            ssl: SslConfig::default(),
            services: Vec::new(),
        }
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
    pub fn build(self) -> Self { self }

    /// Load from a `proxy.toml` file.
    pub fn load_file(path: &str) -> Result<Self, ProxyError> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| ProxyError::Config(format!("read {path}: {e}")))?;
        toml::from_str(&contents)
            .map_err(|e| ProxyError::Config(format!("parse {path}: {e}")))
    }

    pub async fn start(self) -> Result<crate::server::ProxyServer, ProxyError> {
        crate::server::ProxyServer::start(self).await
    }
}

/// SSL configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SslConfig { pub provider: SslProvider }

impl SslConfig {
    #[must_use]
    pub fn lets_encrypt(email: &str) -> Self {
        Self { provider: SslProvider::LetsEncrypt { email: email.to_string() } }
    }

    #[must_use]
    pub fn cloudflare(zone_id: &str) -> Self {
        Self { provider: SslProvider::Cloudflare { zone_id: zone_id.to_string() } }
    }

    #[must_use]
    pub fn static_cert(cert: &str, key: &str) -> Self {
        Self { provider: SslProvider::Static { cert: PathBuf::from(cert), key: PathBuf::from(key) } }
    }
}

impl Default for SslConfig {
    fn default() -> Self { Self { provider: SslProvider::None } }
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
    pub backends: Vec<String>,
    #[serde(default)]
    pub health_check: Option<HealthCheckConfig>,
}

impl ServiceConfig {
    #[must_use]
    pub fn new(name: &str, host: &str) -> Self {
        Self {
            name: name.to_string(), host: host.to_string(),
            path_prefix: None, backends: Vec::new(), health_check: None,
        }
    }

    #[must_use]
    pub fn backend(mut self, url: &str) -> Self {
        self.backends.push(url.to_string());
        self
    }

    #[must_use]
    pub fn health_check(mut self, path: &str, interval: Duration) -> Self {
        self.health_check = Some(HealthCheckConfig {
            path: path.to_string(), interval, timeout: Duration::from_secs(2),
            healthy_threshold: 2, unhealthy_threshold: 3,
        });
        self
    }
}

/// Backend target — where traffic is routed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendTarget {
    pub url: String,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default = "default_max_conns")]
    pub max_connections: u32,
}

fn default_weight() -> u32 { 1 }
fn default_max_conns() -> u32 { 1024 }

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

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;
    pub fn serialize<S>(d: &Duration, s: S) -> Result<S::Ok, S::Error>
    where S: Serializer { s.serialize_u64(d.as_secs()) }
    pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where D: Deserializer<'de> {
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

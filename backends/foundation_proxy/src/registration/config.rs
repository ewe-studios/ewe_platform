//! Registration authority settings.
//!
//! WHY: every size, timeout and policy limit of the registration authority is
//! configuration, never a constant in the code path (root rule 8). The operator
//! writes them in `proxy.toml` or sets them with the builder.
//!
//! WHAT: [`RegistrationConfig`] (listeners, lease timing, limits) and
//! [`RegistrationPolicy`] (the addresses and routes that a sidecar may
//! register).
//!
//! HOW: plain `serde` derives with `deny_unknown_fields`, so the TOML loader
//! refuses a misspelled key instead of silently ignoring it. Durations are
//! written as whole milliseconds.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use ipnet::IpNet;
use serde::{Deserialize, Serialize};

/// The registration authority's settings.
///
/// A `ProxyConfig` with `registration = None` turns the whole feature off.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationConfig {
    /// The mutual-TLS (mTLS) listener, for example `"10.0.0.2:9443"`. `None`
    /// means no network listener. The enrollment listener binds on the same
    /// host on an ephemeral port unless [`Self::enrollment_bind`] sets one.
    pub bind: Option<String>,
    /// The address of the enrollment listener, which uses server-auth TLS only.
    /// `None` = the host of [`Self::bind`] on an ephemeral port.
    #[serde(default)]
    pub enrollment_bind: Option<String>,
    /// Optional same-machine Unix socket (section 5.5). `None` turns it off.
    #[serde(default)]
    pub unix_socket: Option<PathBuf>,
    /// How often sidecars report. Default 10 s: the default `periodSeconds`
    /// of a Kubernetes probe.
    #[serde(default = "default_report_interval", with = "duration_millis")]
    pub report_interval: Duration,
    /// A lease with no report for this long ends. Default 30 s: three missed
    /// reports, the default Kubernetes `failureThreshold` of 3.
    #[serde(default = "default_lease_ttl", with = "duration_millis")]
    pub lease_ttl: Duration,
    /// Stale backends are removed after this. Default 300 s: the default
    /// Kubernetes `pod-eviction-timeout`.
    #[serde(default = "default_stale_remove_after", with = "duration_millis")]
    pub stale_remove_after: Duration,
    /// The longest drain of a removed address. Default 30 s: the default
    /// Kubernetes `terminationGracePeriodSeconds`.
    #[serde(default = "default_drain_timeout", with = "duration_millis")]
    pub drain_timeout: Duration,
    /// How often the sweeper runs. Default 1 s.
    #[serde(default = "default_sweep_interval", with = "duration_millis")]
    pub sweep_interval: Duration,
    /// Most backends in one session. Default 256: one machine with many apps.
    #[serde(default = "default_max_backends_per_session")]
    pub max_backends_per_session: usize,
    /// Largest registration request body. Default 64 KiB: 256 backends of about
    /// 200 bytes each, with room.
    #[serde(default = "default_max_request_bytes")]
    pub max_request_bytes: usize,
    /// Sidecar certificate lifetime. Default 30 days.
    #[serde(default = "default_client_cert_lifetime", with = "duration_millis")]
    pub client_cert_lifetime: Duration,
    /// Where the registration certificate authority (CA) key, the CA
    /// certificate and the trusted-client store live. `None` uses
    /// `Foundation::state_dir()`'s `proxy_registration` directory.
    #[serde(default)]
    pub state_dir: Option<PathBuf>,
    /// The address and route policy.
    #[serde(default)]
    pub policy: RegistrationPolicy,
}

impl Default for RegistrationConfig {
    fn default() -> Self {
        Self {
            bind: None,
            enrollment_bind: None,
            unix_socket: None,
            report_interval: default_report_interval(),
            lease_ttl: default_lease_ttl(),
            stale_remove_after: default_stale_remove_after(),
            drain_timeout: default_drain_timeout(),
            sweep_interval: default_sweep_interval(),
            max_backends_per_session: default_max_backends_per_session(),
            max_request_bytes: default_max_request_bytes(),
            client_cert_lifetime: default_client_cert_lifetime(),
            state_dir: None,
            policy: RegistrationPolicy::default(),
        }
    }
}

impl RegistrationConfig {
    /// The documented defaults of [`Default`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the mutual-TLS registration listener.
    #[must_use]
    pub fn with_bind(mut self, bind: impl Into<String>) -> Self {
        self.bind = Some(bind.into());
        self
    }

    /// Set the enrollment listener.
    #[must_use]
    pub fn with_enrollment_bind(mut self, bind: impl Into<String>) -> Self {
        self.enrollment_bind = Some(bind.into());
        self
    }

    /// Set the same-machine Unix socket.
    #[must_use]
    pub fn with_unix_socket(mut self, path: impl Into<PathBuf>) -> Self {
        self.unix_socket = Some(path.into());
        self
    }

    /// Set [`Self::report_interval`]. Default 10 s.
    #[must_use]
    pub fn with_report_interval(mut self, interval: Duration) -> Self {
        self.report_interval = interval;
        self
    }

    /// Set [`Self::lease_ttl`]. Default 30 s.
    #[must_use]
    pub fn with_lease_ttl(mut self, ttl: Duration) -> Self {
        self.lease_ttl = ttl;
        self
    }

    /// Set [`Self::stale_remove_after`]. Default 300 s.
    #[must_use]
    pub fn with_stale_remove_after(mut self, after: Duration) -> Self {
        self.stale_remove_after = after;
        self
    }

    /// Set [`Self::drain_timeout`]. Default 30 s.
    #[must_use]
    pub fn with_drain_timeout(mut self, timeout: Duration) -> Self {
        self.drain_timeout = timeout;
        self
    }

    /// Set [`Self::sweep_interval`]. Default 1 s.
    #[must_use]
    pub fn with_sweep_interval(mut self, interval: Duration) -> Self {
        self.sweep_interval = interval;
        self
    }

    /// Set [`Self::max_backends_per_session`]. Default 256.
    #[must_use]
    pub fn with_max_backends_per_session(mut self, max: usize) -> Self {
        self.max_backends_per_session = max;
        self
    }

    /// Set [`Self::max_request_bytes`]. Default 64 KiB.
    #[must_use]
    pub fn with_max_request_bytes(mut self, max: usize) -> Self {
        self.max_request_bytes = max;
        self
    }

    /// Set [`Self::client_cert_lifetime`]. Default 30 days.
    #[must_use]
    pub fn with_client_cert_lifetime(mut self, lifetime: Duration) -> Self {
        self.client_cert_lifetime = lifetime;
        self
    }

    /// Set the state directory of the CA and the trust store.
    #[must_use]
    pub fn with_state_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.state_dir = Some(dir.into());
        self
    }

    /// Set the address and route policy.
    #[must_use]
    pub fn with_policy(mut self, policy: RegistrationPolicy) -> Self {
        self.policy = policy;
        self
    }
}

/// What sidecars may register. Part of [`RegistrationConfig`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationPolicy {
    /// Backend addresses must be inside one of these networks. Default:
    /// loopback and the private ranges. A Domain Name System (DNS) name is
    /// resolved and every result must match.
    #[serde(default = "default_allowed_networks")]
    pub allowed_networks: Vec<IpNet>,
    /// Route hosts for which a sidecar may create a new service. Default
    /// empty: a sidecar may join only services in the config file.
    #[serde(default)]
    pub allowed_route_hosts: Vec<String>,
    /// The services that each `host_id` may join. `None` = any.
    #[serde(default)]
    pub services_by_host: Option<BTreeMap<String, Vec<String>>>,
}

impl Default for RegistrationPolicy {
    fn default() -> Self {
        Self {
            allowed_networks: default_allowed_networks(),
            allowed_route_hosts: Vec::new(),
            services_by_host: None,
        }
    }
}

impl RegistrationPolicy {
    /// The documented defaults of [`Default`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the allowed networks.
    #[must_use]
    pub fn with_allowed_networks(mut self, networks: Vec<IpNet>) -> Self {
        self.allowed_networks = networks;
        self
    }

    /// Add one allowed network.
    #[must_use]
    pub fn allow_network(mut self, network: IpNet) -> Self {
        self.allowed_networks.push(network);
        self
    }

    /// Replace the route hosts that a sidecar may create.
    #[must_use]
    pub fn with_allowed_route_hosts(mut self, hosts: Vec<String>) -> Self {
        self.allowed_route_hosts = hosts;
        self
    }

    /// Restrict each `host_id` to a set of service names.
    #[must_use]
    pub fn with_services_by_host(mut self, services: BTreeMap<String, Vec<String>>) -> Self {
        self.services_by_host = Some(services);
        self
    }
}

/// Loopback and the private ranges: `127.0.0.0/8`, `::1/128`, `10.0.0.0/8`,
/// `172.16.0.0/12`, `192.168.0.0/16`, `fc00::/7`.
fn default_allowed_networks() -> Vec<IpNet> {
    [
        "127.0.0.0/8",
        "::1/128",
        "10.0.0.0/8",
        "172.16.0.0/12",
        "192.168.0.0/16",
        "fc00::/7",
    ]
    .iter()
    .filter_map(|net| net.parse().ok())
    .collect()
}

fn default_report_interval() -> Duration {
    Duration::from_secs(10)
}

fn default_lease_ttl() -> Duration {
    Duration::from_secs(30)
}

fn default_stale_remove_after() -> Duration {
    Duration::from_secs(300)
}

fn default_drain_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_sweep_interval() -> Duration {
    Duration::from_secs(1)
}

const fn default_max_backends_per_session() -> usize {
    256
}

const fn default_max_request_bytes() -> usize {
    64 * 1024
}

fn default_client_cert_lifetime() -> Duration {
    Duration::from_hours(30 * 24)
}

/// Serialise a `Duration` as whole milliseconds.
mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u128(duration.as_millis())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis: u64 = Deserialize::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

//! The registrar configuration: one binary, two modes.
//!
//! WHY: every path, timeout and size limit is a field (root rule 8), and the
//! sidecar reads them from flags or `EWE_REGISTRAR_*` environment variables
//! (spec 82 feature 01, section 5).
//!
//! WHAT: [`RegistrarConfig`], [`RegistrarMode`] and the command-line parser.
//!
//! HOW: `clap` parses the flags and the matching environment variables. The
//! library takes the parsed value; only the binary calls the parser.

use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;

use crate::error::RegistrarError;

/// How the sidecar runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrarMode {
    /// One sidecar for the machine. It watches every entry.
    Host,
    /// One sidecar next to one app, or next to the listed `entries`.
    Container,
}

impl std::str::FromStr for RegistrarMode {
    type Err = RegistrarError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "host" => Ok(Self::Host),
            "container" => Ok(Self::Container),
            other => Err(RegistrarError::Config(format!(
                "mode must be \"host\" or \"container\", not {other:?}"
            ))),
        }
    }
}

/// The registrar configuration.
#[derive(Debug, Clone)]
pub struct RegistrarConfig {
    /// How the sidecar runs.
    pub mode: RegistrarMode,
    /// The stable name of this machine or pod. Example: `host-17`.
    pub host_id: String,
    /// The manifest file. Default `/etc/ewe/manifest.json`.
    pub manifest_path: PathBuf,
    /// The registration authority: `https://10.0.0.2:9443`, or
    /// `unix:///run/ewe/registration.sock`.
    pub proxy: String,
    /// The enrollment listener: `https://10.0.0.2:9444`. It uses server-auth
    /// Transport Layer Security (TLS). It is required when there is no valid
    /// certificate and `proxy` is not a Unix socket.
    pub enrollment_url: Option<String>,
    /// An optional SHA-256 fingerprint pin for the registration certificate.
    pub proxy_fingerprint_sha256: Option<String>,
    /// The enrollment token file. Read at the first start, then deleted.
    pub enrollment_token_path: Option<PathBuf>,
    /// Certificate, key and session data. Default `/var/lib/ewe/registrar`.
    pub state_dir: PathBuf,
    /// Host mode: the address that the proxy dials. Required in host mode.
    pub advertise_host: Option<String>,
    /// Container mode with Docker: the network to read the container IP from.
    pub docker_network: Option<String>,
    /// Container mode: the entries that this sidecar watches. Empty = all.
    pub entries: Vec<String>,
    /// Container mode in Kubernetes: the pod IP from the downward API.
    pub pod_ip: Option<String>,
    /// The first retry delay after a failed call. Default 1 s.
    pub backoff_initial: Duration,
    /// The longest retry delay. Default 30 s: one lease lifetime.
    pub backoff_max: Duration,
    /// The longest wait at stop. Default 10 s.
    pub shutdown_grace: Duration,
    /// How often to report before the proxy names an interval. Default 10 s,
    /// the Kubernetes probe period.
    pub default_report_interval: Duration,
    /// The largest manifest file that the sidecar reads. Default 1 MiB, the
    /// default `client_max_body_size` of nginx.
    pub max_manifest_bytes: u64,
    /// The longest probe failure text that the sidecar sends. Default 256
    /// bytes, the field limit of `BackendReport.detail`.
    pub detail_max_bytes: usize,
    /// The longest wait for one registration call. Default 10 s.
    pub call_timeout: Duration,
}

impl Default for RegistrarConfig {
    fn default() -> Self {
        Self {
            mode: RegistrarMode::Host,
            host_id: String::new(),
            manifest_path: PathBuf::from("/etc/ewe/manifest.json"),
            proxy: String::new(),
            enrollment_url: None,
            proxy_fingerprint_sha256: None,
            enrollment_token_path: None,
            state_dir: PathBuf::from("/var/lib/ewe/registrar"),
            advertise_host: None,
            docker_network: None,
            entries: Vec::new(),
            pod_ip: None,
            backoff_initial: Duration::from_secs(1),
            backoff_max: Duration::from_secs(30),
            shutdown_grace: Duration::from_secs(10),
            default_report_interval: Duration::from_secs(10),
            max_manifest_bytes: 1024 * 1024,
            detail_max_bytes: 256,
            call_timeout: Duration::from_secs(10),
        }
    }
}

impl RegistrarConfig {
    /// A configuration with the default paths and timers.
    #[must_use]
    pub fn new(mode: RegistrarMode, host_id: impl Into<String>) -> Self {
        Self {
            mode,
            host_id: host_id.into(),
            ..Self::default()
        }
    }

    /// Set the registration authority URL.
    #[must_use]
    pub fn with_proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = proxy.into();
        self
    }

    /// Set the enrollment listener URL.
    #[must_use]
    pub fn with_enrollment_url(mut self, url: impl Into<String>) -> Self {
        self.enrollment_url = Some(url.into());
        self
    }

    /// Set the enrollment token file.
    #[must_use]
    pub fn with_enrollment_token_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.enrollment_token_path = Some(path.into());
        self
    }

    /// Set the state directory.
    #[must_use]
    pub fn with_state_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.state_dir = path.into();
        self
    }

    /// Set the host-mode advertise address.
    #[must_use]
    pub fn with_advertise_host(mut self, host: impl Into<String>) -> Self {
        self.advertise_host = Some(host.into());
        self
    }

    /// Set the manifest path.
    #[must_use]
    pub fn with_manifest_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.manifest_path = path.into();
        self
    }

    /// Set the default report interval.
    #[must_use]
    pub fn with_default_report_interval(mut self, interval: Duration) -> Self {
        self.default_report_interval = interval;
        self
    }

    /// Set the watched entries. An empty list watches every entry.
    #[must_use]
    pub fn with_entries(mut self, entries: Vec<String>) -> Self {
        self.entries = entries;
        self
    }

    /// Parse the configuration from an argument list (the first item is the
    /// program name). The environment variables are read too.
    ///
    /// # Errors
    /// [`RegistrarError::Config`] when the arguments do not parse or a value is
    /// not valid.
    pub fn from_args<I, T>(args: I) -> Result<Self, RegistrarError>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let cli =
            Cli::try_parse_from(args).map_err(|error| RegistrarError::Config(error.to_string()))?;
        cli.into_config()
    }

    /// Check the values that the process cannot run without.
    ///
    /// # Errors
    /// [`RegistrarError::Config`] for a missing host ID, a missing host-mode
    /// advertise address, an unknown proxy URL scheme or an invalid entry name.
    pub fn validate(&self) -> Result<(), RegistrarError> {
        if self.host_id.is_empty() {
            return Err(RegistrarError::Config("host_id is required".to_string()));
        }
        if self.mode == RegistrarMode::Host && self.advertise_host.is_none() {
            return Err(RegistrarError::Config(
                "advertise_host is required in host mode".to_string(),
            ));
        }
        if !(self.proxy.starts_with("https://") || self.proxy.starts_with("unix://")) {
            return Err(RegistrarError::Config(
                "proxy must start with https:// or unix://".to_string(),
            ));
        }
        for entry in &self.entries {
            validate_entry_name(entry)?;
        }
        if self.backoff_initial.is_zero() || self.backoff_max < self.backoff_initial {
            return Err(RegistrarError::Config(
                "backoff_initial must be at least 1 ms and no longer than backoff_max".to_string(),
            ));
        }
        if self.default_report_interval.is_zero() {
            return Err(RegistrarError::Config(
                "default_report_interval must be at least 1 ms".to_string(),
            ));
        }
        if self.detail_max_bytes == 0 {
            return Err(RegistrarError::Config(
                "detail_max_bytes must be at least 1".to_string(),
            ));
        }
        Ok(())
    }

    /// Whether this sidecar watches `entry`. An empty `entries` watches all.
    #[must_use]
    pub fn watches(&self, entry: &str) -> bool {
        self.entries.is_empty() || self.entries.iter().any(|name| name == entry)
    }
}

/// Apply the manifest entry-name rule to a configured entry name.
fn validate_entry_name(name: &str) -> Result<(), RegistrarError> {
    let valid = !name.is_empty()
        && name.len() <= 63
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(RegistrarError::Config(format!(
            "entry {name:?} must match [a-z0-9-]{{1,63}} and not start or end with '-'"
        )))
    }
}

/// The command-line surface.
#[derive(Debug, Parser)]
#[command(
    name = "ewe-registrar",
    about = "Watch the apps of a manifest and report them to the proxy registration authority"
)]
struct Cli {
    /// How the sidecar runs: host or container.
    #[arg(long, env = "EWE_REGISTRAR_MODE", default_value = "host")]
    mode: String,
    /// The stable name of this machine or pod.
    #[arg(long, env = "EWE_REGISTRAR_HOST_ID")]
    host_id: Option<String>,
    /// The manifest file.
    #[arg(
        long,
        env = "EWE_REGISTRAR_MANIFEST_PATH",
        default_value = "/etc/ewe/manifest.json"
    )]
    manifest_path: PathBuf,
    /// The registration authority URL.
    #[arg(long, env = "EWE_REGISTRAR_PROXY")]
    proxy: Option<String>,
    /// The enrollment listener URL.
    #[arg(long, env = "EWE_REGISTRAR_ENROLLMENT_URL")]
    enrollment_url: Option<String>,
    /// The SHA-256 fingerprint of the proxy registration certificate.
    #[arg(long, env = "EWE_REGISTRAR_PROXY_FINGERPRINT_SHA256")]
    proxy_fingerprint_sha256: Option<String>,
    /// The enrollment token file.
    #[arg(long, env = "EWE_REGISTRAR_ENROLLMENT_TOKEN_PATH")]
    enrollment_token_path: Option<PathBuf>,
    /// Certificate, key and session data.
    #[arg(
        long,
        env = "EWE_REGISTRAR_STATE_DIR",
        default_value = "/var/lib/ewe/registrar"
    )]
    state_dir: PathBuf,
    /// Host mode: the address that the proxy dials.
    #[arg(long, env = "EWE_REGISTRAR_ADVERTISE_HOST")]
    advertise_host: Option<String>,
    /// Container mode with Docker: the network to read the container IP from.
    #[arg(long, env = "EWE_REGISTRAR_DOCKER_NETWORK")]
    docker_network: Option<String>,
    /// Container mode: one or more watched entries (comma separated).
    #[arg(long, env = "EWE_REGISTRAR_ENTRIES", value_delimiter = ',')]
    entries: Vec<String>,
    /// Container mode in Kubernetes: the pod IP.
    #[arg(long, env = "POD_IP")]
    pod_ip: Option<String>,
    /// The first retry delay, in seconds.
    #[arg(long, env = "EWE_REGISTRAR_BACKOFF_INITIAL_SECS", default_value_t = 1)]
    backoff_initial_secs: u64,
    /// The longest retry delay, in seconds.
    #[arg(long, env = "EWE_REGISTRAR_BACKOFF_MAX_SECS", default_value_t = 30)]
    backoff_max_secs: u64,
    /// The longest wait at stop, in seconds.
    #[arg(long, env = "EWE_REGISTRAR_SHUTDOWN_GRACE_SECS", default_value_t = 10)]
    shutdown_grace_secs: u64,
    /// The report interval before the proxy names one, in milliseconds.
    #[arg(
        long,
        env = "EWE_REGISTRAR_DEFAULT_REPORT_INTERVAL_MS",
        default_value_t = 10_000
    )]
    default_report_interval_ms: u64,
    /// The largest manifest file, in bytes.
    #[arg(
        long,
        env = "EWE_REGISTRAR_MAX_MANIFEST_BYTES",
        default_value_t = 1024 * 1024
    )]
    max_manifest_bytes: u64,
    /// The longest probe failure text, in bytes.
    #[arg(long, env = "EWE_REGISTRAR_DETAIL_MAX_BYTES", default_value_t = 256)]
    detail_max_bytes: usize,
    /// The longest wait for one registration call, in seconds.
    #[arg(long, env = "EWE_REGISTRAR_CALL_TIMEOUT_SECS", default_value_t = 10)]
    call_timeout_secs: u64,
}

impl Cli {
    fn into_config(self) -> Result<RegistrarConfig, RegistrarError> {
        let mode = self.mode.parse()?;
        let host_id = self
            .host_id
            .ok_or_else(|| RegistrarError::Config("host_id is required".to_string()))?;
        let proxy = self
            .proxy
            .ok_or_else(|| RegistrarError::Config("proxy is required".to_string()))?;
        let config = RegistrarConfig {
            mode,
            host_id,
            manifest_path: self.manifest_path,
            proxy,
            enrollment_url: self.enrollment_url,
            proxy_fingerprint_sha256: self.proxy_fingerprint_sha256,
            enrollment_token_path: self.enrollment_token_path,
            state_dir: self.state_dir,
            advertise_host: self.advertise_host,
            docker_network: self.docker_network,
            entries: self.entries,
            pod_ip: self.pod_ip,
            backoff_initial: Duration::from_millis(self.backoff_initial_secs.max(1)),
            backoff_max: Duration::from_millis(self.backoff_max_secs.max(1)),
            shutdown_grace: Duration::from_secs(self.shutdown_grace_secs),
            default_report_interval: Duration::from_millis(self.default_report_interval_ms.max(1)),
            max_manifest_bytes: self.max_manifest_bytes.max(1),
            detail_max_bytes: self.detail_max_bytes.max(1),
            call_timeout: Duration::from_secs(self.call_timeout_secs.max(1)),
        };
        config.validate()?;
        Ok(config)
    }
}

//! The configuration of the native host.
//!
//! WHY: every limit and choice of a host is a configuration field with a
//! documented default (CLAUDE.md rule 8). The binary fills it from command-line
//! flags and environment variables.
//!
//! WHAT: [`NativeHostConfig`], [`NativeSecurity`], [`ServerIdentityConfig`],
//! [`OperatorKeyConfig`] and their defaults.
//!
//! HOW: the defaults copy the value of an established server, as the field
//! documentation says. The key paths derive from `state_dir`, so a host that
//! changes the state directory needs no second change.

use std::path::PathBuf;
use std::sync::Arc;

use foundation_auth::pki::CaConfig;
use foundation_auth::trust::TrustConfig;
use foundation_compact::Duration;
use foundation_http::native::server::ServerConfig;

use crate::core::config::{EnrollConfig, RegistryConfig, TurnLimits};
use crate::core::model::ModelCatalog;
use crate::model::ModelSource;

/// The environment variable that overrides the bootstrap signing key.
pub const SIGNING_KEY_ENV: &str = "EWE_AGENT_RPC_SIGNING_KEY";
/// The environment variable that overrides the certificate authority (CA)
/// private key.
pub const CA_KEY_ENV: &str = "EWE_AGENT_RPC_CA_KEY";
/// The environment variable that overrides the server private key.
pub const SERVER_KEY_ENV: &str = "EWE_AGENT_RPC_SERVER_KEY";
/// File name of the bootstrap signing key inside the state directory.
pub const SIGNING_KEY_FILE: &str = "bootstrap-signing-key.pem";
/// File name of the CA private key inside the state directory.
pub const CA_KEY_FILE: &str = "ca-key.pem";
/// File name of the server private key inside the state directory.
pub const SERVER_KEY_FILE: &str = "server-key.pem";

/// One operator private key: an environment variable override and a file.
///
/// The environment value is an inline Privacy-Enhanced Mail (PEM) block or the
/// path of one. An error in the override stops the start; the host never falls
/// back to another key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorKeyConfig {
    /// The file that holds the key.
    pub path: PathBuf,
    /// The environment variable that overrides the file. `None`: no override.
    pub env_var: Option<String>,
}

impl OperatorKeyConfig {
    /// A key in `path`, overridden by `env_var` when it is set.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, env_var: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            env_var: Some(env_var.into()),
        }
    }

    /// A key in `path`, with no environment override.
    #[must_use]
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            env_var: None,
        }
    }
}

/// The names and lifetime of the server certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerIdentityConfig {
    /// The Domain Name System (DNS) names and Internet Protocol (IP)
    /// addresses that the certificate covers.
    pub names: Vec<String>,
    /// The lifetime of the certificate. Default 90 days: the lifetime of a
    /// Let's Encrypt leaf certificate.
    pub lifetime: Duration,
}

impl Default for ServerIdentityConfig {
    fn default() -> Self {
        Self {
            names: vec!["localhost".into(), "127.0.0.1".into(), "::1".into()],
            lifetime: Duration::from_hours(90 * 24),
        }
    }
}

impl ServerIdentityConfig {
    /// Replace the names that the certificate covers.
    #[must_use]
    pub fn with_names(mut self, names: Vec<String>) -> Self {
        self.names = names;
        self
    }

    /// Replace the certificate lifetime.
    #[must_use]
    pub fn with_lifetime(mut self, lifetime: Duration) -> Self {
        self.lifetime = lifetime;
        self
    }
}

/// How the native host authenticates a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeSecurity {
    /// Enrollment and mutual TLS (mTLS), the default. With
    /// `accept_access_secrets`, a call can also show a bearer secret with
    /// purpose `access`, for example from a browser.
    Mtls {
        /// Accept a signed access secret on the agent listener.
        accept_access_secrets: bool,
    },
    /// Development only: no TLS and no auth. The host refuses to start unless
    /// `bind` is a loopback address.
    InsecurePlaintext,
}

impl Default for NativeSecurity {
    fn default() -> Self {
        Self::Mtls {
            accept_access_secrets: false,
        }
    }
}

/// Everything that the native host needs.
#[derive(Clone)]
pub struct NativeHostConfig {
    /// The agent listener, `host:port`. Port 0 picks a free port. Default
    /// `127.0.0.1:7443`.
    pub bind: String,
    /// The enrollment listener. `None`: the host of `bind`, port 0.
    pub bootstrap_bind: Option<String>,
    /// Key files and the trust database. Default [`crate::secret::default_state_dir`].
    pub state_dir: PathBuf,
    /// The local models to load. Ignored when `catalog` is set.
    pub models: Vec<ModelSource>,
    /// A ready catalog. `None`: the host builds a `LocalCatalog` from
    /// `models`. A test uses this field to inject a scripted catalog.
    pub catalog: Option<Arc<dyn ModelCatalog>>,
    /// Load every model at start, so the first `CreateInstance` does not wait.
    /// Default true.
    pub preload: bool,
    /// How the host authenticates a call.
    pub security: NativeSecurity,
    /// The `AgentService` URL that a bootstrap payload gives a client. `None`:
    /// `https://` and the bound agent address. Set it when clients reach the
    /// host by another name.
    pub public_url: Option<String>,
    /// The `BootstrapService` URL for the enrollment call. `None`: `https://`
    /// and the bound enrollment address.
    pub public_enroll_url: Option<String>,
    /// The HTTP limits of the agent listener. Default
    /// [`ServerConfig::defaults`].
    pub http: ServerConfig,
    /// The instance limits.
    pub registry: RegistryConfig,
    /// The turn limits.
    pub turns: TurnLimits,
    /// The enrollment limits.
    pub enroll: EnrollConfig,
    /// The trusted-client store limits.
    pub trust: TrustConfig,
    /// The certificate authority (CA) settings.
    pub ca: CaConfig,
    /// The server certificate settings.
    pub server_identity: ServerIdentityConfig,
    /// The signing key source. `None`: [`SIGNING_KEY_FILE`] in `state_dir`,
    /// overridable by [`SIGNING_KEY_ENV`].
    pub signing_key: Option<OperatorKeyConfig>,
    /// The CA key source. `None`: [`CA_KEY_FILE`] in `state_dir`, overridable
    /// by [`CA_KEY_ENV`].
    pub ca_key: Option<OperatorKeyConfig>,
    /// The server key source. `None`: [`SERVER_KEY_FILE`] in `state_dir`,
    /// overridable by [`SERVER_KEY_ENV`].
    pub server_key: Option<OperatorKeyConfig>,
    /// Workers in the valtron pool. Default 16: at least 8, as LEARNINGS 01
    /// task T6 says.
    pub workers: usize,
    /// How long shutdown waits for a running turn. Default 30 s: the default
    /// `terminationGracePeriodSeconds` of Kubernetes.
    pub shutdown_grace: Duration,
}

impl std::fmt::Debug for NativeHostConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeHostConfig")
            .field("bind", &self.bind)
            .field("bootstrap_bind", &self.bootstrap_bind)
            .field("state_dir", &self.state_dir)
            .field("models", &self.models)
            .field("catalog", &self.catalog.is_some())
            .field("preload", &self.preload)
            .field("security", &self.security)
            .field("public_url", &self.public_url)
            .field("public_enroll_url", &self.public_enroll_url)
            .field("registry", &self.registry)
            .field("turns", &self.turns)
            .field("enroll", &self.enroll)
            .field("trust", &self.trust)
            .field("ca", &self.ca)
            .field("server_identity", &self.server_identity)
            .field("signing_key", &self.signing_key)
            .field("ca_key", &self.ca_key)
            .field("server_key", &self.server_key)
            .field("workers", &self.workers)
            .field("shutdown_grace", &self.shutdown_grace)
            .finish_non_exhaustive()
    }
}

impl Default for NativeHostConfig {
    /// The bind address is `127.0.0.1:7443`, the safe default: a host that
    /// must serve another interface sets it. The state directory is
    /// [`crate::secret::default_state_dir`]; when neither `XDG_STATE_HOME` nor
    /// `HOME` is set, the relative path `.ewe/agent-rpc` is used so that
    /// `Default` cannot fail.
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:7443".into(),
            bootstrap_bind: None,
            state_dir: crate::secret::default_state_dir()
                .unwrap_or_else(|_| PathBuf::from(".ewe").join("agent-rpc")),
            models: Vec::new(),
            catalog: None,
            preload: true,
            security: NativeSecurity::default(),
            public_url: None,
            public_enroll_url: None,
            http: ServerConfig::defaults(),
            registry: RegistryConfig::default(),
            turns: TurnLimits::default(),
            enroll: EnrollConfig::default(),
            trust: TrustConfig::default(),
            ca: CaConfig::default(),
            server_identity: ServerIdentityConfig::default(),
            signing_key: None,
            ca_key: None,
            server_key: None,
            workers: 16,
            shutdown_grace: Duration::from_secs(30),
        }
    }
}

impl NativeHostConfig {
    /// Set the state directory.
    #[must_use]
    pub fn with_state_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.state_dir = dir.into();
        self
    }

    /// Set the agent listener address.
    #[must_use]
    pub fn with_bind(mut self, bind: impl Into<String>) -> Self {
        self.bind = bind.into();
        self
    }

    /// Set the enrollment listener address.
    #[must_use]
    pub fn with_bootstrap_bind(mut self, bind: impl Into<String>) -> Self {
        self.bootstrap_bind = Some(bind.into());
        self
    }

    /// Set how the host authenticates a call.
    #[must_use]
    pub fn with_security(mut self, security: NativeSecurity) -> Self {
        self.security = security;
        self
    }

    /// Use a ready catalog instead of loading `models`.
    #[must_use]
    pub fn with_catalog(mut self, catalog: Arc<dyn ModelCatalog>) -> Self {
        self.catalog = Some(catalog);
        self
    }

    /// Replace the local models.
    #[must_use]
    pub fn with_models(mut self, models: Vec<ModelSource>) -> Self {
        self.models = models;
        self
    }

    /// Turn model preloading on or off.
    #[must_use]
    pub fn with_preload(mut self, preload: bool) -> Self {
        self.preload = preload;
        self
    }

    /// Set the enrollment listener's public URL.
    #[must_use]
    pub fn with_public_urls(mut self, agent: Option<String>, enroll: Option<String>) -> Self {
        self.public_url = agent;
        self.public_enroll_url = enroll;
        self
    }

    /// Set how long shutdown waits for a running turn.
    #[must_use]
    pub fn with_shutdown_grace(mut self, grace: Duration) -> Self {
        self.shutdown_grace = grace;
        self
    }

    /// Replace the HTTP limits of the two listeners.
    #[must_use]
    pub fn with_http(mut self, http: ServerConfig) -> Self {
        self.http = http;
        self
    }

    /// The signing key source, with the default path and variable.
    #[must_use]
    pub fn signing_key_source(&self) -> OperatorKeyConfig {
        self.signing_key.clone().unwrap_or_else(|| {
            OperatorKeyConfig::new(self.state_dir.join(SIGNING_KEY_FILE), SIGNING_KEY_ENV)
        })
    }

    /// The CA key source, with the default path and variable.
    #[must_use]
    pub fn ca_key_source(&self) -> OperatorKeyConfig {
        self.ca_key
            .clone()
            .unwrap_or_else(|| OperatorKeyConfig::new(self.state_dir.join(CA_KEY_FILE), CA_KEY_ENV))
    }

    /// The server key source, with the default path and variable.
    #[must_use]
    pub fn server_key_source(&self) -> OperatorKeyConfig {
        self.server_key.clone().unwrap_or_else(|| {
            OperatorKeyConfig::new(self.state_dir.join(SERVER_KEY_FILE), SERVER_KEY_ENV)
        })
    }

    /// The enrollment bind, when `bootstrap_bind` is `None`: the host of
    /// `bind`, port 0.
    #[must_use]
    pub fn bootstrap_bind_or_default(&self, agent_host: &str) -> String {
        self.bootstrap_bind
            .clone()
            .unwrap_or_else(|| format!("{agent_host}:0"))
    }

    /// The `AgentService` URL that a bootstrap payload gives a client.
    #[must_use]
    pub fn agent_public_url(&self, bound: &str) -> String {
        self.public_url
            .clone()
            .unwrap_or_else(|| format!("https://{bound}"))
    }

    /// The `BootstrapService` URL for the enrollment call.
    #[must_use]
    pub fn enroll_public_url(&self, bound: &str) -> String {
        self.public_enroll_url
            .clone()
            .unwrap_or_else(|| format!("https://{bound}"))
    }
}

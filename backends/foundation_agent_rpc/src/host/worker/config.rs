//! The Worker configuration and its limits (spec 83 feature 05, section 5.2).
//!
//! WHY: every limit is configuration, not a constant in the code path (root
//! rule 8). The operator sets the values in the Worker vars.
//!
//! WHAT: [`WorkerHostConfig`], [`WorkerEnrollment`] and [`ServeLimits`].
//!
//! HOW: [`WorkerHostConfig::from_bindings`] reads the vars once for each
//! isolate. A bad or missing var stops the first request with a
//! [`AgentRpcError::Config`] that names the var, never a secret value.

use crate::core::config::{EnrollConfig, RegistryConfig, TurnLimits};
use crate::core::cors::CorsConfig;
use crate::core::model::ApiModelConfig;
use crate::error::{AgentRpcError, AgentRpcResult};
use foundation_auth::trust::TrustConfig;
use foundation_compact::Duration;

use worker::Env;

/// The largest request body that the Worker accepts. Default 4 mebibytes
/// (MiB): the gRPC default maximum receive message size.
pub const DEFAULT_MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;

/// Chunks queued in one response before the handler waits. Default 4: the
/// pipe depth of `ClientOptions` today.
pub const DEFAULT_RESPONSE_QUEUE: usize = 4;

/// Largest response header block. Default 32 kibibytes (KiB): nginx's
/// default `large_client_header_buffers` is four buffers of 8 KiB.
pub const DEFAULT_MAX_HEADER_BYTES: usize = 32 * 1024;

/// Everything a Worker host reads from its bindings.
#[derive(Debug, Clone)]
pub struct WorkerHostConfig {
    /// The public base URL. It is the audience of a bearer secret and the
    /// `url` of the bootstrap payload. Var `AGENT_RPC_PUBLIC_URL`.
    pub public_url: String,
    /// Remote models. Var `AGENT_RPC_MODELS`, a JSON list of
    /// [`ApiModelConfig`].
    pub models: Vec<ApiModelConfig>,
    /// Default `PerOwner`: a Worker serves unrelated clients.
    pub registry: RegistryConfig,
    /// The turn limits.
    pub turns: TurnLimits,
    /// The trust-store limits. Default [`TrustConfig::default`].
    pub trust: TrustConfig,
    /// The Cross-Origin Resource Sharing (CORS) answer for browsers.
    pub cors: CorsConfig,
    /// Accept Cloudflare mutual TLS (mTLS) (`request.cf.tlsClientAuth`).
    /// Default true. It does nothing until the zone has mTLS rules.
    pub cloudflare_mtls: bool,
    /// Destroy an instance after this long with no turn. Default `None`.
    pub instance_idle_ttl: Option<Duration>,
    /// Largest request body that the Worker accepts. Default
    /// [`DEFAULT_MAX_REQUEST_BYTES`].
    pub max_request_bytes: usize,
    /// Chunks queued in one response before the handler waits. Default
    /// [`DEFAULT_RESPONSE_QUEUE`].
    pub response_queue: usize,
    /// Largest response header block. Default [`DEFAULT_MAX_HEADER_BYTES`].
    pub max_header_bytes: usize,
    /// `Some`: this Worker serves `BootstrapService` on its own origin.
    /// `None` (default): verify-only.
    pub enrollment: Option<WorkerEnrollment>,
    /// The history archive limits. Default [`ArchiveConfig::default`].
    pub archive: crate::core::archive::ArchiveConfig,
}

impl Default for WorkerHostConfig {
    fn default() -> Self {
        Self {
            public_url: String::new(),
            models: Vec::new(),
            registry: RegistryConfig::default(),
            turns: TurnLimits::default(),
            trust: TrustConfig::default(),
            cors: CorsConfig::default(),
            cloudflare_mtls: true,
            instance_idle_ttl: None,
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
            response_queue: DEFAULT_RESPONSE_QUEUE,
            max_header_bytes: DEFAULT_MAX_HEADER_BYTES,
            enrollment: None,
            archive: crate::core::archive::ArchiveConfig::default(),
        }
    }
}

/// This Worker serves enrollment, with the payload key in a Worker secret.
#[derive(Debug, Clone)]
pub struct WorkerEnrollment {
    /// The name of the Worker secret that holds the payload private key.
    /// Default `AGENT_RPC_SIGNING_KEY`.
    pub signing_key_secret: String,
    /// The enrollment limits.
    pub enroll: EnrollConfig,
}

impl Default for WorkerEnrollment {
    fn default() -> Self {
        Self {
            signing_key_secret: "AGENT_RPC_SIGNING_KEY".to_string(),
            enroll: EnrollConfig::default(),
        }
    }
}

/// The limits that one served call reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeLimits {
    /// Chunks queued in the response before the handler waits.
    pub response_queue: usize,
    /// Largest response header block.
    pub max_header_bytes: usize,
    /// Largest request body that the bridge reads.
    pub max_request_bytes: usize,
}

impl From<&WorkerHostConfig> for ServeLimits {
    fn from(config: &WorkerHostConfig) -> Self {
        Self {
            response_queue: config.response_queue,
            max_header_bytes: config.max_header_bytes,
            max_request_bytes: config.max_request_bytes,
        }
    }
}

impl WorkerHostConfig {
    /// Read the vars of the Worker.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] names the bad or missing var.
    pub fn from_bindings(bindings: &Env) -> AgentRpcResult<Self> {
        let public_url = required_var(bindings, "AGENT_RPC_PUBLIC_URL")?;
        let models: Vec<ApiModelConfig> = match optional_var(bindings, "AGENT_RPC_MODELS") {
            Some(json) => serde_json::from_str(&json).map_err(|error| {
                AgentRpcError::Config(format!(
                    "AGENT_RPC_MODELS is not a JSON model list: {error}"
                ))
            })?,
            None => Vec::new(),
        };
        let mut registry = RegistryConfig::default();
        registry.scope_policy = crate::core::registry::ScopePolicy::PerOwner;
        let mut cors = CorsConfig::default();
        if let Some(origins) = optional_var(bindings, "AGENT_RPC_CORS_ORIGINS") {
            cors.allowed_origins = origins
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_string)
                .collect();
        }
        let enrollment =
            optional_var(bindings, "AGENT_RPC_SIGNING_KEY_SECRET").map(|name| WorkerEnrollment {
                signing_key_secret: name,
                enroll: EnrollConfig::default(),
            });
        Ok(Self {
            public_url,
            models,
            registry,
            cors,
            enrollment,
            ..Self::default()
        })
    }
}

fn required_var(bindings: &Env, name: &str) -> AgentRpcResult<String> {
    optional_var(bindings, name)
        .ok_or_else(|| AgentRpcError::Config(format!("the Worker var {name} is missing")).into())
}

fn optional_var(bindings: &Env, name: &str) -> Option<String> {
    bindings.var(name).ok().map(|value| value.to_string())
}

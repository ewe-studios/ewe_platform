//! Configuration limits for the core.
//!
//! WHY: every limit is configuration, not a constant in the code path (root
//! rule 8). A host sets these values once and the core reads them from then on.
//!
//! WHAT: [`RegistryConfig`], [`TurnLimits`] and [`EnrollConfig`], with builder
//! methods and documented defaults.
//!
//! HOW: each default names the server whose value it copies. The reasons are
//! in the field documentation.

use foundation_compact::Duration;

use crate::core::registry::ScopePolicy;

/// Registry limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryConfig {
    /// Live instances per host (native) or per scope (Worker). Default 1024:
    /// nginx's `worker_connections` in its shipped `nginx.conf`.
    pub max_instances: usize,
    /// Longest instance name. Default 63: a Request for Comments (RFC) 1123
    /// Domain Name System (DNS) label.
    pub max_name_len: usize,
    /// `ListInstances` page size when the client sends 0. Default 30: the
    /// `GitHub` REST API default `per_page`.
    pub default_page_size: u32,
    /// Largest page. Default 100: the `GitHub` REST API maximum `per_page`.
    pub max_page_size: u32,
    /// Which scope a call gets. Default [`ScopePolicy::Shared`], the behavior
    /// of today on the native host. A Worker sets
    /// [`ScopePolicy::PerOwner`].
    pub scope_policy: ScopePolicy,
}

impl RegistryConfig {
    /// The live-instance cap.
    #[must_use]
    pub fn with_max_instances(mut self, max: usize) -> Self {
        self.max_instances = max;
        self
    }

    /// The instance-name length cap.
    #[must_use]
    pub fn with_max_name_len(mut self, max: usize) -> Self {
        self.max_name_len = max;
        self
    }

    /// The page size that a zero request gets.
    #[must_use]
    pub fn with_default_page_size(mut self, size: u32) -> Self {
        self.default_page_size = size;
        self
    }

    /// The largest page.
    #[must_use]
    pub fn with_max_page_size(mut self, size: u32) -> Self {
        self.max_page_size = size;
        self
    }

    /// Which scope a call gets.
    #[must_use]
    pub fn with_scope_policy(mut self, policy: ScopePolicy) -> Self {
        self.scope_policy = policy;
        self
    }
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            max_instances: 1024,
            max_name_len: 63,
            default_page_size: 30,
            max_page_size: 100,
            scope_policy: ScopePolicy::Shared,
        }
    }
}

/// Turn limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnLimits {
    /// End a turn when the model makes no event for this long. Default 60 s:
    /// nginx's default `proxy_read_timeout`.
    pub idle_timeout: Duration,
    /// End a turn after this long in total. Default `None` (no limit): Go
    /// `net/http`'s default `WriteTimeout` is 0, which means no limit.
    pub max_duration: Option<Duration>,
    /// Largest prompt. Default 1 mebibyte (MiB): nginx's default
    /// `client_max_body_size`.
    pub max_prompt_bytes: usize,
    /// Largest `Chat` (unary) reply. A longer answer fails with
    /// `RESOURCE_EXHAUSTED` and the hint "use `ChatStream`". Default 4 MiB: the
    /// gRPC default maximum receive message size.
    pub max_unary_reply_bytes: usize,
}

impl TurnLimits {
    /// The idle timeout.
    #[must_use]
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// The total turn limit. `None` means no limit.
    #[must_use]
    pub fn with_max_duration(mut self, max: Option<Duration>) -> Self {
        self.max_duration = max;
        self
    }

    /// The largest prompt.
    #[must_use]
    pub fn with_max_prompt_bytes(mut self, bytes: usize) -> Self {
        self.max_prompt_bytes = bytes;
        self
    }

    /// The largest unary reply.
    #[must_use]
    pub fn with_max_unary_reply_bytes(mut self, bytes: usize) -> Self {
        self.max_unary_reply_bytes = bytes;
        self
    }
}

impl Default for TurnLimits {
    fn default() -> Self {
        Self {
            idle_timeout: Duration::from_secs(60),
            max_duration: None,
            max_prompt_bytes: 1024 * 1024,
            max_unary_reply_bytes: 4 * 1024 * 1024,
        }
    }
}

/// Enrollment limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrollConfig {
    /// Lifetime of an issued client certificate. Default 90 days: the lifetime
    /// of a Let's Encrypt certificate.
    pub client_cert_lifetime: Duration,
    /// Lifetime of an access secret that enrollment or refresh issues.
    /// Default 1 hour: the lifetime of a Google OAuth 2.0 access token.
    pub access_lifetime: Duration,
    /// Allowed difference between the client and server clocks on a refresh.
    /// Default 5 minutes: the default `clockskew` of MIT Kerberos (300 s).
    pub clock_skew: Duration,
    /// How many clients one enrollment secret may enroll. Default 1.
    pub max_enrollments_per_secret: u32,
    /// `Enroll` and `RefreshAccess` calls per peer address. Default 1 per
    /// second with a burst of 5: the `limit_req` example in the nginx
    /// documentation.
    pub rate: RateLimit,
    /// Largest certificate signing request (CSR) in Privacy-Enhanced Mail
    /// (PEM). Default 8 kibibytes (KiB): one nginx
    /// `large_client_header_buffers` buffer.
    pub max_csr_bytes: usize,
    /// Longest client display name. Default 128 bytes: a conventional
    /// human-readable name limit.
    pub max_client_name_bytes: usize,
    /// Largest refresh nonce. Default 64 bytes: enough for two SHA-256 values.
    pub max_refresh_nonce_bytes: usize,
}

impl EnrollConfig {
    /// The client-certificate lifetime.
    #[must_use]
    pub fn with_client_cert_lifetime(mut self, lifetime: Duration) -> Self {
        self.client_cert_lifetime = lifetime;
        self
    }

    /// The access-secret lifetime.
    #[must_use]
    pub fn with_access_lifetime(mut self, lifetime: Duration) -> Self {
        self.access_lifetime = lifetime;
        self
    }

    /// The allowed clock difference on a refresh.
    #[must_use]
    pub fn with_clock_skew(mut self, skew: Duration) -> Self {
        self.clock_skew = skew;
        self
    }

    /// The number of clients that one secret may enroll.
    #[must_use]
    pub fn with_max_enrollments_per_secret(mut self, max: u32) -> Self {
        self.max_enrollments_per_secret = max;
        self
    }

    /// The enrollment rate limit.
    #[must_use]
    pub fn with_rate(mut self, rate: RateLimit) -> Self {
        self.rate = rate;
        self
    }

    /// The largest certificate signing request.
    #[must_use]
    pub fn with_max_csr_bytes(mut self, bytes: usize) -> Self {
        self.max_csr_bytes = bytes;
        self
    }

    /// The longest client display name.
    #[must_use]
    pub fn with_max_client_name_bytes(mut self, bytes: usize) -> Self {
        self.max_client_name_bytes = bytes;
        self
    }

    /// The largest refresh nonce.
    #[must_use]
    pub fn with_max_refresh_nonce_bytes(mut self, bytes: usize) -> Self {
        self.max_refresh_nonce_bytes = bytes;
        self
    }
}

impl Default for EnrollConfig {
    fn default() -> Self {
        Self {
            client_cert_lifetime: Duration::from_hours(90 * 24),
            access_lifetime: Duration::from_hours(1),
            clock_skew: Duration::from_mins(5),
            max_enrollments_per_secret: 1,
            rate: RateLimit {
                per_second: 1,
                burst: 5,
            },
            max_csr_bytes: 8 * 1024,
            max_client_name_bytes: 128,
            max_refresh_nonce_bytes: 64,
        }
    }
}

/// The default port of the native agent-rpc server in a Cloudflare container.
///
/// The image must listen on this port. `AgentRpcConfig::port` changes the value
/// that the caller passes to `AgentContainer::call`.
pub const DEFAULT_AGENT_RPC_PORT: u16 = 8_080;

/// A token-bucket rate limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    /// Tokens added each second.
    pub per_second: u32,
    /// The bucket size: the largest burst.
    pub burst: u32,
}

/// Where one model provider application programming interface (API) key
/// lives.
///
/// A key is a secret, like a private key. The configuration names the secret,
/// never the value. The native host reads the secret from the host, and the
/// Worker reads it from a Worker secret or an account Secrets Store binding.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SecretRef {
    /// A per-host secret (`wrangler secret put <name>` on a Worker, or the
    /// native secret source of the same name).
    WorkerSecret {
        /// The binding name.
        name: String,
    },
    /// An account-level Secrets Store binding. Read with `get()`.
    SecretsStore {
        /// The binding name.
        binding: String,
    },
}

impl SecretRef {
    /// Read the reference from the `api_key_secret` text.
    ///
    /// A plain name is a host secret, as in the earlier configuration. A text
    /// that starts with `{` is a JSON [`SecretRef`].
    ///
    /// # Errors
    /// [`crate::error::AgentRpcError::Config`] for JSON that is not a
    /// [`SecretRef`].
    pub fn parse(text: &str) -> crate::error::AgentRpcResult<Self> {
        let text = text.trim();
        if text.starts_with('{') {
            serde_json::from_str(text).map_err(|error| {
                crate::error::AgentRpcError::Config(format!(
                    "api_key_secret is not a secret reference: {error}"
                ))
                .into()
            })
        } else {
            Ok(Self::WorkerSecret {
                name: text.to_string(),
            })
        }
    }

    /// The binding name, for an error message that never shows the value.
    #[must_use]
    pub fn binding(&self) -> &str {
        match self {
            Self::WorkerSecret { name } => name,
            Self::SecretsStore { binding } => binding,
        }
    }
}

//! `HetznerClient` — credentials, transport, and the API version pin.
//!
//! **WHY:** the generated `*_request` functions take an HTTP client and a bearer
//! token per call. Something has to own both, resolve the token from the
//! environment the way the rest of the ecosystem spells it, and keep it out of
//! everything that prints.
//!
//! **WHAT:** three constructors (spec-56 decision 02 §5) — [`HetznerClient::from_env`]
//! for the ergonomic path, [`HetznerClient::new`] for callers with their own
//! secret store, and [`HetznerClient::with_client`] to inject the transport, which
//! is what the mock-API tests use.
//!
//! **HOW:** the token never leaves this module except through
//! [`HetznerClient::token`]. `Debug` is hand-written and redacts it (decision 02
//! §4): a derived `Debug` on a struct holding a bearer token is one
//! `tracing::debug!` away from putting it in a log.

use foundation_netio::{DynNetClient, HttpClientBuilder};

use crate::types::HetznerError;

/// The environment variable Hetzner's own ecosystem uses.
///
/// `hcloud` and terraform's provider both read this, so anyone already deploying
/// to Hetzner needs no setup from us.
///
/// The override is **derived**, not invented: `EWE_` + this name. Strip the prefix
/// and you are back at the vendor's variable, so there is no third spelling to
/// remember (decision 02 §1).
pub const VENDOR_VARS: &[&str] = &["HCLOUD_TOKEN"];

/// Hetzner's API version, fixed into the client.
///
/// An API version is not a per-call decision, so callers never pass it
/// (feature 00 §3).
pub const API_VERSION: &str = "v1";

/// The base URL for Hetzner Cloud's API.
pub const DEFAULT_BASE_URL: &str = "https://api.hetzner.cloud/v1";

/// Every variable [`HetznerClient::from_env`] reads, in precedence order.
///
/// **All `EWE_*` before all vendor-native** (decision 02 §1): an override says
/// "use *this* token for this workspace", and that beats any vendor variable
/// lying around in the environment — not merely its own twin.
#[must_use]
pub fn credential_vars() -> Vec<String> {
    VENDOR_VARS
        .iter()
        .map(|v| format!("EWE_{v}"))
        .chain(VENDOR_VARS.iter().map(|v| (*v).to_string()))
        .collect()
}

/// Hetzner Cloud API client.
///
/// Holds a bearer token. See the module docs on why `Debug` is hand-written.
#[derive(Clone)]
pub struct HetznerClient {
    http: DynNetClient,
    token: String,
    base_url: String,
}

impl std::fmt::Debug for HetznerClient {
    /// Redacts the token.
    ///
    /// Not a derive, deliberately (decision 02 §4). The token is the one field
    /// here worth protecting, and a derived `Debug` would print it in any error
    /// chain, panic message or `tracing` field that formats the client.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HetznerClient")
            .field("base_url", &self.base_url)
            .field("token", &"<redacted>")
            .finish()
    }
}

impl HetznerClient {
    /// Read the token from the environment.
    ///
    /// Tries `EWE_HCLOUD_TOKEN`, then `HCLOUD_TOKEN`.
    ///
    /// # Errors
    /// [`HetznerError::NoCredentials`] when none is set **or all are empty**,
    /// naming every variable it looked for. An empty token is treated as absent:
    /// `export HCLOUD_TOKEN=` is a mistake, and an anonymous client that fails at
    /// the first request would report it as a 401 — blaming the token rather than
    /// the setup (decision 02 §2).
    pub fn from_env() -> Result<Self, HetznerError> {
        let vars = credential_vars();
        for name in &vars {
            match std::env::var(name) {
                Ok(token) if !token.trim().is_empty() => return Ok(Self::new(token)),
                _ => {}
            }
        }
        Err(HetznerError::NoCredentials { looked_for: vars })
    }

    /// Build with an explicit token.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            http: HttpClientBuilder::new().build(),
            token: token.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    /// Build with a caller-supplied transport.
    ///
    /// What the mock-API tests use: they inject a client and a token rather than
    /// mutating the process environment, which is global and would race under
    /// `--test-threads` (decision 02 §5).
    #[must_use]
    pub fn with_client(http: DynNetClient, token: impl Into<String>) -> Self {
        Self {
            http,
            token: token.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    /// Point the client at a different base URL — a mock server, in practice.
    #[must_use]
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// The HTTP client handle (a cheap `Arc` clone).
    #[must_use]
    pub fn http(&self) -> DynNetClient {
        self.http.clone()
    }

    /// The bearer token.
    #[must_use]
    pub fn token(&self) -> String {
        self.token.clone()
    }

    /// The API base URL.
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The pinned API version (`v1`).
    #[must_use]
    pub fn api_version(&self) -> &'static str {
        API_VERSION
    }
}

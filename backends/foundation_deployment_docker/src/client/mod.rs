//! Hand-written `DockerClient` — Unix-socket transport, version negotiation, and
//! container lifecycle over the generated async `*_request` functions.
//!
//! WHY: The generator produces the raw per-endpoint calls, but the Docker-specific
//! glue — resolving the daemon socket, negotiating the API version, and the
//! create → start → wait → stop → remove lifecycle — is hand-written.
//!
//! WHAT: [`DockerClient`] built on `foundation_netio::DynNetClient` over a Unix
//! socket, plus P0 container operations.
//!
//! HOW: `HttpClientBuilder::new().unix_socket(path).build()` yields a
//! `DynNetClient`; each method calls the matching generated `async fn`.

use foundation_deployment::providers::common::{ApiClient, BodyLimits};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use crate::error::DockerError;
use crate::generated::auth::AuthResponse;
use crate::generated::containers::{container_delete_request, ContainerDeleteArgs};
use crate::generated::create::{container_create_request, ContainerCreateArgs, ContainerCreateResponse};
use crate::generated::json::{container_inspect_request, ContainerInspectArgs, ContainerInspectResponse};
use crate::generated::pause::{container_pause_request, ContainerPauseArgs};
use crate::generated::shared::{ApiError, IDResponse};
use crate::generated::start::{container_start_request, ContainerStartArgs};
use crate::generated::stop::{container_stop_request, ContainerStopArgs};
use crate::generated::unpause::{container_unpause_request, ContainerUnpauseArgs};
use crate::generated::version::{system_version_request, SystemVersion, SystemVersionArgs};
use crate::generated::wait::{container_wait_request, ContainerWaitArgs, ContainerWaitResponse};

/// Default Docker daemon Unix socket path.
pub const DEFAULT_DOCKER_SOCKET: &str = "/var/run/docker.sock";

/// The newest Docker Engine API version this client speaks (the version its
/// generated types were built from).
///
/// The client does not assume the daemon supports it: before the first
/// versioned request it negotiates with the daemon (see
/// [`DockerClient::negotiate_version`]) and talks
/// `min(DEFAULT_API_VERSION, daemon ApiVersion)` from then on, the way the
/// `docker` CLI does. A daemon asked for a version newer than its own rejects
/// every request with `400 Bad Request`.
pub const DEFAULT_API_VERSION: &str = "1.53";

/// A closure type for the (empty) `builder_mod` argument — no request mutation.
pub(crate) type NoMod = fn(&mut PreparedRequestBuilder);

/// How much of the daemon's answers a [`DockerClient`] holds in memory at
/// once.
///
/// WHY: Every streamed answer is read incrementally, but some pieces must be
/// held whole before they can be used: an error body read for its message,
/// one line of a JSON-lines stream (pull/push/build progress, events, stats),
/// one frame of a multiplexed stdout/stderr stream (logs, attach, exec). A
/// corrupt or hostile daemon could make any of them unbounded, so each has a
/// cap — configuration, not a constant, so a caller with unusual output (huge
/// log lines, a proxy's large error pages) can raise it.
///
/// None of these limit how long a stream runs or how much it carries in all:
/// a followed log or an image export streams for as long as it is read.
///
/// HOW: Set on the client with [`DockerClient::with_limits`]; the error-body
/// and line caps travel to the generated functions in the client's
/// [`ApiClient`] ([`DockerClient::api`]), the frame cap to each
/// [`LogStream`](body::LogStream) the client hands out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockerLimits {
    /// The most of a non-2xx answer's body read for its error message.
    /// Default 64 KiB — see [`BodyLimits::max_error_body`].
    pub max_error_body: usize,
    /// The longest line of a JSON-lines answer buffered before it is an
    /// error. Default 1 MiB — see [`BodyLimits::max_json_line`].
    pub max_json_line: usize,
    /// The largest multiplexed stdout/stderr frame a [`LogStream`] buffers;
    /// a frame header announcing more is a corrupt stream, refused before
    /// any of its payload is buffered.
    ///
    /// Default 16 MiB. dockerd writes one frame per write the container
    /// makes, and its log drivers split lines at 16 KiB, so real frames are
    /// at most tens of kilobytes; Moby's own `stdcopy` reader sets no cap at
    /// all and grows its buffer to whatever a header announces. 16 MiB keeps
    /// a generous margin over any real frame while bounding a corrupt
    /// header, which can announce up to 4 GiB.
    ///
    /// [`LogStream`]: body::LogStream
    pub max_log_frame: usize,
}

impl Default for DockerLimits {
    fn default() -> Self {
        let body = BodyLimits::default();
        Self {
            max_error_body: body.max_error_body,
            max_json_line: body.max_json_line,
            max_log_frame: 16 * 1024 * 1024,
        }
    }
}

impl DockerLimits {
    /// Read at most `bytes` of an error body.
    #[must_use]
    pub fn with_max_error_body(mut self, bytes: usize) -> Self {
        self.max_error_body = bytes;
        self
    }

    /// Buffer at most `bytes` of one JSON-lines line.
    #[must_use]
    pub fn with_max_json_line(mut self, bytes: usize) -> Self {
        self.max_json_line = bytes;
        self
    }

    /// Buffer at most `bytes` of one multiplexed stdout/stderr frame.
    #[must_use]
    pub fn with_max_log_frame(mut self, bytes: usize) -> Self {
        self.max_log_frame = bytes;
        self
    }

    /// The limits the generated functions read responses under.
    #[must_use]
    pub fn body_limits(&self) -> BodyLimits {
        BodyLimits::default()
            .with_max_error_body(self.max_error_body)
            .with_max_json_line(self.max_json_line)
    }
}

// ── Container create body (hand-written; the spec has this as an unnamed allOf) ─

/// The body of `POST /containers/create` — combines `ContainerConfig` with an
/// optional `HostConfig` (networking + resources) and `NetworkingConfig`.
///
/// The Docker Engine API spec models this as an unnamed `allOf` of
/// `ContainerConfig` + `{HostConfig, NetworkingConfig}`, so the code generator
/// can't give it a name. This hand-written struct fills the gap, using the
/// generated component types. Resource limits use [`ContainerHostConfig`] which
/// merges the generated `HostConfig` + `Resources` (the spec has them as one
/// object; the generator split them).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ContainerCreateBody {
    // ── Flattened ContainerConfig fields (generated) ──
    #[serde(flatten)]
    pub config: crate::generated::json::ContainerConfig,

    /// Port bindings, volumes, capabilities PLUS CPU/memory/device limits.
    #[serde(
        rename = "HostConfig",
        skip_serializing_if = "Option::is_none"
    )]
    pub host_config: Option<ContainerHostConfig>,

    /// Per-network endpoint configuration (aliases, static IPs, MAC address).
    #[serde(
        rename = "NetworkingConfig",
        skip_serializing_if = "Option::is_none"
    )]
    pub networking_config: Option<NetworkingConfig>,
}

impl ContainerCreateBody {
    /// Create a minimal body with just the image set.
    #[must_use]
    pub fn new(image: impl Into<String>) -> Self {
        let mut config = crate::generated::json::ContainerConfig::default();
        config.image = Some(image.into());
        Self { config, host_config: None, networking_config: None }
    }

    /// Set the command to run in the container.
    #[must_use]
    pub fn with_cmd(mut self, cmd: Vec<String>) -> Self {
        self.config.cmd = Some(cmd);
        self
    }

    /// Add environment variables.
    #[must_use]
    pub fn with_env(mut self, env: Vec<(String, String)>) -> Self {
        self.config.env = Some(env.into_iter().map(|(k, v)| format!("{k}={v}")).collect());
        self
    }

    /// Set exposed ports (keys are `"port/proto"`, values are `{}` per Docker spec).
    #[must_use]
    pub fn with_exposed_ports(mut self, ports: serde_json::Value) -> Self {
        self.config.exposed_ports = Some(ports);
        self
    }

    /// Set container labels.
    #[must_use]
    pub fn with_labels(mut self, labels: serde_json::Value) -> Self {
        self.config.labels = Some(labels);
        self
    }

    /// Attach a host config (port bindings, binds, resources, capabilities).
    #[must_use]
    pub fn with_host_config(mut self, hc: ContainerHostConfig) -> Self {
        self.host_config = Some(hc);
        self
    }
}

/// Merged `HostConfig` + `Resources` for container creation.
///
/// The Docker Engine API spec defines `HostConfig` as a single object that
/// includes both networking/runtime fields and CPU/memory/device resource
/// limits. Our generated code splits them into two types (`HostConfig` and
/// `Resources`). This struct re-merges them for the `POST /containers/create`
/// body via `#[serde(flatten)]` — the field name sets (`PortBindings`, … and
/// `Memory`, `NanoCpus`, …) are disjoint, so the serialized JSON is correct.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ContainerHostConfig {
    /// Networking, runtime, security fields from `HostConfig`.
    #[serde(flatten)]
    pub base: crate::generated::json::HostConfig,
    /// CPU, memory, device resource limits from `Resources`.
    #[serde(flatten)]
    pub resources: crate::generated::json::Resources,
}

/// Container networking configuration — maps network names to endpoint settings.
///
/// Corresponds to the `NetworkingConfig` definition in the Docker Engine API spec.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkingConfig {
    /// A mapping of network name to endpoint configuration for that network.
    #[serde(rename = "EndpointsConfig")]
    pub endpoints_config: std::collections::HashMap<String, crate::generated::connect::EndpointSettings>,
}

/// Container lifecycle operations (restart, kill, rename, resize, top, changes,
/// logs, stats, export, archive, attach, list, prune).
pub mod containers_lifecycle;

/// Network operations (list, inspect, create, delete, connect, disconnect, prune).
pub mod networks;

/// Volume operations (list, inspect, create, delete, update, prune).
pub mod volumes;

/// Image operations (list, inspect, tag, delete, search, history,
/// load, commit, prune, build, push, pull).
pub mod images;

/// Build contexts (tar streams) for `POST /build`.
pub mod build_context;

/// Streamed answers as async streams of typed items ([`body::ProgressStream`],
/// [`body::EventStream`], [`body::StatsStream`], [`body::ByteBody`],
/// [`body::LogStream`]) — large and
/// long-lived bodies are read incrementally, never buffered whole.
pub mod body;

/// Exec instance operations (create, start, inspect, resize).
pub mod exec;

/// System operations (info, ping, events, data usage, auth).
/// (`version` lives directly on [`DockerClient`].)
pub mod system;

/// SSH transport (`ssh://user@host` via `docker system dial-stdio`).
///
/// Behind the `ssh` feature: it links libssh2/OpenSSL, which cannot share a
/// binary with the BoringSSL the `buildkit` feature pulls in.
#[cfg(feature = "ssh")]
pub mod ssh;

/// Docker daemon client over a Unix socket.
///
/// WHY: Talks to `dockerd` without bollard/tokio — a `DynNetClient` over the Unix
/// socket, driven by valtron.
///
/// WHAT: Holds the client handle, the socket path, and the negotiated API version.
///
/// HOW: Constructed via [`DockerClient::connect_unix`] /
/// [`DockerClient::connect_with_defaults`]. The API version is negotiated with
/// the daemon lazily, on the first versioned request, and cached; clones share
/// the cache, since they talk to the same daemon.
#[derive(Clone)]
pub struct DockerClient {
    http: DynNetClient,
    socket_path: PathBuf,
    /// The API version negotiated with the daemon. Empty until the first
    /// versioned request (or an explicit [`Self::negotiate_version`]).
    negotiated_version: Arc<OnceLock<String>>,
    /// Optional remote host:port for TCP connections (e.g. "192.168.1.10:2375").
    /// When `None`, the base URL uses `localhost` (the Unix-socket default).
    remote_host: Option<String>,
    /// Whether the transport is TLS — controls the `https://` vs `http://`
    /// scheme in [`Self::base_url`].
    tls: bool,
    /// How much of the daemon's answers is held in memory at once.
    limits: DockerLimits,
}

/// Mutual-TLS material for a remote Docker daemon (the `DOCKER_CERT_PATH`
/// `ca.pem` / `cert.pem` / `key.pem` trio).
#[derive(Clone)]
pub struct DockerTls {
    /// `ca.pem` — the CA that signed the daemon's cert. `None` uses the system
    /// roots (rarely what a private Docker CA wants).
    pub ca_pem: Option<Vec<u8>>,
    /// `cert.pem` — the client certificate presented to the daemon.
    pub cert_pem: Vec<u8>,
    /// `key.pem` — the client private key.
    pub key_pem: Vec<u8>,
    /// Verify the daemon's certificate against `ca_pem` (`DOCKER_TLS_VERIFY`).
    /// `false` accepts any server cert — encrypted but unauthenticated.
    pub verify: bool,
}

impl DockerTls {
    /// Load the `ca.pem` / `cert.pem` / `key.pem` trio from a `DOCKER_CERT_PATH`
    /// directory.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError::Unavailable`] if `cert.pem` or `key.pem` cannot be
    /// read.
    pub fn from_cert_dir(dir: impl AsRef<Path>, verify: bool) -> Result<Self, DockerError> {
        let dir = dir.as_ref();
        let read = |name: &str| {
            std::fs::read(dir.join(name))
                .map_err(|e| DockerError::Unavailable(format!("read {name}: {e}")))
        };
        Ok(Self {
            ca_pem: std::fs::read(dir.join("ca.pem")).ok(),
            cert_pem: read("cert.pem")?,
            key_pem: read("key.pem")?,
            verify,
        })
    }
}

impl std::fmt::Debug for DockerClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockerClient")
            .field("socket_path", &self.socket_path)
            .field("api_version", &self.api_version())
            .field("remote_host", &self.remote_host)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl DockerClient {
    /// Connect to the Docker daemon over the given Unix socket path.
    ///
    /// WHY: The socket is Docker's default, TLS-free, permission-authenticated
    /// local transport.
    ///
    /// WHAT: Builds a `DynNetClient` routed over `socket_path`.
    ///
    /// HOW: `HttpClientBuilder::new().unix_socket(path).build()`.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn connect_unix(socket_path: impl Into<PathBuf>) -> Self {
        let socket_path = socket_path.into();
        // Docker's blocking endpoints (stop, wait, restart) may not send
        // response headers for tens of seconds — 120 s covers the worst case.
        let http = HttpClientBuilder::new()
            .unix_socket(&socket_path)
            .read_timeout(std::time::Duration::from_secs(120))
            .build();
        Self {
            http,
            socket_path,
            negotiated_version: Arc::new(OnceLock::new()),
            remote_host: None,
            tls: false,
            limits: DockerLimits::default(),
        }
    }

    /// Connect via TCP to a remote Docker daemon (e.g. "192.168.1.10:2375").
    ///
    /// WHY: Docker daemons can be exposed over TCP (TLS or plaintext). This is the
    /// remote / non-localhost path. For a `--tlsverify` daemon use
    /// [`Self::connect_tls`].
    ///
    /// WHAT: Uses `HttpClientBuilder` default (TCP) + sets `remote_host` so
    /// [`Self::base_url`] generates the correct URL.
    #[must_use]
    pub fn connect_tcp(host: &str) -> Self {
        let socket_path = PathBuf::from(format!("tcp://{host}"));
        let http = HttpClientBuilder::new()
            .read_timeout(std::time::Duration::from_secs(120))
            .build();
        Self {
            http,
            socket_path,
            negotiated_version: Arc::new(OnceLock::new()),
            remote_host: Some(host.to_string()),
            tls: false,
            limits: DockerLimits::default(),
        }
    }

    /// Connect to a remote Docker daemon over **mutual TLS** (the standard
    /// `tcp://host:2376` + `DOCKER_CERT_PATH` setup).
    ///
    /// WHY: TCP-exposed daemons are normally protected with client-cert mTLS.
    ///
    /// WHAT: Builds a TLS connector from `tls` (client cert/key + CA) and dials
    /// `host` over `https://`.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError::Unavailable`] if the certificate material is
    /// invalid.
    pub fn connect_tls(host: &str, tls: DockerTls) -> Result<Self, DockerError> {
        use foundation_netio::netcap::ssl::SSLConnector;

        let connector = SSLConnector::from_client_mutual_pem(
            tls.ca_pem.as_deref(),
            &tls.cert_pem,
            &tls.key_pem,
            tls.verify,
        )
        .map_err(|e| DockerError::Unavailable(format!("build TLS connector: {e}")))?;

        let http = HttpClientBuilder::new()
            .with_tls_connector(connector)
            .read_timeout(std::time::Duration::from_secs(120))
            .build();
        Ok(Self {
            http,
            socket_path: PathBuf::from(format!("tcp://{host}")),
            negotiated_version: Arc::new(OnceLock::new()),
            remote_host: Some(host.to_string()),
            tls: true,
            limits: DockerLimits::default(),
        })
    }

    /// Connect to a remote Docker daemon over **SSH** (`ssh://user@host[:port]`).
    ///
    /// WHY: SSH is Docker's port-less remote transport — it reuses an existing
    /// SSH login instead of exposing a TCP port or managing TLS certs.
    ///
    /// WHAT: SSHes in and runs `docker system dial-stdio`, whose stdio bridges to
    /// the remote `/var/run/docker.sock`. The client then speaks the ordinary
    /// Docker HTTP API over that channel (base URL `http://localhost`).
    ///
    /// HOW: Installs an [`ssh::SshConnector`] on the HTTP client via
    /// [`HttpClientBuilder::with_connector`]; each request opens a fresh
    /// `dial-stdio` channel (session reused across requests, channel not pooled).
    /// The target is resolved through `~/.ssh/config` (HostName/User/Port/
    /// IdentityFile), the server's key is checked against `~/.ssh/known_hosts`
    /// (trust-on-first-use), and auth follows OpenSSH order: agent, then the
    /// resolved/default `~/.ssh/id_*` keys — see [`foundation_sshkit::Host`].
    ///
    /// # Errors
    ///
    /// Never fails at construction (the SSH session is established lazily on the
    /// first request); returns [`DockerError`] only via later API calls.
    #[cfg(feature = "ssh")]
    pub fn connect_ssh(url: &str) -> Result<Self, DockerError> {
        use foundation_sshkit::Host;

        // `Host::resolve` accepts `[user@]alias[:port]` and layers in
        // `~/.ssh/config` (HostName/User/Port/IdentityFile) — strip the scheme first.
        let spec = url.strip_prefix("ssh://").unwrap_or(url);
        let host = Host::resolve(spec);
        let connector = std::sync::Arc::new(ssh::SshConnector::new(host));
        let http = HttpClientBuilder::new()
            .with_connector(connector)
            .read_timeout(std::time::Duration::from_secs(120))
            .build();
        Ok(Self {
            http,
            socket_path: PathBuf::from(url),
            negotiated_version: Arc::new(OnceLock::new()),
            // dial-stdio bridges to the remote's *local* docker.sock — Host: localhost.
            remote_host: None,
            tls: false,
            limits: DockerLimits::default(),
        })
    }

    /// Set a custom remote host (e.g. for Docker contexts, SSH tunnels).
    ///
    /// WHY: Lets callers use `connect_unix` then override the URL for cases
    /// where the socket path is correct but the Host header / URL must differ.
    #[must_use]
    pub fn with_remote_host(mut self, host: &str) -> Self {
        self.remote_host = Some(host.to_string());
        self
    }

    /// Connect using the default socket resolution.
    ///
    /// WHY: Most callers want the standard local daemon without specifying a path.
    ///
    /// WHAT: Uses `DOCKER_HOST` when it names a `unix://` socket, else
    /// [`DEFAULT_DOCKER_SOCKET`].
    ///
    /// HOW: Parses `DOCKER_HOST`; falls back to `/var/run/docker.sock`. A
    /// `tcp://` host with `DOCKER_TLS_VERIFY` set (or a `DOCKER_CERT_PATH` with
    /// certs) is dialed over mutual TLS via [`Self::connect_tls`]; a `tcp://`
    /// host without TLS env is plaintext [`Self::connect_tcp`]; an `ssh://` host
    /// is driven over `docker system dial-stdio` via [`Self::connect_ssh`].
    ///
    /// # Errors
    ///
    /// Returns [`DockerError::Unavailable`] if `DOCKER_HOST` names an unrecognized
    /// transport or if TLS material is invalid.
    pub fn connect_with_defaults() -> Result<Self, DockerError> {
        let host = std::env::var("DOCKER_HOST").unwrap_or_default();
        if host.is_empty() {
            return Ok(Self::connect_unix(DEFAULT_DOCKER_SOCKET));
        }
        if let Some(path) = host.strip_prefix("unix://") {
            return Ok(Self::connect_unix(path));
        }
        if let Some(addr) = host.strip_prefix("tcp://") {
            // TLS when DOCKER_TLS_VERIFY is set, or a DOCKER_CERT_PATH is given.
            let verify = std::env::var("DOCKER_TLS_VERIFY").is_ok_and(|v| v != "0" && !v.is_empty());
            let cert_dir = std::env::var("DOCKER_CERT_PATH").ok();
            if verify || cert_dir.is_some() {
                let dir = cert_dir.ok_or_else(|| {
                    DockerError::Unavailable(
                        "DOCKER_TLS_VERIFY set but DOCKER_CERT_PATH is missing".to_string(),
                    )
                })?;
                let tls = DockerTls::from_cert_dir(&dir, verify)?;
                return Self::connect_tls(addr, tls);
            }
            return Ok(Self::connect_tcp(addr));
        }
        if host.starts_with("ssh://") {
            #[cfg(feature = "ssh")]
            {
                return Self::connect_ssh(&host);
            }
            // Say so plainly rather than falling through to "unrecognized
            // transport", which would send the caller hunting for a typo in a
            // DOCKER_HOST that is in fact perfectly valid.
            #[cfg(not(feature = "ssh"))]
            {
                return Err(DockerError::Unavailable(format!(
                    "DOCKER_HOST is {host}, but this build has the `ssh` feature off — \
                     enable foundation_deployment_docker/ssh (note it cannot be combined \
                     with `buildkit`: libssh2/OpenSSL vs BoringSSL)"
                )));
            }
        }
        Err(DockerError::Unavailable(format!(
            "unrecognized DOCKER_HOST transport: {host}"
        )))
    }

    /// Read the daemon's answers under `limits` instead of
    /// [`DockerLimits::default`].
    #[must_use]
    pub fn with_limits(mut self, limits: DockerLimits) -> Self {
        self.limits = limits;
        self
    }

    /// The limits this client reads the daemon's answers under.
    #[must_use]
    pub fn limits(&self) -> DockerLimits {
        self.limits
    }

    /// The cross-platform HTTP client handle (cheap `Arc` clone).
    ///
    /// To call a generated `*_request` function directly, pass
    /// [`Self::api`] instead, so the response is read under this client's
    /// limits.
    #[must_use]
    pub fn http(&self) -> DynNetClient {
        self.http.clone()
    }

    /// The handle the generated `*_request` functions take: the transport,
    /// and this client's [`DockerLimits`] for reading error bodies and
    /// JSON-lines answers.
    #[must_use]
    pub fn api(&self) -> ApiClient {
        ApiClient::new(self.http.clone()).with_limits(self.limits.body_limits())
    }

    /// The API version requests are sent with: the negotiated one once
    /// negotiation has run, [`DEFAULT_API_VERSION`] before that.
    #[must_use]
    pub fn api_version(&self) -> &str {
        self.negotiated_version
            .get()
            .map_or(DEFAULT_API_VERSION, String::as_str)
    }

    /// The daemon's root URL, without an API version: `http://localhost` for a
    /// Unix socket, `http://192.168.1.10:2375` for a remote daemon.
    ///
    /// WHY: `GET /version` must be reachable before a version is agreed on —
    /// the daemon answers it unversioned, whatever API versions it supports.
    #[must_use]
    pub fn unversioned_base_url(&self) -> String {
        let host = self.remote_host.as_deref().unwrap_or("localhost");
        let scheme = if self.tls { "https" } else { "http" };
        format!("{scheme}://{host}")
    }

    /// The base URL for API calls with the version currently in effect:
    /// `http://{host}/v{version}`.
    ///
    /// WHY: Generated functions take a `base_url: &str` so callers can point at
    /// remote Docker daemons over TCP, not just Unix-socket-local. Uses
    /// `remote_host` when set (via [`Self::connect_tcp`] or
    /// [`Self::with_remote_host`]), otherwise defaults to `localhost`.
    ///
    /// WHAT: `http://localhost/v1.53` for Unix-socket, `http://192.168.1.10:2375/v1.53`
    /// for remote.
    ///
    /// This does not negotiate: before negotiation it carries
    /// [`DEFAULT_API_VERSION`], which an older daemon rejects with `400`. Build
    /// request URLs with [`Self::api_url`].
    #[must_use]
    pub fn base_url(&self) -> String {
        format!("{}/v{}", self.unversioned_base_url(), self.api_version())
    }

    /// The base URL for API calls, negotiating the API version with the daemon
    /// first if this client has not done so yet.
    ///
    /// Every request method on [`DockerClient`] builds its URL with this.
    ///
    /// # Errors
    ///
    /// Returns the [`DockerError`] from [`Self::negotiate_version`].
    pub async fn api_url(&self) -> Result<String, DockerError> {
        self.negotiate_version().await?;
        Ok(self.base_url())
    }

    /// The Unix socket path this client dials.
    #[must_use]
    pub fn socket_path(&self) -> &std::path::Path {
        &self.socket_path
    }

    /// Fetch daemon version info (`GET /version`).
    ///
    /// Sent unversioned: version negotiation is built on it, so it must work
    /// before a version is agreed on.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn version(&self) -> Result<SystemVersion, DockerError> {
        let response = system_version_request(
            self.api(),
            &SystemVersionArgs::default(),
            &self.unversioned_base_url(),
            None::<NoMod>,
        )
        .await?;
        Ok(response.body)
    }

    /// Negotiate the API version with the daemon, once per client.
    ///
    /// WHY: The client and the daemon each support a range of API versions. A
    /// daemon rejects a request for a version newer than its own `ApiVersion`
    /// with `400 Bad Request`, so a client that hard-codes its newest version
    /// cannot talk to an older daemon at all (GitHub's runners have one).
    ///
    /// WHAT: Reads the daemon's `ApiVersion` and `MinAPIVersion` and settles on
    /// `min(DEFAULT_API_VERSION, ApiVersion)`; see [`negotiate_api_version`].
    /// The result is cached: later calls, and clones of this client, reuse it
    /// without asking the daemon again. A failed negotiation is not cached.
    ///
    /// HOW: unversioned `GET /version`.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError::Transport`] if the daemon cannot be reached,
    /// [`DockerError::Api`] if it answers `/version` with an error status,
    /// [`DockerError::JsonParse`] if it reports no usable `ApiVersion`, and
    /// [`DockerError::UnsupportedApiVersion`] if the versions do not overlap.
    pub async fn negotiate_version(&self) -> Result<(), DockerError> {
        if self.negotiated_version.get().is_some() {
            return Ok(());
        }
        let version = self.version().await?;
        let agreed = negotiate_api_version(
            DEFAULT_API_VERSION,
            version.api_version.as_deref(),
            version.min_api_version.as_deref(),
        )?;
        // Two concurrent first requests may both negotiate. They asked the same
        // daemon, so they agree, and whichever stores first wins: `set` failing
        // only means the other one got there first.
        if let Err(ours) = self.negotiated_version.set(agreed) {
            debug_assert_eq!(Some(&ours), self.negotiated_version.get());
        }
        Ok(())
    }

    /// Create a container (`POST /containers/create`).
    ///
    /// WHAT: Reads `config` into the generated `ContainerCreateRequest` and
    /// sends it; returns the created container id + warnings.
    ///
    /// Accepts any `impl Serialize` — `serde_json::json!({...})` for quick inline
    /// configs, or [`ContainerCreateBody`] for typed construction.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure, non-2xx status, or a config
    /// that cannot be serialized or does not fit `ContainerCreateRequest`
    /// ([`DockerError::JsonParse`]).
    pub async fn create_container(
        &self,
        config: &impl Serialize,
        name: Option<&str>,
    ) -> Result<ContainerCreateResponse, DockerError> {
        let json = serde_json::to_value(config)
            .map_err(|e| DockerError::JsonParse(format!("serialize container config: {e}")))?;
        let args = ContainerCreateArgs {
            name: name.map(str::to_string),
            platform: None,
            body: generated_body("container config", json)?,
        };
        let response =
            container_create_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(response.body)
    }

    /// Start a container (`POST /containers/{id}/start`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn start_container(&self, id: &str) -> Result<(), DockerError> {
        let args = ContainerStartArgs {
            id: id.to_string(),
            detach_keys: None,
        };
        container_start_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(())
    }

    /// Wait for a container to reach a condition (`POST /containers/{id}/wait`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn wait_container(
        &self,
        id: &str,
        condition: Option<&str>,
    ) -> Result<ContainerWaitResponse, DockerError> {
        let args = ContainerWaitArgs {
            id: id.to_string(),
            condition: condition.map(str::to_string),
        };
        let response = container_wait_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(response.body)
    }

    /// Stop a container (`POST /containers/{id}/stop`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx/304 status.
    ///
    /// # Note
    ///
    /// Docker returns `304 Not Modified` when the container is already stopped
    /// (or never started). This is not an error — the desired state is reached.
    pub async fn stop_container(
        &self,
        id: &str,
        timeout_secs: Option<u32>,
    ) -> Result<(), DockerError> {
        let args = ContainerStopArgs {
            id: id.to_string(),
            signal: None,
            t: timeout_secs.map(|t| t.to_string()),
        };
        match container_stop_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await {
            Ok(_) => Ok(()),
            // Docker returns 304 when the container is already exited — not an error.
            Err(ApiError::HttpStatus { code: 304, .. }) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Remove a container (`DELETE /containers/{id}`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn remove_container(&self, id: &str, force: bool) -> Result<(), DockerError> {
        let args = ContainerDeleteArgs {
            id: id.to_string(),
            v: None,
            force: Some(force.to_string()),
            link: None,
        };
        container_delete_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(())
    }

    /// Inspect a container (`GET /containers/{id}/json`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn inspect_container(
        &self,
        id: &str,
    ) -> Result<ContainerInspectResponse, DockerError> {
        let args = ContainerInspectArgs {
            id: id.to_string(),
            size: None,
        };
        let response = container_inspect_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(response.body)
    }

    /// Pause a container (`POST /containers/{id}/pause`).
    ///
    /// WHY: Freezes all processes in the container via cgroups freezer.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn pause_container(&self, id: &str) -> Result<(), DockerError> {
        let args = ContainerPauseArgs {
            id: id.to_string(),
        };
        container_pause_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(())
    }

    /// Unpause a container (`POST /containers/{id}/unpause`).
    ///
    /// WHY: Resumes a paused container.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn unpause_container(&self, id: &str) -> Result<(), DockerError> {
        let args = ContainerUnpauseArgs {
            id: id.to_string(),
        };
        container_unpause_request(self.api(), &args, &self.api_url().await?, None::<NoMod>).await?;
        Ok(())
    }

    // ── Exec ───────────────────────────────────────────────────────────

    /// Create an exec instance in a running container (`POST /containers/{id}/exec`).
    ///
    /// Delegates to [`exec::exec_create`].
    pub async fn exec_create(&self, id: &str, cmd: &[&str]) -> Result<IDResponse, DockerError> {
        exec::exec_create(self, id, cmd).await
    }

    /// Start a previously-created exec instance (`POST /exec/{id}/start`).
    ///
    /// Delegates to [`exec::exec_start`].
    pub async fn exec_start(&self, id: &str, detach: bool) -> Result<body::LogStream, DockerError> {
        exec::exec_start(self, id, detach).await
    }

    /// Inspect an exec instance (`GET /exec/{id}/json`).
    ///
    /// Delegates to [`exec::exec_inspect`].
    pub async fn exec_inspect(&self, id: &str) -> Result<serde_json::Value, DockerError> {
        exec::exec_inspect(self, id).await
    }

    /// Resize the TTY of an exec instance (`POST /exec/{id}/resize`).
    ///
    /// Delegates to [`exec::exec_resize`].
    pub async fn exec_resize(&self, id: &str, h: u32, w: u32) -> Result<(), DockerError> {
        exec::exec_resize(self, id, h, w).await
    }

    // ── System ─────────────────────────────────────────────────────────

    /// Get system information (`GET /info`).
    ///
    /// Delegates to [`system::system_info`].
    ///
    /// Returns `serde_json::Value` because Docker's `LocalNodeState` is a raw
    /// string (`"inactive"`) that the generated `SystemInfo` struct can't parse.
    pub async fn system_info(&self) -> Result<serde_json::Value, DockerError> {
        system::system_info(self).await
    }

    /// Ping the daemon (`GET /_ping`).
    ///
    /// Delegates to [`system::system_ping`].
    pub async fn system_ping(&self) -> Result<(), DockerError> {
        system::system_ping(self).await
    }

    /// Stream daemon events (`GET /events`).
    ///
    /// Delegates to [`system::system_events`].
    pub async fn system_events(
        &self,
        since: Option<&str>,
        until: Option<&str>,
        filters: Option<&str>,
    ) -> Result<body::EventStream, DockerError> {
        system::system_events(self, since, until, filters).await
    }

    /// Get data usage information (`GET /system/df`).
    ///
    /// Delegates to [`system::system_data_usage`].
    pub async fn system_data_usage(&self, types: Option<&str>) -> Result<serde_json::Value, DockerError> {
        system::system_data_usage(self, types).await
    }

    /// Check auth configuration (`POST /auth`).
    ///
    /// Delegates to [`system::system_auth`].
    pub async fn system_auth(&self, username: &str, password: &str) -> Result<AuthResponse, DockerError> {
        system::system_auth(self, username, password).await
    }
}

/// Read a caller's JSON request body into the generated request type it must
/// fit, so what goes on the wire is the spec's shape.
///
/// # Errors
///
/// [`DockerError::JsonParse`] naming `what`, if `value` does not fit `T`.
pub(crate) fn generated_body<T: serde::de::DeserializeOwned>(
    what: &str,
    value: serde_json::Value,
) -> Result<T, DockerError> {
    serde_json::from_value(value)
        .map_err(|e| DockerError::JsonParse(format!("{what} does not fit the API's request type: {e}")))
}

/// A Docker Engine API version, `major.minor` (e.g. `1.53`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ApiVersion {
    major: u32,
    minor: u32,
}

impl ApiVersion {
    /// Parse `"major.minor"`. Anything else is an error rather than a guess:
    /// a misread version would pick the wrong API for every request.
    fn parse(s: &str) -> Result<Self, DockerError> {
        let invalid = || DockerError::JsonParse(format!("invalid Docker API version {s:?}"));
        let (major, minor) = s.trim().split_once('.').ok_or_else(invalid)?;
        Ok(Self {
            major: major.parse().map_err(|_| invalid())?,
            minor: minor.parse().map_err(|_| invalid())?,
        })
    }
}

/// Pick the API version to talk to a daemon with, as the `docker` CLI does:
/// the lower of the client's newest version and the daemon's `ApiVersion`,
/// provided the daemon still accepts it (`MinAPIVersion`).
///
/// Returns the chosen version as the side it came from wrote it.
///
/// # Errors
///
/// - [`DockerError::JsonParse`] if the daemon reported no `ApiVersion`, or a
///   version is not `major.minor`.
/// - [`DockerError::UnsupportedApiVersion`] if the daemon's minimum is newer
///   than the client's newest version, so there is no version both speak.
pub fn negotiate_api_version(
    client_max: &str,
    server_max: Option<&str>,
    server_min: Option<&str>,
) -> Result<String, DockerError> {
    let server_max_str = server_max.ok_or_else(|| {
        DockerError::JsonParse("the daemon's GET /version reported no ApiVersion".to_string())
    })?;
    let client = ApiVersion::parse(client_max)?;
    let server = ApiVersion::parse(server_max_str)?;
    let (chosen, chosen_str) = if server < client {
        (server, server_max_str)
    } else {
        (client, client_max)
    };
    if let Some(min_str) = server_min {
        if chosen < ApiVersion::parse(min_str)? {
            return Err(DockerError::UnsupportedApiVersion {
                client_max: client_max.to_string(),
                server_min: min_str.to_string(),
                server_max: server_max_str.to_string(),
            });
        }
    }
    Ok(chosen_str.trim().to_string())
}

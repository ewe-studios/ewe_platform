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

use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use serde::Serialize;
use std::path::PathBuf;

use crate::error::DockerError;
use crate::generated::auth::AuthResponse;
use crate::generated::containers::{container_delete_request, ContainerDeleteArgs};
use crate::generated::create::{container_create_request, ContainerCreateArgs, ContainerCreateResponse};
use crate::generated::info::SystemInfo;
use crate::generated::json::{container_inspect_request, ContainerInspectArgs, ContainerInspectResponse};
use crate::generated::pause::{container_pause_request, ContainerPauseArgs};
use crate::generated::shared::IDResponse;
use crate::generated::start::{container_start_request, ContainerStartArgs};
use crate::generated::stop::{container_stop_request, ContainerStopArgs};
use crate::generated::unpause::{container_unpause_request, ContainerUnpauseArgs};
use crate::generated::version::{system_version_request, SystemVersion, SystemVersionArgs};
use crate::generated::wait::{container_wait_request, ContainerWaitArgs, ContainerWaitResponse};

/// Default Docker daemon Unix socket path.
pub const DEFAULT_DOCKER_SOCKET: &str = "/var/run/docker.sock";

/// The Docker Engine API version this client targets.
pub const DEFAULT_API_VERSION: &str = "1.53";

/// A closure type for the (empty) `builder_mod` argument — no request mutation.
pub(crate) type NoMod = fn(&mut PreparedRequestBuilder);

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

/// Exec instance operations (create, start, inspect, resize).
pub mod exec;

/// System operations (info, ping, events, data usage, auth).
/// (`version` lives directly on [`DockerClient`].)
pub mod system;

/// Docker daemon client over a Unix socket.
///
/// WHY: Talks to `dockerd` without bollard/tokio — a `DynNetClient` over the Unix
/// socket, driven by valtron.
///
/// WHAT: Holds the client handle, the socket path, and the negotiated API version.
///
/// HOW: Constructed via [`DockerClient::connect_unix`] /
/// [`DockerClient::connect_with_defaults`].
#[derive(Clone)]
pub struct DockerClient {
    http: DynNetClient,
    socket_path: PathBuf,
    api_version: String,
    /// Optional remote host:port for TCP connections (e.g. "192.168.1.10:2375").
    /// When `None`, the base URL uses `localhost` (the Unix-socket default).
    remote_host: Option<String>,
}

impl std::fmt::Debug for DockerClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockerClient")
            .field("socket_path", &self.socket_path)
            .field("api_version", &self.api_version)
            .field("remote_host", &self.remote_host)
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
            api_version: DEFAULT_API_VERSION.to_string(),
            remote_host: None,
        }
    }

    /// Connect via TCP to a remote Docker daemon (e.g. "192.168.1.10:2375").
    ///
    /// WHY: Docker daemons can be exposed over TCP (TLS or plaintext). This is the
    /// remote / non-localhost path.
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
            api_version: DEFAULT_API_VERSION.to_string(),
            remote_host: Some(host.to_string()),
        }
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
    /// HOW: Parses `DOCKER_HOST`; falls back to `/var/run/docker.sock`.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError::Unavailable`] if `DOCKER_HOST` names a non-unix
    /// transport (tcp/ssh) — not yet supported.
    pub fn connect_with_defaults() -> Result<Self, DockerError> {
        let path = match std::env::var("DOCKER_HOST") {
            Ok(host) if host.starts_with("unix://") => {
                PathBuf::from(host.trim_start_matches("unix://"))
            }
            Ok(host) if host.is_empty() => PathBuf::from(DEFAULT_DOCKER_SOCKET),
            Ok(host) => {
                return Err(DockerError::Unavailable(format!(
                    "unsupported DOCKER_HOST transport (only unix:// supported): {host}"
                )))
            }
            Err(_) => PathBuf::from(DEFAULT_DOCKER_SOCKET),
        };
        Ok(Self::connect_unix(path))
    }

    /// The cross-platform HTTP client handle (cheap `Arc` clone).
    #[must_use]
    pub fn http(&self) -> DynNetClient {
        self.http.clone()
    }

    /// The negotiated (or default) API version, e.g. `"1.53"`.
    #[must_use]
    pub fn api_version(&self) -> &str {
        &self.api_version
    }

    /// The base URL for API calls: `http://{host}/v{version}`.
    ///
    /// WHY: Generated functions take a `base_url: &str` so callers can point at
    /// remote Docker daemons over TCP, not just Unix-socket-local. Uses
    /// `remote_host` when set (via [`Self::connect_tcp`] or
    /// [`Self::with_remote_host`]), otherwise defaults to `localhost`.
    ///
    /// WHAT: `http://localhost/v1.53` for Unix-socket, `http://192.168.1.10:2375/v1.53`
    /// for remote.
    #[must_use]
    pub fn base_url(&self) -> String {
        let host = self.remote_host.as_deref().unwrap_or("localhost");
        format!("http://{host}/v{}", self.api_version)
    }

    /// The Unix socket path this client dials.
    #[must_use]
    pub fn socket_path(&self) -> &std::path::Path {
        &self.socket_path
    }

    /// Fetch daemon version info (`GET /version`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn version(&self) -> Result<SystemVersion, DockerError> {
        let response =
            system_version_request(self.http(), &SystemVersionArgs::default(), &self.base_url(), None::<NoMod>)
                .await?;
        Ok(response.body)
    }

    /// Negotiate the effective API version with the daemon.
    ///
    /// WHY: The client and server may support different API versions; Docker
    /// negotiates the minimum common version.
    ///
    /// WHAT: Reads the daemon's `ApiVersion`; picks the lower of client/server.
    ///
    /// HOW: `GET /version` → compare with [`DEFAULT_API_VERSION`].
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
    pub async fn negotiate_version(&mut self) -> Result<(), DockerError> {
        let version = self.version().await?;
        if let Some(server) = version.api_version {
            // Both are dotted "major.minor"; the lexicographically-lower valid
            // version is the safe common denominator for our fixed client version.
            if version_lt(&server, &self.api_version) {
                self.api_version = server;
            }
        }
        Ok(())
    }

    /// Create a container (`POST /containers/create`).
    ///
    /// WHY: The generated `ContainerCreateArgs` carries only the `name`/`platform`
    /// query params (the request body was an inline `allOf` the generator did not
    /// name), so the container config is injected through the request builder.
    ///
    /// WHAT: Serializes `config` as the JSON body and returns the created id.
    ///
    /// HOW: `container_create_request` with a `builder_mod` that sets the JSON body.
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure, non-2xx status, or a config
    /// that cannot be serialized.
    pub async fn create_container<C: Serialize>(
        &self,
        config: &C,
        name: Option<&str>,
    ) -> Result<ContainerCreateResponse, DockerError> {
        let body = serde_json::to_value(config)
            .map_err(|e| DockerError::JsonParse(format!("serialize container config: {e}")))?;
        let args = ContainerCreateArgs {
            name: name.map(str::to_string),
            platform: None,
        };
        let response = container_create_request(
            self.http(),
            &args,
            &self.base_url(),
            Some(move |b: &mut PreparedRequestBuilder| {
                b.set_header(SimpleHeader::CONTENT_TYPE, "application/json");
                let _ = b.set_body_json(&body);
            }),
        )
        .await?;
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
        container_start_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
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
        let response = container_wait_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(response.body)
    }

    /// Stop a container (`POST /containers/{id}/stop`).
    ///
    /// # Errors
    ///
    /// Returns [`DockerError`] on transport failure or non-2xx status.
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
        container_stop_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
        Ok(())
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
        container_delete_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
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
        let response = container_inspect_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
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
        container_pause_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
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
        container_unpause_request(self.http(), &args, &self.base_url(), None::<NoMod>).await?;
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
    pub async fn exec_start(&self, id: &str, detach: bool) -> Result<Vec<u8>, DockerError> {
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
    pub async fn system_info(&self) -> Result<SystemInfo, DockerError> {
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
    ) -> Result<Vec<u8>, DockerError> {
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

/// Compare two dotted `"major.minor"` version strings, returning `true` when
/// `a < b`. Non-numeric components sort as 0.
///
/// # Panics
///
/// Never panics.
fn version_lt(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<u32> {
        s.split('.').map(|p| p.parse().unwrap_or(0)).collect()
    };
    parse(a) < parse(b)
}

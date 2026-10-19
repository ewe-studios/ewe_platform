//! Container lifecycle — the RAII guard for running Docker containers.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};

use foundation_deployment_docker::client::containers_lifecycle::ContainerLogsOptions;
use foundation_deployment_docker::streaming::decoder::LogOutput;

use foundation_deployment_docker::client::{
    ContainerCreateBody, ContainerHostConfig, NetworkingConfig,
};
use foundation_deployment_docker::generated::connect::EndpointSettings;
use foundation_deployment_docker::generated::json::{
    ContainerConfig as GenContainerConfig, DeviceMapping, HostConfig, PortMap, Resources,
};
use foundation_deployment_docker::DockerClient;
use tracing::{debug, info, warn};

use crate::docker::config::{parse_memory_bytes, ContainerConfig};
use crate::docker::network::NetworkHandle;
use crate::docker::error::{classify_docker_client_err, docker_err, DockerError, DockerResult};

/// RAII guard for a running Docker container.
pub struct ContainerHandle {
    docker: DockerClient,
    container_id: String,
    container_name: Option<String>,
    stop_timeout: u64,
    ports: HashMap<String, u16>,
    /// The address this process reaches the published ports at — see
    /// [`ContainerHandle::host_ip`].
    host_ip: Ipv4Addr,
}

/// What a command run with [`ContainerHandle::exec_async`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecOutput {
    /// The command's exit code.
    pub exit_code: i64,
    /// Everything it wrote to stdout.
    pub stdout: String,
    /// Everything it wrote to stderr.
    pub stderr: String,
}

impl ExecOutput {
    /// `true` when the command exited 0.
    #[must_use]
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }
}

/// The container this process runs in, when the daemon it talks to runs it —
/// a CI job container with the daemon's socket mounted, as GitHub Actions'
/// `container:` jobs have.
#[derive(Debug)]
pub struct CallerContainer {
    /// The container's id.
    pub id: String,
    /// Its `NetworkSettings.Networks`, as the daemon reports them.
    pub networks: Option<serde_json::Value>,
}

/// Find the container this process runs in, if this daemon runs it.
///
/// HOW: The process is in a container when `/.dockerenv` exists; it is one of
/// this daemon's when the daemon knows a container by this host's name (Docker
/// sets the hostname to the container id). A remote daemon (`tcp://`,
/// `ssh://`) runs its containers on another machine, so this process cannot be
/// one of them.
///
/// # Errors
/// Returns the daemon's error if inspecting the container fails for any reason
/// other than "no such container", or if `/etc/hostname` cannot be read.
pub(crate) async fn caller_container(docker: &DockerClient) -> DockerResult<Option<CallerContainer>> {
    if !docker.socket_path().is_absolute() || !std::path::Path::new("/.dockerenv").exists() {
        return Ok(None);
    }
    let hostname = match std::fs::read_to_string("/etc/hostname") {
        Ok(h) => h.trim().to_string(),
        // No hostname file: nothing to look this container up by.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(docker_err(DockerError::InvalidConfig(format!(
                "reading /etc/hostname to find this container: {e}"
            ))));
        }
    };
    if hostname.is_empty() {
        return Ok(None);
    }
    match docker.inspect_container(&hostname).await {
        Ok(info) => Ok(Some(CallerContainer {
            id: info.id.unwrap_or(hostname),
            networks: info.network_settings.and_then(|ns| ns.networks),
        })),
        // A container, but not one this daemon runs (a sandbox, another
        // engine).
        Err(foundation_deployment_docker::DockerError::Api { status: 404, .. }) => Ok(None),
        Err(e) => Err(docker_err(classify_docker_client_err("inspect_container (self)", e))),
    }
}

/// Where this process reaches the ports Docker publishes, which is also the
/// host address they are published on.
///
/// WHY: Published ports are bound on the Docker host. A process running on the
/// host reaches them on `127.0.0.1`. A process running inside a container of
/// the same daemon has its own loopback, so `127.0.0.1` reaches nothing there;
/// it reaches the host on its network's gateway address instead.
///
/// HOW: The caller container's first network with a gateway (by name, for
/// determinism). A container on the host network shares the host's loopback,
/// and has no gateway, so it stays on `127.0.0.1`.
#[must_use]
pub fn published_port_host(caller: Option<&CallerContainer>) -> Ipv4Addr {
    caller
        .and_then(|c| gateway_of(c.networks.as_ref()))
        .unwrap_or(Ipv4Addr::LOCALHOST)
}

/// The network mode for a container that shares the caller's network
/// namespace: the caller's container, or the host's when the caller runs on it.
#[must_use]
pub fn caller_network_mode(caller: Option<&CallerContainer>) -> String {
    match caller {
        Some(c) => format!("container:{}", c.id),
        None => "host".to_string(),
    }
}

/// The gateway of the first network (by name) in an inspect response's
/// `NetworkSettings.Networks` that has an IPv4 one.
#[must_use]
pub fn gateway_of(networks: Option<&serde_json::Value>) -> Option<Ipv4Addr> {
    let networks = networks?.as_object()?;
    let mut names: Vec<&String> = networks.keys().collect();
    names.sort();
    names.into_iter().find_map(|name| {
        networks[name]
            .get("Gateway")
            .and_then(serde_json::Value::as_str)
            .and_then(|g| g.parse::<Ipv4Addr>().ok())
    })
}

impl ContainerHandle {
    /// Start a container: pull → create → start → inspect → wait.
    pub async fn start_async(config: ContainerConfig) -> DockerResult<Self> {
        let docker = DockerClient::connect_with_defaults()
            .map_err(|e| docker_err(classify_docker_client_err("connect", e)))?;

        // Validated up front: a bad memory limit is a config mistake, and
        // failing here beats creating a container that silently ignores it.
        let memory_bytes = match config.memory.as_deref() {
            Some(mem) => Some(
                parse_memory_bytes(mem).map_err(|e| docker_err(DockerError::InvalidConfig(e)))?,
            ),
            None => None,
        };

        // Sharing the caller's network namespace means sharing its ports too:
        // there is nothing to publish, and Docker refuses to try.
        if config.share_caller_network && !config.ports.is_empty() {
            return Err(docker_err(DockerError::InvalidConfig(format!(
                "{}: a container sharing the caller's network cannot publish ports \
                 (it already shares the caller's)",
                config.image
            ))));
        }

        match &config.build {
            // Built (or found already built) under `image`: nothing to pull.
            Some(dockerfile) => {
                dockerfile.build_once().await?;
            }
            None => Self::ensure_image(&docker, &config.image, config.always_pull).await?,
        }

        let caller = caller_container(&docker).await?;
        let network_mode = if config.share_caller_network {
            Some(caller_network_mode(caller.as_ref()))
        } else {
            // A user-defined network must exist before a container can join it —
            // Docker fails the create otherwise. `create_or_find` is idempotent,
            // so naming a network is enough to get one (Decision 10). It is
            // deliberately left in place afterwards: it may be shared with
            // containers this handle knows nothing about, so it is not ours to
            // remove.
            if let Some(ref network) = config.network {
                NetworkHandle::create_or_find(&docker, network, None).await?;
            }
            config.network.clone()
        };

        let host_ip = published_port_host(caller.as_ref());
        let body = Self::build_body(&config, memory_bytes, host_ip, network_mode);

        let created = docker
            .create_container(&body, config.name.as_deref())
            .await
            .map_err(|e| docker_err(DockerError::ContainerCreate(format!("{e}"))))?;

        let container_id = created.id;

        // Past this point the container exists in Docker, but no `ContainerHandle`
        // owns it yet — so nothing would run Drop's teardown. Any failure from
        // here on must remove it by hand, or a failed start (a port conflict, a
        // readiness timeout) strands a container that keeps holding its ports and
        // makes every later run fail the same way.
        match Self::finish_start(&docker, &container_id, &config, host_ip).await {
            Ok(ports) => Ok(Self {
                docker,
                container_id,
                container_name: config.name.clone(),
                stop_timeout: config.stop_timeout,
                ports,
                host_ip,
            }),
            Err(e) => {
                Self::discard(&docker, &container_id, config.stop_timeout).await;
                Err(e)
            }
        }
    }

    /// Start → inspect ports → await readiness, for a container that already
    /// exists. Split out of `start_async` so a single cleanup path covers every
    /// way these steps can fail.
    async fn finish_start(
        docker: &DockerClient,
        container_id: &str,
        config: &ContainerConfig,
        host_ip: Ipv4Addr,
    ) -> DockerResult<HashMap<String, u16>> {
        docker
            .start_container(container_id)
            .await
            .map_err(|e| docker_err(DockerError::ContainerStart(format!("{e}"))))?;

        let ports = Self::resolve_ports(docker, container_id, config).await?;

        config.wait.apply(docker, container_id, host_ip, &ports).await?;

        Ok(ports)
    }

    /// Best-effort teardown of a container no handle owns. Errors are logged,
    /// never propagated: the caller is already returning the real failure and
    /// must not have it masked by a cleanup problem.
    async fn discard(docker: &DockerClient, container_id: &str, stop_timeout: u64) {
        if let Err(e) = docker.stop_container(container_id, Some(stop_timeout as u32)).await {
            warn!(container_id = %container_id, "start cleanup: failed to stop container: {e}");
        }
        if let Err(e) = docker.remove_container(container_id, true).await {
            warn!(container_id = %container_id, "start cleanup: failed to remove container: {e}");
        }
    }

    /// Sync convenience — delegates to `start_async()` via `block_on`.
    pub fn start(config: ContainerConfig) -> DockerResult<Self> {
        crate::block_on(Self::start_async(config))
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.container_id
    }

    /// The container name, if set via ContainerConfig::name().
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.container_name.as_deref()
    }

    #[must_use]
    pub fn host_port(&self, container_port: u16) -> Option<u16> {
        let key = format!("{container_port}/tcp");
        self.ports.get(&key).copied()
    }

    /// The host port mapped to `container_port`'s UDP binding.
    #[must_use]
    pub fn host_port_udp(&self, container_port: u16) -> Option<u16> {
        let key = format!("{container_port}/udp");
        self.ports.get(&key).copied()
    }

    #[must_use]
    pub fn host_ports(&self) -> &HashMap<String, u16> {
        &self.ports
    }

    /// The host address this process reaches the published ports at:
    /// `127.0.0.1` on the Docker host itself, or the host's address on this
    /// container's network when the process runs in a container of the same
    /// daemon (a CI job container). Pair it with [`Self::host_port`], or use
    /// [`Self::address`], which does.
    #[must_use]
    pub fn host_ip(&self) -> Ipv4Addr {
        self.host_ip
    }

    /// The host-side address `container_port` is reachable at, e.g.
    /// `127.0.0.1:49153`. This is the address to connect to from the test
    /// process, and it is the reason a caller rarely needs `port_mapped`: let
    /// Docker assign the host port with `port = N` and ask the handle where it
    /// landed, instead of pinning a host port that a parallel run may already
    /// hold. The IP is [`Self::host_ip`].
    ///
    /// `None` if `container_port` was never exposed as TCP.
    #[must_use]
    pub fn address(&self, container_port: u16) -> Option<SocketAddr> {
        self.host_port(container_port)
            .map(|p| SocketAddr::from((self.host_ip, p)))
    }

    /// The host-side UDP address for `container_port`. `None` if it was never
    /// exposed as UDP.
    #[must_use]
    pub fn udp_address(&self, container_port: u16) -> Option<SocketAddr> {
        self.host_port_udp(container_port)
            .map(|p| SocketAddr::from((self.host_ip, p)))
    }

    pub async fn shutdown_async(&self) -> DockerResult<()> {
        if let Err(e) = self
            .docker
            .stop_container(&self.container_id, Some(self.stop_timeout as u32))
            .await
        {
            warn!(container_id = %self.container_id, "failed to stop container: {e}");
        }

        if let Err(e) = self
            .docker
            .remove_container(&self.container_id, true)
            .await
        {
            warn!(container_id = %self.container_id, "failed to remove container: {e}");
        }

        Ok(())
    }

    pub fn shutdown(&self) -> DockerResult<()> {
        crate::block_on(self.shutdown_async())
    }

    /// The container's logs so far, stdout and stderr interleaved, with
    /// Docker's stream framing already removed.
    ///
    /// # Errors
    /// Returns [`DockerError::Connection`] if the daemon rejects the request.
    pub async fn logs_async(&self) -> DockerResult<String> {
        let mut frames = self
            .docker
            .container_logs(&self.container_id, &ContainerLogsOptions::default())
            .await
            .map_err(|e| docker_err(classify_docker_client_err("container_logs", e)))?;

        // Decoded frame by frame as the body arrives; stdout and stderr are
        // returned interleaved, as before, so both go to the one string.
        let mut out = String::new();
        while let Some(frame) = frames.next().await {
            match frame.map_err(|e| docker_err(classify_docker_client_err("container_logs", e)))? {
                LogOutput::StdOut { message } | LogOutput::StdErr { message } => {
                    out.push_str(&String::from_utf8_lossy(&message));
                }
                LogOutput::StdIn { .. } | LogOutput::Console { .. } => {}
            }
        }
        Ok(out)
    }

    /// Waits for the container to exit, then returns its logs.
    ///
    /// For a one-shot container — one given a `command` that runs and exits —
    /// this is the way to read what it printed.
    ///
    /// Uses the daemon's `/wait` endpoint rather than polling `State.Running`:
    /// a container that has been created but not yet scheduled *also* reports
    /// `Running == false`, so polling that flag returns the instant the
    /// container starts and hands back empty logs. `/wait` blocks until the
    /// container has genuinely exited.
    ///
    /// # Errors
    /// Returns [`DockerError::Connection`] if the daemon rejects a request.
    pub async fn wait_for_exit_async(&self) -> DockerResult<String> {
        self.docker
            .wait_container(&self.container_id, Some("not-running"))
            .await
            .map_err(|e| docker_err(classify_docker_client_err("wait_container", e)))?;

        self.logs_async().await
    }

    /// Run `cmd` inside the container and wait for it to finish — `docker
    /// exec`, over the API (no `docker` CLI needed).
    ///
    /// # Errors
    /// Returns the daemon's error (classified, see
    /// [`crate::docker::error::classify_docker_client_err`]) if the exec cannot
    /// be created, run or inspected, and [`DockerError::InvalidConfig`] if the
    /// daemon reports no exit code for a finished exec.
    pub async fn exec_async(&self, cmd: &[&str]) -> DockerResult<ExecOutput> {
        let exec = self
            .docker
            .exec_create(&self.container_id, cmd)
            .await
            .map_err(|e| docker_err(classify_docker_client_err("exec_create", e)))?;
        // Attached (not detached): the call returns once the command has
        // exited, with its multiplexed stdout/stderr as the body.
        let mut out = ExecOutput { exit_code: 0, stdout: String::new(), stderr: String::new() };
        // The command's output, decoded as it streams; the daemon ends the
        // stream when the command exits.
        self.docker
            .exec_start(&exec.id, false)
            .await
            .map_err(|e| docker_err(classify_docker_client_err("exec_start", e)))?
            .read_into(&mut out.stdout, &mut out.stderr)
            .await
            .map_err(|e| docker_err(classify_docker_client_err("exec_start", e)))?;
        let info = self
            .docker
            .exec_inspect(&exec.id)
            .await
            .map_err(|e| docker_err(classify_docker_client_err("exec_inspect", e)))?;
        out.exit_code = info
            .get("ExitCode")
            .and_then(serde_json::Value::as_i64)
            .ok_or_else(|| {
                docker_err(DockerError::InvalidConfig(format!(
                    "exec {cmd:?}: the daemon reported no ExitCode: {info}"
                )))
            })?;

        Ok(out)
    }

    /// Sync convenience: [`Self::exec_async`] via `block_on`, for test bodies
    /// that are not async.
    ///
    /// # Errors
    /// As [`Self::exec_async`].
    pub fn exec(&self, cmd: &[&str]) -> DockerResult<ExecOutput> {
        crate::block_on(self.exec_async(cmd))
    }

    pub async fn is_running_async(&self) -> DockerResult<bool> {
        let info = self
            .docker
            .inspect_container(&self.container_id)
            .await
            .map_err(|e| docker_err(classify_docker_client_err("inspect_container", e)))?;

        Ok(info.state.and_then(|s| s.running).unwrap_or(false))
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        crate::block_on(self.is_running_async()).unwrap_or(false)
    }

    // ── Private ──

    /// Pull the image when forced, or when it is not already in the local cache.
    async fn ensure_image(
        docker: &DockerClient,
        image: &str,
        always_pull: bool,
    ) -> DockerResult<()> {
        if !always_pull {
            match docker.image_inspect(image).await {
                Ok(_) => return Ok(()),
                // 404 = not cached → fall through to pull.
                Err(foundation_deployment_docker::DockerError::Api { status: 404, .. }) => {}
                Err(e) => return Err(docker_err(classify_docker_client_err("image_inspect", e))),
            }
        }
        Self::pull_image(docker, image).await
    }

    async fn pull_image(docker: &DockerClient, image: &str) -> DockerResult<()> {
        let (repo, tag) = Self::split_image_tag(image);
        info!(image = %image, "pulling image");

        let map_pull_err = |e| match e {
            foundation_deployment_docker::DockerError::PullFailed(reason) => {
                docker_err(DockerError::ImagePull { image: image.to_string(), reason })
            }
            other => docker_err(classify_docker_client_err("image_pull", other)),
        };
        let mut progress = docker
            .image_pull(
                Some(&repo),
                None, // from_src
                None, // repo (alias)
                tag.as_deref(),
                None, // message
                None, // platform
            )
            .await
            .map_err(map_pull_err)?;

        // The daemon pulls while it streams this progress, and reports a failed
        // pull inside it: read it to its end, item by item, so the image exists
        // when `create` runs and a failure surfaces as `ImagePull`.
        let mut items = 0usize;
        while let Some(item) = progress.next().await {
            let item = item.map_err(map_pull_err)?;
            items += 1;
            if let Some(status) = item.status.as_deref() {
                debug!(image = %image, layer = item.id.as_deref().unwrap_or(""), "{status}");
            }
        }
        info!(image = %image, progress_items = items, "pulled image");

        Ok(())
    }

    /// Split `"alpine:latest"` into `("alpine", Some("latest"))`. A registry port
    /// colon (e.g. `"localhost:5000/img"`) is not a tag separator.
    #[must_use]
    pub fn split_image_tag(image: &str) -> (String, Option<String>) {
        // Only a colon in the last path segment separates a tag; one before a
        // `/` is a registry port.
        let last_segment = image.rsplit('/').next().unwrap_or(image);
        if last_segment.contains(':') {
            if let Some((repo, tag)) = image.rsplit_once(':') {
                return (repo.to_string(), Some(tag.to_string()));
            }
        }
        (image.to_string(), None)
    }

    /// Build the typed [`ContainerCreateBody`] that [`Self::start_async`]
    /// sends for `config`: its ports published on `host_ip`, and the
    /// container on `network_mode` when one is given (the caller's network,
    /// see [`caller_network_mode`]) instead of the config's own networks.
    #[must_use]
    pub fn build_body(
        config: &ContainerConfig,
        memory_bytes: Option<i64>,
        host_ip: Ipv4Addr,
        network_mode: Option<String>,
    ) -> ContainerCreateBody {
        let host_ip = host_ip.to_string();
        let mut exposed_ports = serde_json::Map::new();
        let mut port_bindings = serde_json::Map::new();

        for pm in &config.ports {
            let port_key = format!(
                "{}/{}",
                pm.container_port,
                match pm.protocol {
                    crate::docker::config::PortProtocol::Tcp => "tcp",
                    crate::docker::config::PortProtocol::Udp => "udp",
                }
            );
            // ExposedPorts: keys mapped to empty objects.
            exposed_ports.insert(port_key.clone(), serde_json::Value::Object(Default::default()));

            // PortBindings: keys mapped to arrays of {HostIp, HostPort}.
            let host_port_str = pm
                .host_port
                .map(|p| p.to_string())
                .unwrap_or_else(|| String::new());
            port_bindings.insert(
                port_key,
                serde_json::json!([{
                    "HostIp": host_ip,
                    "HostPort": host_port_str,
                }]),
            );
        }

        // Docker bind syntax is `source:target[:ro]`, where `source` is a host
        // path for a bind mount and the volume's name for a named volume — the
        // daemon tells them apart by the leading `/`. Both kinds go in `Binds`,
        // and a read-only mount needs the explicit `:ro` suffix.
        let binds: Vec<String> = config
            .volumes
            .iter()
            .map(|v| {
                let source = match &v.source {
                    crate::docker::config::VolumeSource::Bind(host_path) => {
                        host_path.display().to_string()
                    }
                    crate::docker::config::VolumeSource::Named(name) => name.clone(),
                };
                let mode = if v.read_only { ":ro" } else { "" };
                format!("{source}:{}{mode}", v.target.display())
            })
            .collect();

        let host_config = ContainerHostConfig {
            base: HostConfig {
                port_bindings: if port_bindings.is_empty() {
                    None
                } else {
                    // `PortMap` is a free-form map in the spec: JSON itself.
                    Some(PortMap::Object(port_bindings))
                },
                binds: if binds.is_empty() { None } else { Some(binds) },
                cap_add: if config.cap_add.is_empty() {
                    None
                } else {
                    Some(config.cap_add.clone())
                },
                extra_hosts: if config.extra_hosts.is_empty() {
                    None
                } else {
                    Some(config.extra_hosts.clone())
                },
                network_mode: network_mode.clone(),
                ..Default::default()
            },
            resources: Resources {
                memory: memory_bytes,
                nano_cpus: config.cpus.map(|c| c as i64 * 1_000_000_000),
                devices: if config.devices.is_empty() {
                    None
                } else {
                    Some(
                        config
                            .devices
                            .iter()
                            .map(|d| DeviceMapping {
                                path_on_host: Some(d.host_path.display().to_string()),
                                path_in_container: d
                                    .container_path
                                    .as_ref()
                                    .map(|p| p.display().to_string()),
                                cgroup_permissions: Some("rwm".to_string()),
                            })
                            .collect(),
                    )
                },
                ..Default::default()
            },
        };

        let gen_config = GenContainerConfig {
            image: Some(config.image.clone()),
            env: if config.env.is_empty() {
                None
            } else {
                Some(
                    config
                        .env
                        .iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect(),
                )
            },
            cmd: config.command.clone(),
            exposed_ports: if exposed_ports.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(exposed_ports))
            },
            labels: if config.labels.is_empty() {
                None
            } else {
                Some(serde_json::to_value(&config.labels).unwrap_or_default())
            },
            ..Default::default()
        };

        // Network aliases are extra DNS names this container answers to, and
        // they attach to a *specific* network's endpoint — so they can only be
        // set when a network was named. Setting them at create time (rather than
        // connecting afterwards) means the container is resolvable by alias from
        // the moment it starts, with no window where a peer could miss it.
        // A container sharing the caller's namespace has no endpoint of its own.
        let named_network = if config.share_caller_network { None } else { config.network.as_ref() };
        let networking_config = match (named_network, config.network_aliases.is_empty()) {
            (Some(network), false) => {
                let mut endpoints_config = HashMap::new();
                endpoints_config.insert(
                    network.clone(),
                    EndpointSettings {
                        aliases: Some(config.network_aliases.clone()),
                        ..Default::default()
                    },
                );
                Some(NetworkingConfig { endpoints_config })
            }
            _ => None,
        };

        ContainerCreateBody {
            config: gen_config,
            host_config: Some(host_config),
            networking_config,
        }
    }

    async fn resolve_ports(
        docker: &DockerClient,
        container_id: &str,
        config: &ContainerConfig,
    ) -> DockerResult<HashMap<String, u16>> {
        let info = docker
            .inspect_container(container_id)
            .await
            .map_err(|e| {
                docker_err(classify_docker_client_err("inspect_container", e))
            })?;

        let mut ports = HashMap::new();

        if let Some(ns) = &info.network_settings {
            if let Some(bindings) = &ns.ports {
                for pm in &config.ports {
                    let key = format!(
                        "{}/{}",
                        pm.container_port,
                        match pm.protocol {
                            crate::docker::config::PortProtocol::Tcp => "tcp",
                            crate::docker::config::PortProtocol::Udp => "udp",
                        }
                    );
                    if let Some(binding) = bindings.get(&key) {
                        if let Some(arr) = binding.as_array() {
                            if let Some(first) = arr.first() {
                                if let Some(hp) = first
                                    .get("HostPort")
                                    .and_then(|v| v.as_str())
                                {
                                    if let Ok(port) = hp.parse::<u16>() {
                                        ports.insert(key, port);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(ports)
    }
}

impl Drop for ContainerHandle {
    fn drop(&mut self) {
        let docker = self.docker.clone();
        let id = self.container_id.clone();
        let timeout = self.stop_timeout;

        let _ = crate::block_on(async move {
            if let Err(e) = docker
                .stop_container(&id, Some(timeout as u32))
                .await
            {
                warn!(container_id = %id, "drop: failed to stop container: {e}");
            }
            if let Err(e) = docker.remove_container(&id, true).await {
                warn!(container_id = %id, "drop: failed to remove container: {e}");
            }
        });
    }
}

impl std::fmt::Debug for ContainerHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerHandle")
            .field("container_id", &self.container_id)
            .field("host_ip", &self.host_ip)
            .field("ports", &self.ports)
            .finish()
    }
}

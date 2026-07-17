//! Container lifecycle — the RAII guard for running Docker containers.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};

use foundation_deployment_docker::client::{ContainerCreateBody, ContainerHostConfig};
use foundation_deployment_docker::generated::json::{
    ContainerConfig as GenContainerConfig, DeviceMapping, HostConfig, PortMap, Resources,
};
use foundation_deployment_docker::DockerClient;
use tracing::{info, warn};

use crate::docker::config::{parse_memory_bytes, ContainerConfig};
use crate::docker::error::{docker_err, DockerError, DockerResult};

/// RAII guard for a running Docker container.
pub struct ContainerHandle {
    docker: DockerClient,
    container_id: String,
    container_name: Option<String>,
    stop_timeout: u64,
    ports: HashMap<String, u16>,
}

impl ContainerHandle {
    /// Start a container: pull → create → start → inspect → wait.
    pub async fn start_async(config: ContainerConfig) -> DockerResult<Self> {
        let docker = DockerClient::connect_with_defaults()
            .map_err(|e| docker_err(DockerError::Connection(format!("{e}"))))?;

        // Validated up front: a bad memory limit is a config mistake, and
        // failing here beats creating a container that silently ignores it.
        let memory_bytes = match config.memory.as_deref() {
            Some(mem) => Some(
                parse_memory_bytes(mem).map_err(|e| docker_err(DockerError::InvalidConfig(e)))?,
            ),
            None => None,
        };

        Self::ensure_image(&docker, &config.image, config.always_pull).await?;

        let body = Self::build_body(&config, memory_bytes);

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
        match Self::finish_start(&docker, &container_id, &config).await {
            Ok(ports) => Ok(Self {
                docker,
                container_id,
                container_name: config.name.clone(),
                stop_timeout: config.stop_timeout,
                ports,
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
    ) -> DockerResult<HashMap<String, u16>> {
        docker
            .start_container(container_id)
            .await
            .map_err(|e| docker_err(DockerError::ContainerStart(format!("{e}"))))?;

        let ports = Self::resolve_ports(docker, container_id, config).await?;

        config.wait.apply(docker, container_id, &ports).await?;

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

    /// The host-side address `container_port` is reachable at, e.g.
    /// `127.0.0.1:49153`. This is the address to connect to from the test
    /// process, and it is the reason a caller rarely needs `port_mapped`: let
    /// Docker assign the host port with `port = N` and ask the handle where it
    /// landed, instead of pinning a host port that a parallel run may already
    /// hold.
    ///
    /// `None` if `container_port` was never exposed as TCP.
    #[must_use]
    pub fn address(&self, container_port: u16) -> Option<SocketAddr> {
        self.host_port(container_port)
            .map(|p| SocketAddr::from((Ipv4Addr::LOCALHOST, p)))
    }

    /// The host-side UDP address for `container_port`. `None` if it was never
    /// exposed as UDP.
    #[must_use]
    pub fn udp_address(&self, container_port: u16) -> Option<SocketAddr> {
        self.host_port_udp(container_port)
            .map(|p| SocketAddr::from((Ipv4Addr::LOCALHOST, p)))
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

    pub async fn is_running_async(&self) -> DockerResult<bool> {
        let info = self
            .docker
            .inspect_container(&self.container_id)
            .await
            .map_err(|e| docker_err(DockerError::Connection(format!("inspect failed: {e}"))))?;

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
                Err(e) => {
                    return Err(docker_err(DockerError::Connection(format!(
                        "image_inspect: {e}"
                    ))));
                }
            }
        }
        Self::pull_image(docker, image).await
    }

    async fn pull_image(docker: &DockerClient, image: &str) -> DockerResult<()> {
        let (repo, tag) = Self::split_image_tag(image);
        info!(image = %image, "pulling image");

        docker
            .image_pull(
                Some(&repo),
                None, // from_src
                None, // repo (alias)
                tag.as_deref(),
                None, // message
                None, // platform
            )
            .await
            .map_err(|e| {
                docker_err(DockerError::ImagePull {
                    image: image.to_string(),
                    reason: format!("{e}"),
                })
            })?;

        Ok(())
    }

    /// Split `"alpine:latest"` into `("alpine", Some("latest"))`. A registry port
    /// colon (e.g. `"localhost:5000/img"`) is not a tag separator.
    fn split_image_tag(image: &str) -> (String, Option<String>) {
        // The last segment is after the final `/`. If it contains `:`, that's the tag.
        let last_segment = image.rsplit('/').next().unwrap_or(image);
        if let Some((repo, tag)) = image.rsplit_once(':') {
            // Only split if the colon is in the last path segment (not a registry port).
            if repo.rsplit('/').next().map_or(true, |s| !s.contains(':')) {
                return (repo.to_string(), Some(tag.to_string()));
            }
        }
        (image.to_string(), None)
    }

    /// Build a typed [`ContainerCreateBody`] from our `ContainerConfig` builder.
    fn build_body(config: &ContainerConfig, memory_bytes: Option<i64>) -> ContainerCreateBody {
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
                    "HostIp": "127.0.0.1",
                    "HostPort": host_port_str,
                }]),
            );
        }

        let binds: Vec<String> = config
            .volumes
            .iter()
            .filter_map(|v| match &v.source {
                crate::docker::config::VolumeSource::Bind(host_path) => {
                    Some(format!("{}:{}", host_path.display(), v.target.display()))
                }
                crate::docker::config::VolumeSource::Named(_) => None,
            })
            .collect();

        let host_config = ContainerHostConfig {
            base: HostConfig {
                port_bindings: if port_bindings.is_empty() {
                    None
                } else {
                    Some(PortMap {
                        data: port_bindings.into_iter().collect(),
                    })
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
                network_mode: config.network.clone(),
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

        ContainerCreateBody {
            config: gen_config,
            host_config: Some(host_config),
            networking_config: None,
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
                docker_err(DockerError::Connection(format!("inspect failed: {e}")))
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
                    if let Some(binding) = bindings.data.get(&key) {
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
            .field("ports", &self.ports)
            .finish()
    }
}

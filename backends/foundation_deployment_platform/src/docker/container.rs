//! Container lifecycle — the RAII guard for running Docker containers.

use std::collections::HashMap;

use bollard::models::{ContainerCreateBody, HostConfig};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, CreateImageOptionsBuilder, InspectContainerOptions,
    RemoveContainerOptionsBuilder, StartContainerOptions, StopContainerOptionsBuilder,
};
use tracing::{info, warn};

use crate::docker::config::ContainerConfig;
use crate::docker::error::{docker_err, DockerError, DockerResult};

/// RAII guard for a running Docker container.
pub struct ContainerHandle {
    docker: bollard::Docker,
    container_id: String,
    container_name: Option<String>,
    stop_timeout: u64,
    ports: HashMap<String, u16>,
}

impl ContainerHandle {
    /// Start a container: pull → create → start → inspect → wait.
    pub async fn start_async(config: ContainerConfig) -> DockerResult<Self> {
        let docker = bollard::Docker::connect_with_local_defaults().map_err(|e| {
            docker_err(DockerError::Connection(format!(
                "failed to connect to Docker daemon: {e}"
            )))
        })?;

        Self::ensure_image(&docker, &config.image, config.always_pull).await?;

        let body = Self::build_body(&config);

        let create_opts = config
            .name
            .as_deref()
            .map(|n| CreateContainerOptionsBuilder::default().name(n).build());

        let create_result = docker
            .create_container(create_opts, body)
            .await
            .map_err(|e| docker_err(DockerError::ContainerCreate(format!("{e}"))))?;

        let container_id = create_result.id;

        docker
            .start_container(&container_id, None::<StartContainerOptions>)
            .await
            .map_err(|e| docker_err(DockerError::ContainerStart(format!("{e}"))))?;

        let ports =
            Self::resolve_ports(&docker, &container_id, &config).await?;

        // Apply wait strategy
        config.wait.apply(&docker, &container_id, &ports).await?;

        Ok(Self {
            docker,
            container_id,
            container_name: config.name.clone(),
            stop_timeout: config.stop_timeout,
            ports,
        })
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

    pub async fn shutdown_async(&self) -> DockerResult<()> {
        let stop_opts = Some(
            StopContainerOptionsBuilder::default()
                .t(self.stop_timeout as i32)
                .build(),
        );
        if let Err(e) = self
            .docker
            .stop_container(&self.container_id, stop_opts)
            .await
        {
            warn!(container_id = %self.container_id, "failed to stop container: {e}");
        }

        let remove_opts = Some(
            RemoveContainerOptionsBuilder::default()
                .force(true)
                .v(true)
                .build(),
        );
        if let Err(e) = self
            .docker
            .remove_container(&self.container_id, remove_opts)
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
            .inspect_container(&self.container_id, None::<InspectContainerOptions>)
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
    ///
    /// Decision 09 fixes the lifecycle as pull → create → start; feature 05 adds
    /// the local cache. Pulling only on `always_pull` left a missing image to
    /// fail at `create_container` with an opaque `404: No such image`, which
    /// breaks every container test on a cold machine or in CI.
    async fn ensure_image(
        docker: &bollard::Docker,
        image: &str,
        always_pull: bool,
    ) -> DockerResult<()> {
        if !always_pull && docker.inspect_image(image).await.is_ok() {
            return Ok(());
        }
        Self::pull_image(docker, image).await
    }

    /// Qualify a bare repository with `:latest`.
    ///
    /// `create_image` treats a missing tag as "every tag in the repository", so
    /// an unqualified name would pull the entire repo. A colon in the final
    /// path segment is the tag; a colon before the last `/` is a registry port.
    fn with_default_tag(image: &str) -> String {
        let last_segment = image.rsplit('/').next().unwrap_or(image);
        if last_segment.contains(':') {
            image.to_string()
        } else {
            format!("{image}:latest")
        }
    }

    async fn pull_image(docker: &bollard::Docker, image: &str) -> DockerResult<()> {
        use futures_util::StreamExt;

        let tagged = Self::with_default_tag(image);
        info!(image = %tagged, "pulling image");

        let options = Some(
            CreateImageOptionsBuilder::default()
                .from_image(&tagged)
                .build(),
        );

        let mut stream = docker.create_image(options, None, None);
        while let Some(result) = stream.next().await {
            if let Err(e) = result {
                return Err(docker_err(DockerError::ImagePull {
                    image: image.to_string(),
                    reason: format!("{e}"),
                }));
            }
        }
        Ok(())
    }

    fn build_body(config: &ContainerConfig) -> ContainerCreateBody {
        let mut port_bindings = HashMap::new();
        let mut exposed_ports = Vec::new();

        for pm in &config.ports {
            let port_key = format!(
                "{}/{}",
                pm.container_port,
                match pm.protocol {
                    crate::docker::config::PortProtocol::Tcp => "tcp",
                    crate::docker::config::PortProtocol::Udp => "udp",
                }
            );
            exposed_ports.push(port_key.clone());

            let host_port_str = pm
                .host_port
                .map(|p| p.to_string())
                .unwrap_or_else(|| "0".to_string());
            port_bindings.insert(
                port_key,
                Some(vec![bollard::models::PortBinding {
                    host_ip: Some("127.0.0.1".to_string()),
                    host_port: Some(host_port_str),
                }]),
            );
        }

        let binds: Option<Vec<String>> = if config.volumes.is_empty() {
            None
        } else {
            Some(
                config
                    .volumes
                    .iter()
                    .filter_map(|v| match &v.source {
                        crate::docker::config::VolumeSource::Bind(host_path) => {
                            Some(format!("{}:{}", host_path.display(), v.target.display()))
                        }
                        crate::docker::config::VolumeSource::Named(_) => None,
                    })
                    .collect(),
            )
        };

        let host_config = HostConfig {
            port_bindings: Some(port_bindings),
            memory: config.memory.as_ref().and_then(|m| m.parse::<i64>().ok()),
            nano_cpus: config.cpus.map(|c| c as i64 * 1_000_000_000),
            devices: if config.devices.is_empty() {
                None
            } else {
                Some(
                    config
                        .devices
                        .iter()
                        .map(|d| bollard::models::DeviceMapping {
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
            cap_add: if config.cap_add.is_empty() {
                None
            } else {
                Some(config.cap_add.clone())
            },
            binds,
            extra_hosts: if config.extra_hosts.is_empty() {
                None
            } else {
                Some(config.extra_hosts.clone())
            },
            network_mode: config.network.clone(),
            ..Default::default()
        };

        ContainerCreateBody {
            image: Some(config.image.clone()),
            env: Some(
                config
                    .env
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect(),
            ),
            cmd: config.command.clone(),
            exposed_ports: if exposed_ports.is_empty() {
                None
            } else {
                Some(exposed_ports)
            },
            host_config: Some(host_config),
            labels: if config.labels.is_empty() {
                None
            } else {
                Some(config.labels.clone())
            },
            ..Default::default()
        }
    }

    async fn resolve_ports(
        docker: &bollard::Docker,
        container_id: &str,
        config: &ContainerConfig,
    ) -> DockerResult<HashMap<String, u16>> {
        let info = docker
            .inspect_container(container_id, None::<InspectContainerOptions>)
            .await
            .map_err(|e| {
                docker_err(DockerError::Connection(format!("inspect failed: {e}")))
            })?;

        let mut ports = HashMap::new();

        if let Some(settings) = &info.network_settings {
            if let Some(bindings) = &settings.ports {
                for pm in &config.ports {
                    let key = format!(
                        "{}/{}",
                        pm.container_port,
                        match pm.protocol {
                            crate::docker::config::PortProtocol::Tcp => "tcp",
                            crate::docker::config::PortProtocol::Udp => "udp",
                        }
                    );
                    if let Some(Some(binding_list)) = bindings.get(&key) {
                        if let Some(first) = binding_list.first() {
                            if let Some(host_port_str) = &first.host_port {
                                if let Ok(port) = host_port_str.parse::<u16>() {
                                    ports.insert(key, port);
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
            let stop_opts = Some(
                StopContainerOptionsBuilder::default()
                    .t(timeout as i32)
                    .build(),
            );
            if let Err(e) = docker.stop_container(&id, stop_opts).await {
                warn!(container_id = %id, "drop: failed to stop container: {e}");
            }
            let remove_opts = Some(
                RemoveContainerOptionsBuilder::default()
                    .force(true)
                    .v(true)
                    .build(),
            );
            if let Err(e) = docker.remove_container(&id, remove_opts).await {
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

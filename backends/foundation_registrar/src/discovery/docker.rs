//! Docker discovery through the generated Docker client.
//!
//! WHY: in container mode the installer creates the container `<name>` with
//! the labels `dev.ewe.entry=<name>` and `dev.ewe.version=<version>`
//! (spec 79 feature 00, section 4.4). The sidecar lists by the label and
//! inspects the container for its state, start time and IP.
//!
//! WHAT: [`ContainerApi`] (the small seam over the concrete client),
//! [`DockerContainerApi`] (the host implementation over `DockerClient`) and
//! [`DockerDiscovery`].
//!
//! HOW: `container_list` takes the label filter, `inspect_container` returns
//! the generated `ContainerInspectResponse`, and the network settings give the
//! container IP on `RegistrarConfig::docker_network`.

use std::net::IpAddr;
use std::sync::Arc;

use async_trait::async_trait;
use foundation_deployment_docker::generated::json::ContainerInspectResponse;
use foundation_deployment_docker::generated::stats::ContainerStatsResponse;
use foundation_deployment_docker::DockerClient;

use crate::discovery::{Discovered, Discovery, RunHandle};
use crate::error::RegistrarError;
use crate::manifest::EntryView;
use crate::RegistrarContext;

/// The label that names the manifest entry.
pub const ENTRY_LABEL: &str = "dev.ewe.entry";
/// The label that names the version.
pub const VERSION_LABEL: &str = "dev.ewe.version";

/// The Docker calls that discovery and metrics use.
///
/// WHY: `foundation_deployment_docker::DockerClient` is a concrete type, so a
/// test cannot replace it with a fake. This trait is the seam: production uses
/// [`DockerContainerApi`], and a test returns generated values.
#[async_trait]
pub trait ContainerApi: Send + Sync {
    /// The containers with `label`, as the daemon's JSON list.
    ///
    /// # Errors
    /// [`RegistrarError::Discovery`] when the daemon fails.
    async fn container_list_label(
        &self,
        label: &str,
    ) -> Result<Vec<serde_json::Value>, RegistrarError>;

    /// Inspect one container.
    ///
    /// # Errors
    /// [`RegistrarError::Discovery`] when the daemon fails.
    async fn inspect_container(&self, id: &str)
        -> Result<ContainerInspectResponse, RegistrarError>;

    /// One resource-usage sample of one container.
    ///
    /// # Errors
    /// [`RegistrarError::Metrics`] when the daemon fails.
    async fn container_stats(&self, id: &str) -> Result<ContainerStatsResponse, RegistrarError>;
}

/// The host implementation over `foundation_deployment_docker`.
#[derive(Debug, Clone)]
pub struct DockerContainerApi {
    client: Arc<DockerClient>,
}

impl DockerContainerApi {
    /// A seam over `client`.
    #[must_use]
    pub fn new(client: Arc<DockerClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ContainerApi for DockerContainerApi {
    async fn container_list_label(
        &self,
        label: &str,
    ) -> Result<Vec<serde_json::Value>, RegistrarError> {
        let filters = serde_json::json!({ "label": [label] }).to_string();
        let value = self
            .client
            .container_list(true, None, false, Some(&filters))
            .await
            .map_err(|error| RegistrarError::Discovery {
                entry: label.to_string(),
                reason: error.to_string(),
            })?;
        Ok(value.as_array().cloned().unwrap_or_default())
    }

    async fn inspect_container(
        &self,
        id: &str,
    ) -> Result<ContainerInspectResponse, RegistrarError> {
        self.client
            .inspect_container(id)
            .await
            .map_err(|error| RegistrarError::Discovery {
                entry: id.to_string(),
                reason: error.to_string(),
            })
    }

    async fn container_stats(&self, id: &str) -> Result<ContainerStatsResponse, RegistrarError> {
        self.client
            .container_stats(id, true)
            .await
            .map_err(|error| RegistrarError::Metrics(error.to_string()))
    }
}

/// An adapter for a host with no Docker daemon. Every call is a discovery
/// error, never a silent "not installed".
#[derive(Debug, Clone, Copy, Default)]
pub struct UnavailableDocker;

#[async_trait]
impl ContainerApi for UnavailableDocker {
    async fn container_list_label(
        &self,
        label: &str,
    ) -> Result<Vec<serde_json::Value>, RegistrarError> {
        Err(RegistrarError::Discovery {
            entry: label.to_string(),
            reason: "no Docker daemon on this host".to_string(),
        })
    }

    async fn inspect_container(
        &self,
        id: &str,
    ) -> Result<ContainerInspectResponse, RegistrarError> {
        Err(RegistrarError::Discovery {
            entry: id.to_string(),
            reason: "no Docker daemon on this host".to_string(),
        })
    }

    async fn container_stats(&self, id: &str) -> Result<ContainerStatsResponse, RegistrarError> {
        Err(RegistrarError::Metrics(format!(
            "container {id}: no Docker daemon on this host"
        )))
    }
}

/// Reads one entry's Docker container.
pub struct DockerDiscovery {
    api: Arc<dyn ContainerApi>,
    network: Option<String>,
}

impl std::fmt::Debug for DockerDiscovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockerDiscovery")
            .field("network", &self.network)
            .finish_non_exhaustive()
    }
}

impl DockerDiscovery {
    /// An adapter over `api`. `network` selects the network for the container
    /// IP; `None` takes the first network that the container joined.
    #[must_use]
    pub fn new(api: Arc<dyn ContainerApi>, network: Option<String>) -> Self {
        Self { api, network }
    }
}

#[async_trait]
impl Discovery for DockerDiscovery {
    fn kind(&self) -> &'static str {
        "docker"
    }

    async fn discover(
        &self,
        entry: &EntryView,
        _ctx: &RegistrarContext,
    ) -> Result<Discovered, RegistrarError> {
        let label = format!("{ENTRY_LABEL}={}", entry.name);
        let containers = self
            .api
            .container_list_label(&label)
            .await
            .map_err(|error| RegistrarError::Discovery {
                entry: entry.name.clone(),
                reason: error.to_string(),
            })?;
        let Some(chosen) = choose_container(&containers) else {
            return Ok(Discovered::NotInstalled);
        };
        let Some(id) = chosen.get("Id").and_then(serde_json::Value::as_str) else {
            return Ok(Discovered::NotInstalled);
        };
        let inspect =
            self.api
                .inspect_container(id)
                .await
                .map_err(|error| RegistrarError::Discovery {
                    entry: entry.name.clone(),
                    reason: error.to_string(),
                })?;

        let labels = inspect
            .config
            .as_ref()
            .and_then(|config| config.labels.as_ref());
        let marked_entry = labels
            .and_then(|labels| labels.get(ENTRY_LABEL))
            .and_then(serde_json::Value::as_str);
        if marked_entry != Some(entry.name.as_str()) {
            return Ok(Discovered::NotInstalled);
        }
        let version = labels
            .and_then(|labels| labels.get(VERSION_LABEL))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();

        let running = inspect
            .state
            .as_ref()
            .and_then(|state| state.running)
            .unwrap_or(false);
        if !running {
            return Ok(Discovered::Stopped { version });
        }

        let started_at_unix_ms = inspect
            .state
            .as_ref()
            .and_then(|state| state.started_at.as_deref())
            .and_then(parse_rfc3339_ms)
            .unwrap_or(0);
        let restarts = inspect
            .restart_count
            .and_then(|count| u32::try_from(count).ok())
            .unwrap_or(0);
        let container_ip = container_ip(&inspect, self.network.as_deref());
        let id = inspect.id.clone().unwrap_or_else(|| id.to_string());
        Ok(Discovered::Running {
            version,
            handle: RunHandle::Container { id },
            started_at_unix_ms,
            restarts,
            container_ip,
        })
    }
}

/// Prefer a running container, else the newest (the daemon lists newest first).
fn choose_container(containers: &[serde_json::Value]) -> Option<&serde_json::Value> {
    containers
        .iter()
        .find(|container| {
            container.get("State").and_then(serde_json::Value::as_str) == Some("running")
        })
        .or_else(|| containers.first())
}

/// The container IP on `network`, or on the first network with an address.
fn container_ip(inspect: &ContainerInspectResponse, network: Option<&str>) -> Option<IpAddr> {
    let networks = inspect
        .network_settings
        .as_ref()
        .and_then(|settings| settings.networks.as_ref())?
        .as_object()?;
    if let Some(network) = network {
        return networks
            .get(network)?
            .get("IPAddress")
            .and_then(serde_json::Value::as_str)
            .and_then(|ip| ip.parse().ok());
    }
    networks.values().find_map(|settings| {
        settings
            .get("IPAddress")
            .and_then(serde_json::Value::as_str)
            .filter(|ip| !ip.is_empty())
            .and_then(|ip| ip.parse().ok())
    })
}

/// Parse an RFC 3339 timestamp from Docker into Unix milliseconds.
fn parse_rfc3339_ms(text: &str) -> Option<u64> {
    let parsed = chrono::DateTime::parse_from_rfc3339(text).ok()?;
    u64::try_from(parsed.timestamp_millis()).ok()
}

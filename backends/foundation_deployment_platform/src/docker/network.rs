//! Docker network management.

use crate::docker::error::{docker_err, DockerError, DockerResult};

/// A handle to a Docker network.
#[derive(Debug, Clone)]
pub struct NetworkHandle {
    network_id: String,
    name: String,
}

impl NetworkHandle {
    /// Create or find a bridge network. Idempotent.
    pub async fn create_or_find(
        docker: &bollard::Docker,
        name: &str,
        _subnet: Option<&str>,
    ) -> DockerResult<Self> {
        if let Some(existing) = Self::find(docker, name).await? {
            return Ok(existing);
        }

        let config = bollard::models::NetworkCreateRequest {
            name: name.to_string(),
            driver: Some("bridge".to_string()),
            ..Default::default()
        };

        let response = docker
            .create_network(config)
            .await
            .map_err(|e| docker_err(DockerError::Network(format!("{e}"))))?;

        Ok(Self { network_id: response.id, name: name.to_string() })
    }

    /// Find an existing network by name.
    pub async fn find(
        docker: &bollard::Docker,
        name: &str,
    ) -> DockerResult<Option<Self>> {
        let networks = docker
            .list_networks(None::<bollard::query_parameters::ListNetworksOptions>)
            .await
            .map_err(|e| docker_err(DockerError::Network(format!("{e}"))))?;

        for net in networks {
            if net.name.as_deref() == Some(name) {
                let id = net.id.unwrap_or_else(|| name.to_string());
                return Ok(Some(Self { network_id: id, name: name.to_string() }));
            }
        }

        Ok(None)
    }

    #[must_use]
    pub fn id(&self) -> &str { &self.network_id }
    #[must_use]
    pub fn name(&self) -> &str { &self.name }

    /// Connect a container to this network.
    pub async fn connect(
        &self,
        docker: &bollard::Docker,
        container_id: &str,
    ) -> DockerResult<()> {
        let config = bollard::models::NetworkConnectRequest {
            container: container_id.to_string(),
            endpoint_config: Some(bollard::models::EndpointSettings::default()),
        };
        docker
            .connect_network(&self.network_id, config)
            .await
            .map_err(|e| docker_err(DockerError::Network(format!("{e}"))))?;
        Ok(())
    }

    /// Remove the network.
    pub async fn remove(&self, docker: &bollard::Docker) -> DockerResult<()> {
        docker
            .remove_network(&self.name)
            .await
            .map_err(|e| docker_err(DockerError::Network(format!("{e}"))))?;
        Ok(())
    }
}

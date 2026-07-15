//! Docker network management.

use foundation_deployment_docker::DockerClient;
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
        client: &DockerClient,
        name: &str,
        _subnet: Option<&str>,
    ) -> DockerResult<Self> {
        if let Some(existing) = Self::find(client, name).await? {
            return Ok(existing);
        }

        let config = serde_json::json!({
            "Name": name,
            "Driver": "bridge",
        });

        let response = client
            .network_create(config)
            .await
            .map_err(|e| docker_err(DockerError::Network(format!("{e}"))))?;

        Ok(Self { network_id: response.id, name: name.to_string() })
    }

    /// Find an existing network by name.
    pub async fn find(
        client: &DockerClient,
        name: &str,
    ) -> DockerResult<Option<Self>> {
        let networks = client
            .network_list(None)
            .await
            .map_err(|e| docker_err(DockerError::Network(format!("{e}"))))?;

        for net in networks.as_array().unwrap_or(&vec![]) {
            if net.get("Name").and_then(|n| n.as_str()) == Some(name) {
                let id = net
                    .get("Id")
                    .and_then(|i| i.as_str())
                    .unwrap_or(name)
                    .to_string();
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
        client: &DockerClient,
        container_id: &str,
    ) -> DockerResult<()> {
        client
            .network_connect(&self.network_id, container_id)
            .await
            .map_err(|e| docker_err(DockerError::Network(format!("{e}"))))?;
        Ok(())
    }

    /// Remove the network.
    pub async fn remove(&self, client: &DockerClient) -> DockerResult<()> {
        client
            .network_delete(&self.name)
            .await
            .map_err(|e| docker_err(DockerError::Network(format!("{e}"))))?;
        Ok(())
    }
}

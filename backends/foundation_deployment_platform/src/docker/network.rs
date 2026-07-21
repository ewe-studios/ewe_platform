//! Docker network management (Decision 10).
//!
//! **WHY:** Containers that must talk to each other need a shared user-defined
//! network — Docker's embedded DNS (127.0.0.11) then resolves container names
//! and aliases to IPs on that network, which is how a test's app container
//! finds its database without knowing any IP or published port.
//!
//! **WHAT:** [`NetworkHandle`] creates (or finds) a bridge network, connects
//! containers to it with optional DNS aliases, and can remove it — on Drop if
//! asked.
//!
//! **HOW:** Holds its own [`DockerClient`] so a handle is self-contained (Drop
//! cannot take one as an argument). `create_or_find` is idempotent so parallel
//! tests naming the same network do not race each other into a 409.

use foundation_deployment_docker::DockerClient;
use tracing::warn;

use crate::docker::error::{docker_err, DockerError, DockerResult};

/// A handle to a Docker network.
#[derive(Clone)]
pub struct NetworkHandle {
    client: DockerClient,
    network_id: String,
    name: String,
    remove_on_drop: bool,
}

impl NetworkHandle {
    /// Create or find a bridge network. Idempotent — returns the existing
    /// network if one with the same name is already there.
    ///
    /// `subnet` is optional (e.g. `"172.20.0.0/16"`); when `None` Docker
    /// auto-assigns one. A named subnet is passed through as an IPAM config, so
    /// callers that need predictable addressing get it.
    ///
    /// # Errors
    /// Returns [`DockerError::Network`] if the daemon rejects the create/list.
    pub async fn create_or_find(
        client: &DockerClient,
        name: &str,
        subnet: Option<&str>,
    ) -> DockerResult<Self> {
        if let Some(existing) = Self::find(client, name).await? {
            return Ok(existing);
        }

        let mut config = serde_json::json!({
            "Name": name,
            "Driver": "bridge",
        });
        if let Some(subnet) = subnet {
            config["IPAM"] = serde_json::json!({
                "Driver": "default",
                "Config": [{ "Subnet": subnet }],
            });
        }

        let response = client
            .network_create(config)
            .await
            .map_err(|e| docker_err(DockerError::Network(format!("{e}"))))?;

        Ok(Self {
            client: client.clone(),
            network_id: response.id,
            name: name.to_string(),
            remove_on_drop: false,
        })
    }

    /// Find an existing network by name. `None` if there is no such network.
    ///
    /// # Errors
    /// Returns [`DockerError::Network`] if the daemon rejects the list.
    pub async fn find(client: &DockerClient, name: &str) -> DockerResult<Option<Self>> {
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
                return Ok(Some(Self {
                    client: client.clone(),
                    network_id: id,
                    name: name.to_string(),
                    remove_on_drop: false,
                }));
            }
        }

        Ok(None)
    }

    /// The network's Docker ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.network_id
    }

    /// The network name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Remove this network when the handle drops (off by default, since
    /// `create_or_find` may have returned a network someone else owns).
    #[must_use]
    pub fn remove_on_drop(mut self) -> Self {
        self.remove_on_drop = true;
        self
    }

    /// Connect a container to this network, answering to `aliases` in addition
    /// to its container name.
    ///
    /// # Errors
    /// Returns [`DockerError::Network`] if the daemon rejects the connect.
    pub async fn connect(&self, container_id: &str, aliases: &[&str]) -> DockerResult<()> {
        self.client
            .network_connect(&self.network_id, container_id, aliases)
            .await
            .map_err(|e| docker_err(DockerError::Network(format!("{e}"))))?;
        Ok(())
    }

    /// Remove the network. Fails while containers are still attached.
    ///
    /// # Errors
    /// Returns [`DockerError::Network`] if the daemon rejects the delete.
    pub async fn remove(&self) -> DockerResult<()> {
        self.client
            .network_delete(&self.name)
            .await
            .map_err(|e| docker_err(DockerError::Network(format!("{e}"))))?;
        Ok(())
    }
}

impl std::fmt::Debug for NetworkHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetworkHandle")
            .field("name", &self.name)
            .field("network_id", &self.network_id)
            .field("remove_on_drop", &self.remove_on_drop)
            .finish()
    }
}

impl Drop for NetworkHandle {
    fn drop(&mut self) {
        if !self.remove_on_drop {
            return;
        }
        // Best-effort: a network with containers still attached cannot be
        // removed, and that must not panic a test's unwind.
        let client = self.client.clone();
        let name = self.name.clone();
        let _ = crate::block_on(async move {
            if let Err(e) = client.network_delete(&name).await {
                warn!(network = %name, "drop: failed to remove network: {e}");
            }
        });
    }
}

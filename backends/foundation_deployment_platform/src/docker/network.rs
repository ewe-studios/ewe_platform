//! Docker network management.
//!
//! **WHY:** Multi-container test scenarios need shared Docker networks so
//! containers can resolve each other by name. Creating and managing networks
//! should be idempotent — create-on-first-use, reuse thereafter.
//!
//! **WHAT:** `NetworkHandle` provides `create_or_find()` (idempotent) and
//! `connect()` for attaching containers. Networks persist across test runs
//! by default (empty networks consume no resources).
//!
//! **HOW:** Wraps a Docker network ID. Uses bollard's `create_network` and
//! `inspect_network` under the hood.

use crate::docker::client::DockerClient;
use crate::docker::error::DockerResult;

/// A handle to a Docker network. Created via `create_or_find()`.
/// Networks persist by default — call `remove()` explicitly if needed.
#[derive(Debug, Clone)]
pub struct NetworkHandle {
    /// The network's Docker ID (hash).
    network_id: String,
    /// Human-readable name.
    name: String,
}

impl NetworkHandle {
    /// Create or find a bridge network with the given name. Idempotent —
    /// if the network already exists, returns its handle without error.
    pub async fn create_or_find(
        _client: &DockerClient,
        name: &str,
        _subnet: Option<&str>,
    ) -> DockerResult<Self> {
        // TODO: bollard network create + inspect implementation
        Ok(Self {
            network_id: String::new(),
            name: name.to_string(),
        })
    }

    /// Create a handle for an existing network by name. Returns `None`
    /// if the network is not found.
    pub async fn find(_client: &DockerClient, name: &str) -> DockerResult<Option<Self>> {
        // TODO: bollard list_networks + filter by name
        Ok(Some(Self {
            network_id: String::new(),
            name: name.to_string(),
        }))
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

    /// Connect a container to this network with optional DNS aliases.
    /// Aliases are additional hostnames the container responds to on
    /// the network.
    pub async fn connect(
        &self,
        _container_id: &str,
        _aliases: &[&str],
    ) -> DockerResult<()> {
        // TODO: bollard connect_container_to_network
        Ok(())
    }

    /// Remove the network. All containers must be disconnected first.
    pub async fn remove(&self, _client: &DockerClient) -> DockerResult<()> {
        // TODO: bollard remove_network
        Ok(())
    }
}

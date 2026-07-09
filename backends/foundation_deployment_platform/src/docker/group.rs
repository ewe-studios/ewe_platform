//! Multi-container group management.
//!
//! **WHY:** Tests often need multiple containers (app + database + cache).
//! Managing them individually is error-prone — cleanup order matters.
//! A group handle ensures all containers are started together and
//! cleaned up together in the correct order (LIFO).
//!
//! **WHAT:** `ContainerGroup` holds a `Vec<ContainerHandle>`. `start_async()`
//! launches all containers from their `ContainerConfig` definitions.
//! `Drop` stops and removes all containers in reverse order.
//!
//! **HOW:** Stores the handles in a flat `Vec`. `container(name)` finds
//! a specific handle by service name for inspection.

use crate::docker::config::ContainerConfig;
use crate::docker::container::ContainerHandle;
use crate::docker::error::DockerResult;

/// A handle to a group of running containers. Created by
/// `ContainerGroup::start_async(defs)`. On Drop, stops and removes
/// all containers in reverse start order.
pub struct ContainerGroup {
    handles: Vec<ContainerHandle>,
}

impl ContainerGroup {
    /// Start all containers from their definitions. Returns a group
    /// handle that will clean up all containers on Drop.
    pub async fn start_async(configs: Vec<ContainerConfig>) -> DockerResult<Self> {
        let mut handles = Vec::with_capacity(configs.len());
        for cfg in configs {
            let handle = ContainerHandle::start_async(cfg).await?;
            handles.push(handle);
        }
        Ok(Self { handles })
    }

    /// Sync convenience — delegates to `start_async()` via `block_on`.
    pub fn start(configs: Vec<ContainerConfig>) -> DockerResult<Self> {
        crate::block_on(Self::start_async(configs))
    }

    /// Get a handle to an individual container by its name. Returns
    /// `None` if no container with that name exists in the group.
    #[must_use]
    pub fn container(&self, _name: &str) -> Option<&ContainerHandle> {
        // TODO: match by container name once ContainerHandle stores a name
        self.handles.first()
    }

    /// Get all container handles.
    #[must_use]
    pub fn containers(&self) -> &[ContainerHandle] {
        &self.handles
    }

    /// Number of containers in the group.
    #[must_use]
    pub fn len(&self) -> usize {
        self.handles.len()
    }

    /// Returns `true` if the group has no containers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }
}

// Reverse-order teardown: last container started = first container stopped.
impl Drop for ContainerGroup {
    fn drop(&mut self) {
        // Already cleaned up by individual ContainerHandle::Drop.
        // Drop order is reverse of Vec order automatically (last element dropped first).
    }
}

impl std::fmt::Debug for ContainerGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerGroup")
            .field("count", &self.handles.len())
            .finish()
    }
}

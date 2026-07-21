//! Multi-container group management.
//!
//! **WHY:** Tests often need multiple containers (app + database + cache).
//! Managing them individually is error-prone — cleanup order matters.
//! A group handle ensures all containers are started together and
//! cleaned up together in the correct order (LIFO).
//!
//! **WHAT:** `ContainerGroup` holds the started containers in start order.
//! `start_async()` launches them all from their `ContainerConfig` definitions.
//! `Drop` stops and removes all containers in reverse order.
//!
//! **HOW:** Stores entries in a flat `Vec`. `container(key)` finds a specific
//! handle for inspection, by **lookup key** — which is either the logical key
//! the caller attached (`#[docker_container(as = "cache")]`) or the container's
//! real Docker name. The logical key exists because Docker names are global to
//! the daemon: pinning one to make a handle findable would make two parallel
//! tests collide with a 409. A logical key never reaches the Docker API, so the
//! daemon keeps auto-naming containers uniquely.

use crate::docker::config::ContainerConfig;
use crate::docker::container::ContainerHandle;
use crate::docker::error::DockerResult;

/// One started container plus the key it is looked up by.
struct GroupEntry {
    /// Logical lookup key, independent of the Docker container name.
    key: Option<String>,
    handle: ContainerHandle,
}

/// A handle to a group of running containers. Created by
/// `ContainerGroup::start_async(defs)`, or built up by `#[docker_container]`.
/// On Drop, stops and removes all containers in reverse start order.
pub struct ContainerGroup {
    entries: Vec<GroupEntry>,
}

impl ContainerGroup {
    /// Start all containers from their definitions. Returns a group
    /// handle that will clean up all containers on Drop.
    pub async fn start_async(configs: Vec<ContainerConfig>) -> DockerResult<Self> {
        let mut entries = Vec::with_capacity(configs.len());
        for cfg in configs {
            let handle = ContainerHandle::start_async(cfg).await?;
            entries.push(GroupEntry { key: None, handle });
        }
        Ok(Self { entries })
    }

    /// An empty group, to be filled with [`ContainerGroup::insert`].
    ///
    /// `#[docker_container]` uses this: it seeds one group, starts every
    /// declared container into it in the order written, and hands it to the
    /// annotated function.
    #[must_use]
    pub fn empty() -> Self {
        Self { entries: Vec::new() }
    }

    /// Add a started container under an optional logical lookup key.
    ///
    /// # Panics
    /// If `key` is already used by another container in this group. Two handles
    /// answering to one key would make `container(key)` silently return an
    /// arbitrary one, so this is a hard error naming the duplicate.
    pub fn insert(&mut self, key: Option<String>, handle: ContainerHandle) {
        if let Some(ref k) = key {
            assert!(
                !self.entries.iter().any(|e| e.key.as_deref() == Some(k.as_str())),
                "duplicate container key {k:?} in this group — give each container a distinct `as = \"...\"`"
            );
        }
        self.entries.push(GroupEntry { key, handle });
    }

    /// Sync convenience — delegates to `start_async()` via `block_on`.
    pub fn start(configs: Vec<ContainerConfig>) -> DockerResult<Self> {
        crate::block_on(Self::start_async(configs))
    }

    /// Get a handle to an individual container by lookup key: the logical key
    /// given via `as = "..."`, or failing that the real Docker container name.
    #[must_use]
    pub fn container(&self, key: &str) -> Option<&ContainerHandle> {
        self.entries
            .iter()
            .find(|e| e.key.as_deref() == Some(key) || e.handle.name() == Some(key))
            .map(|e| &e.handle)
    }

    /// Get all container handles, in start order.
    #[must_use]
    pub fn containers(&self) -> Vec<&ContainerHandle> {
        self.entries.iter().map(|e| &e.handle).collect()
    }

    /// The handle started at `index` (0 = started first), for containers that
    /// were given no lookup key.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&ContainerHandle> {
        self.entries.get(index).map(|e| &e.handle)
    }

    /// Number of containers in the group.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the group has no containers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
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
        let keys: Vec<&str> = self
            .entries
            .iter()
            .map(|e| e.key.as_deref().or_else(|| e.handle.name()).unwrap_or("<unkeyed>"))
            .collect();
        f.debug_struct("ContainerGroup")
            .field("count", &self.entries.len())
            .field("keys", &keys)
            .finish()
    }
}

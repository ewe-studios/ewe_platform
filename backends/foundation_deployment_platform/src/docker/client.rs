//! Docker client — bollard wrapper.
//!
//! **WHY:** The bollard `Docker` client needs connection management —
//! creating it fresh for each container is wasteful, but a shared handle
//! is cheap (bollard's `Docker` is internally `Arc`'d). Auth management
//! and health checks should be collected in one place.
//!
//! **WHAT:** `DockerClient` wraps bollard's `Docker` with convenience
//! methods for local socket and SSH connections, plus `is_available()`
//! for quick health checking.
//!
//! **HOW:** Stores the bollard handle. `connect_local()` tries the
//! default socket path; `connect_ssh()` tunnels over SSH.

use bollard::Docker;
use crate::docker::error::{DockerError, DockerResult};

/// A thin wrapper around bollard's `Docker` handle. Cheap to clone
/// (bollard's `Docker` is internally reference-counted).
#[derive(Clone)]
pub struct DockerClient {
    inner: Docker,
}

impl DockerClient {
    /// Connect to the local Docker daemon via the default socket path.
    /// Tries `DOCKER_HOST` env var, then `/var/run/docker.sock`.
    pub async fn connect_local() -> DockerResult<Self> {
        let docker = Docker::connect_with_local_defaults().map_err(|e| {
            DockerError::Connection(format!("failed to connect to Docker daemon: {e}"))
        })?;
        Ok(Self { inner: docker })
    }

    /// Connect to a remote Docker daemon over SSH.
    pub async fn connect_ssh(
        host: &str,
        _key_paths: &[&str],
        _user: &str,
        _port: u16,
    ) -> DockerResult<Self> {
        // TODO: Docker::connect_with_ssh when bollard feature is enabled
        let _ = host;
        Err(DockerError::Connection(
            "SSH transport not yet implemented".to_string(),
        ))
    }

    /// Check if the Docker daemon is reachable.
    pub async fn is_available(&self) -> bool {
        self.inner.ping().await.is_ok()
    }

    /// Ping the Docker daemon. Returns an error if unreachable.
    pub async fn ping(&self) -> DockerResult<()> {
        self.inner.ping().await.map_err(|e| {
            DockerError::Connection(format!("docker ping failed: {e}"))
        })?;
        Ok(())
    }

    /// Access the inner bollard handle for advanced operations.
    #[must_use]
    pub fn inner(&self) -> &Docker {
        &self.inner
    }

    /// Consume the client and return the inner bollard handle.
    #[must_use]
    pub fn into_inner(self) -> Docker {
        self.inner
    }
}

impl std::fmt::Debug for DockerClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockerClient").finish()
    }
}

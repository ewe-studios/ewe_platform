//! Docker client — bollard wrapper.
//!
//! **WHY:** The bollard `Docker` client needs connection management.
//!
//! **WHAT:** `DockerClient` wraps bollard's `Docker` with convenience
//! methods for local socket and SSH connections.
//!
//! **HOW:** Stores the bollard handle. `connect_local()` tries the
//! default socket path; `connect_ssh()` tunnels over SSH.

use bollard::Docker;
use crate::docker::error::{docker_err, DockerError, DockerResult};

/// A thin wrapper around bollard's `Docker` handle.
#[derive(Clone)]
pub struct DockerClient {
    inner: Docker,
}

impl DockerClient {
    pub async fn connect_local() -> DockerResult<Self> {
        let docker = Docker::connect_with_local_defaults().map_err(|e| {
            docker_err(DockerError::Connection(format!(
                "failed to connect to Docker daemon: {e}"
            )))
        })?;
        Ok(Self { inner: docker })
    }

    pub async fn connect_ssh(
        _host: &str,
        _key_paths: &[&str],
        _user: &str,
        _port: u16,
    ) -> DockerResult<Self> {
        Err(docker_err(DockerError::Connection(
            "SSH transport not yet implemented".to_string(),
        )))
    }

    pub async fn is_available(&self) -> bool {
        self.inner.ping().await.is_ok()
    }

    pub async fn ping(&self) -> DockerResult<()> {
        self.inner.ping().await.map_err(|e| {
            docker_err(DockerError::Connection(format!(
                "docker ping failed: {e}"
            )))
        })?;
        Ok(())
    }

    #[must_use]
    pub fn inner(&self) -> &Docker {
        &self.inner
    }

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

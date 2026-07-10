//! Docker client — bollard wrapper.

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

    /// Connect to a remote Docker daemon over SSH using bollard's native
    /// SSH transport. Requires the `ssh` feature on bollard.
    #[cfg(feature = "bollard-ssh")]
    pub async fn connect_ssh(
        host: &str,
        key_paths: &[&str],
        user: &str,
        port: u16,
    ) -> DockerResult<Self> {
        let docker = Docker::connect_with_ssh(
            host,
            &key_paths.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
            user,
            port,
        ).map_err(|e| {
            docker_err(DockerError::Connection(format!(
                "SSH connection to {user}@{host}:{port} failed: {e}"
            )))
        })?;
        Ok(Self { inner: docker })
    }

    /// Connect via SSH using a pre-configured Host from foundation_sshkit.
    /// The Host provides key_paths, user, hostname, and port.
    #[cfg(feature = "sshkit")]
    pub async fn connect_via_host(ssh_host: &foundation_sshkit::Host) -> DockerResult<Self> {
        let keys: Vec<&str> = ssh_host.key_paths.iter()
            .filter_map(|p| p.to_str())
            .collect();
        Self::connect_ssh(&ssh_host.hostname, &keys, &ssh_host.user, ssh_host.port).await
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

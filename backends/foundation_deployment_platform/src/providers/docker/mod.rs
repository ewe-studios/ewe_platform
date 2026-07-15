//! DockerProvider — implements `Provider` using `ContainerHandle`.

use crate::docker::{ContainerConfig, ContainerHandle, DockerResult};
use crate::providers::{Provider, ProviderId, ResolvedPorts};

pub struct DockerProvider;

impl Provider for DockerProvider {
    type Handle = ContainerHandle;
    type Config = ContainerConfig;
    type Error = foundation_errstacks::ErrorTrace<crate::docker::DockerError>;

    fn name(&self) -> &'static str { "docker" }
    fn id(&self) -> ProviderId { ProviderId::Docker }

    fn launch(&self, config: &ContainerConfig) -> DockerResult<Self::Handle> {
        ContainerHandle::start(config.clone())
    }

    fn stop(&self, handle: &ContainerHandle) -> DockerResult<()> {
        handle.shutdown()
    }

    fn is_running(&self, handle: &ContainerHandle) -> bool {
        handle.is_running()
    }

    fn resolved_ports(&self, handle: &ContainerHandle) -> DockerResult<ResolvedPorts> {
        let ports = handle.host_ports();
        Ok(ResolvedPorts {
            ssh_port: ports.get("22/tcp").copied().unwrap_or(0),
            winrm_port: ports.get("5985/tcp").copied(),
            rdp_port: ports.get("3389/tcp").copied(),
            vnc_port: ports.get("5900/tcp").copied().unwrap_or(0),
        })
    }

    fn host_health(&self) -> Vec<(String, bool, String)> {
        let sock_ok = std::path::Path::new("/var/run/docker.sock").exists()
            || std::env::var("DOCKER_HOST").is_ok();
        vec![(
            "docker_socket".to_string(),
            sock_ok,
            if sock_ok {
                "Docker socket available".to_string()
            } else {
                "Docker socket not found".to_string()
            },
        )]
    }
}

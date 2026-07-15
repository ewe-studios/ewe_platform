//! Docker interaction layer — testcontainers-style container lifecycle.
//!
//! Provides `ContainerHandle` (RAII guard), `ContainerConfig` (builder),
//! `WaitFor` readiness strategies, `ContainerGroup` for multi-container
//! topologies, `NetworkHandle`, `DockerClient`, and `DockerFileConfig`
//! for custom image builds.
//!
//! **Async-first:** Core API is `async fn`. Sync callers use
//! `futures_lite::block_on` (re-exported from `foundation_deployment_platform`).

pub mod config;
pub mod container;
pub mod error;
pub mod group;
pub mod image;
pub mod network;
pub mod wait_for;
pub mod wireguard;

/// Re-export the spec-54 Docker client directly — the old bollard wrapper is gone.
pub use foundation_deployment_docker::DockerClient;
pub use config::{ContainerConfig, DeviceMapping, PortMapping, PortProtocol, VolumeMount, VolumeSource};
pub use container::ContainerHandle;
pub use error::{docker_err, DockerError, DockerResult};
pub use group::ContainerGroup;
pub use image::{DockerFileConfig, ImageBuildResult};
pub use network::NetworkHandle;
pub use wait_for::WaitFor;

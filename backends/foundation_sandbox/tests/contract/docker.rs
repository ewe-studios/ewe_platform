//! The Docker `network_none` rule (feature 10, sections 6.2 and 6.5).
//!
//! The Unix domain socket (UDS) link works with no container network. The
//! Transmission Control Protocol (TCP) link needs one, so `network_none` is
//! false there and `validate` refuses `NetworkMode::None`.

#![cfg(feature = "docker")]

use foundation_sandbox::native::docker::driver::{DockerDriver, DockerDriverConfig, DockerLink};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::spec::{NetworkMode, RootFs, SandboxSpec};

/// A spec from an image with the given network mode.
fn spec(network: NetworkMode) -> SandboxSpec {
    let mut spec = SandboxSpec::new(RootFs::ContainerImage("alpine:3.22".to_string()));
    spec.network = network;
    spec
}

#[test]
fn validate_network_none() {
    let unix = DockerDriver::with_config(DockerDriverConfig::default().with_link(DockerLink::Unix))
        .capabilities(8);
    assert!(unix.network_none);
    assert!(unix.network_host);
    assert!(unix.rootfs_container_image);
    spec(NetworkMode::None)
        .validate(&unix)
        .expect("the Unix link needs no container network");
    spec(NetworkMode::Host)
        .validate(&unix)
        .expect("the Unix link also accepts the host network");

    let tcp = DockerDriver::with_config(DockerDriverConfig::default().with_link(DockerLink::Tcp))
        .capabilities(8);
    assert!(!tcp.network_none);
    let error = spec(NetworkMode::None)
        .validate(&tcp)
        .expect_err("the TCP link needs a container network");
    assert!(matches!(
        error.current_context(),
        SandboxError::UnsupportedNetwork
    ));
    spec(NetworkMode::Host)
        .validate(&tcp)
        .expect("the TCP link accepts the host network");
}

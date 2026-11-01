//! Shared helpers for Docker-gated integration tests.
//!
//! WHY: Several test targets each copied a `docker_available()` that only
//! checks that `/var/run/docker.sock` exists. A socket file can exist while
//! no daemon listens on it, so `--ignored` runs failed with "Docker daemon
//! not reachable" instead of skipping.
//!
//! WHAT: One helper, [`docker_daemon_reachable`], connects with
//! [`DockerClient::connect_with_defaults`] and pings the daemon. Its sync
//! wrapper [`docker_available`] serves `#[test]` files.
//!
//! HOW: The ping is [`DockerClient::negotiate_version`]: it reads the
//! unversioned `GET /version` (the same call a real request makes first) and
//! also checks that the API versions overlap.

use foundation_deployment_docker::DockerClient;

/// Return `true` when a Docker daemon answers `GET /version`.
///
/// The Docker client's transport is valtron-based, so a valtron pool must be
/// live before this is awaited.
pub async fn docker_daemon_reachable() -> bool {
    let Ok(client) = DockerClient::connect_with_defaults() else {
        return false;
    };
    client.negotiate_version().await.is_ok()
}

/// Sync wrapper for [`docker_daemon_reachable`], for `#[test]` files.
///
/// The caller must initialize a valtron pool first (with `initialize_pool`),
/// or run under `#[valtron_test]`, which keeps one alive.
pub fn docker_available() -> bool {
    foundation_core::valtron::block_on_future(docker_daemon_reachable())
}

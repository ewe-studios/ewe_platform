//! Integration tests against a real Docker daemon on the default Unix socket.
//!
//! WHY: Proves `DockerClient` talks to an actual dockerd — version, container
//! create → start → inspect → pause → unpause → stop → remove lifecycle — over
//! the Unix-socket transport, driven by valtron.
//!
//! WHAT: Gated behind `integration-tests` feature (not `#[ignore]`).
//! Run with: `cargo test -p foundation_deployment_docker
//!   --features "docker,integration-tests" --profile uat -- real_docker`
//!
//! HOW: `DockerClient::connect_unix("/var/run/docker.sock")`.

#![cfg(all(unix, feature = "docker", feature = "integration-tests"))]

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;

#[valtron_test]
async fn real_docker_version() {
    let client = DockerClient::connect_unix("/var/run/docker.sock");
    let version = client.version().await.expect("GET /version");
    assert!(version.api_version.is_some(), "ApiVersion should be present");
    assert!(version.os.is_some(), "Os should be present");
    assert!(version.arch.is_some(), "Arch should be present");
}

#[valtron_test]
async fn real_docker_lifecycle_stop_while_running() {
    // WHY: stop on a RUNNING container verifies the full blocking lifecycle —
    // create → start → stop (waits for exit) → remove.
    let client = DockerClient::connect_unix("/var/run/docker.sock");

    let id = client
        .create_container(
            &serde_json::json!({
                "Image": "alpine:latest",
                "Cmd": ["sleep", "60"],
            }),
            Some("ewe-real-docker-test"),
        )
        .await
        .expect("create container");
    assert!(!id.id.is_empty());

    client.start_container(&id.id).await.expect("start container");

    // Stop while running — Docker blocks until container exits (up to 10s).
    // Our 120s read timeout handles this.
    client
        .stop_container(&id.id, Some(10))
        .await
        .expect("stop container (while running)");
    client.remove_container(&id.id, true).await.expect("remove container");
}

#[valtron_test]
async fn real_docker_stop_already_exited_handles_304() {
    // WHY: Docker returns 304 Not Modified when the container is already
    // exited — our `stop_container` wrapper treats 304 as success (the
    // desired state is reached).
    let client = DockerClient::connect_unix("/var/run/docker.sock");

    let id = client
        .create_container(
            &serde_json::json!({
                "Image": "alpine:latest",
                "Cmd": ["true"],
            }),
            Some("ewe-304-test"),
        )
        .await
        .expect("create");

    client.start_container(&id.id).await.expect("start");

    // "true" exits immediately — container is already stopped.
    std::thread::sleep(std::time::Duration::from_secs(2));

    // This returns 304 — DockerClient treats it as success.
    client
        .stop_container(&id.id, Some(5))
        .await
        .expect("stop (already exited) should handle 304");
    client
        .remove_container(&id.id, true)
        .await
        .expect("remove after 304 stop");
}

#[valtron_test]
async fn real_docker_pause_unpause() {
    let client = DockerClient::connect_unix("/var/run/docker.sock");

    let id = client
        .create_container(
            &serde_json::json!({
                "Image": "alpine:latest",
                "Cmd": ["sleep", "30"],
            }),
            Some("ewe-pause-test"),
        )
        .await
        .expect("create container");

    client.start_container(&id.id).await.expect("start container");
    client.pause_container(&id.id).await.expect("pause container");

    let inspect = client.inspect_container(&id.id).await.expect("inspect after pause");
    assert!(
        inspect.state.as_ref().and_then(|s| s.paused).unwrap_or(false),
        "container should be paused"
    );

    client.unpause_container(&id.id).await.expect("unpause container");

    let inspect = client.inspect_container(&id.id).await.expect("inspect after unpause");
    assert!(
        inspect.state.as_ref().and_then(|s| s.running).unwrap_or(false),
        "container should be running after unpause"
    );

    client.stop_container(&id.id, Some(5)).await.expect("stop container");
    client.remove_container(&id.id, true).await.expect("remove container");
}
